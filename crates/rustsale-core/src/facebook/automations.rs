use anyhow::Result;
use reqwest::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentAutomationRule {
    pub id: i64,
    pub channel_account_id: i64,
    pub page_id: String,
    pub rule_name: String,
    pub post_ids: Vec<String>,
    pub keywords: Vec<String>,
    pub match_mode: String,
    pub auto_like: bool,
    pub public_reply_template: Option<String>,
    pub private_reply_template: Option<String>,
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentLogEntry {
    pub id: i64,
    pub comment_id: String,
    pub page_id: String,
    pub post_id: Option<String>,
    pub sender_psid: String,
    pub sender_name: Option<String>,
    pub comment_text: Option<String>,
    pub automation_id: Option<i64>,
    pub like_status: String,
    pub public_reply_status: String,
    pub private_reply_status: String,
    pub owner_id: Option<String>,
    pub claim_deadline: Option<String>,
    pub skip_reason: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct FacebookAutomationEngine {
    client: Client,
}

impl Default for FacebookAutomationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct IncomingCommentParams<'a> {
    pub page_id: &'a str,
    pub post_id: Option<&'a str>,
    pub comment_id: &'a str,
    pub sender_psid: &'a str,
    pub sender_name: Option<&'a str>,
    pub comment_text: &'a str,
    pub created_time_epoch: Option<i64>,
    pub page_token: &'a str,
}

impl FacebookAutomationEngine {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    pub fn matches_keywords(comment_text: &str, keywords: &[String], match_mode: &str) -> bool {
        if keywords.is_empty() {
            return true;
        }

        let text_lower = comment_text.to_lowercase();

        match match_mode {
            "all" => keywords
                .iter()
                .all(|k| text_lower.contains(&k.to_lowercase())),
            "exact" => keywords
                .iter()
                .any(|k| text_lower.trim() == k.to_lowercase().trim()),
            "regex" => keywords.iter().any(|pattern| {
                if pattern.len() > 256 {
                    return false;
                }
                if let Ok(re) = regex::RegexBuilder::new(pattern)
                    .case_insensitive(true)
                    .size_limit(64 * 1024)
                    .dfa_size_limit(64 * 1024)
                    .build()
                {
                    re.is_match(comment_text)
                } else {
                    false
                }
            }),
            _ => {
                // "contains" default
                keywords
                    .iter()
                    .any(|k| text_lower.contains(&k.to_lowercase()))
            }
        }
    }

    pub async fn process_incoming_comment(
        &self,
        conn: &Connection,
        params: &IncomingCommentParams<'_>,
    ) -> Result<Option<i64>> {
        // 1. Loop guard: skip if sender is the page itself
        if params.sender_psid == params.page_id {
            return Ok(None);
        }

        // 2. 7-day age window check
        if let Some(epoch) = params.created_time_epoch {
            let now_ts = chrono::Utc::now().timestamp();
            if now_ts - epoch > 7 * 86400 {
                // Older than 7 days, skip
                return Ok(None);
            }
        }

        // 3. Find matching active rule
        let mut stmt = conn.prepare(
            "SELECT id, channel_account_id, page_id, rule_name, post_ids_json, keywords_json, match_mode, auto_like, public_reply_template, private_reply_template, is_enabled, created_at, updated_at
             FROM facebook_comment_automations
             WHERE page_id = ?1 AND is_enabled = 1
             ORDER BY id ASC",
        )?;

        let rules: Vec<CommentAutomationRule> = stmt
            .query_map([params.page_id], |r| {
                let post_ids_raw: String = r.get(4)?;
                let keywords_raw: String = r.get(5)?;
                Ok(CommentAutomationRule {
                    id: r.get(0)?,
                    channel_account_id: r.get(1)?,
                    page_id: r.get(2)?,
                    rule_name: r.get(3)?,
                    post_ids: serde_json::from_str(&post_ids_raw).unwrap_or_default(),
                    keywords: serde_json::from_str(&keywords_raw).unwrap_or_default(),
                    match_mode: r.get(6)?,
                    auto_like: r.get::<_, i64>(7)? == 1,
                    public_reply_template: r.get(8)?,
                    private_reply_template: r.get(9)?,
                    is_enabled: r.get::<_, i64>(10)? == 1,
                    created_at: r.get(11)?,
                    updated_at: r.get(12)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let matched_rule = rules.into_iter().find(|rule| {
            // Check post_id filter if specified in rule
            if !rule.post_ids.is_empty() {
                if let Some(pid) = params.post_id {
                    if !rule.post_ids.contains(&pid.to_string()) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            // Check keyword match
            Self::matches_keywords(params.comment_text, &rule.keywords, &rule.match_mode)
        });

        let rule = match matched_rule {
            Some(r) => r,
            None => return Ok(None),
        };

        // 4. Atomic 1-reply claim in facebook_comment_logs
        let insert_rows = conn.execute(
            "INSERT OR IGNORE INTO facebook_comment_logs
             (comment_id, page_id, post_id, sender_psid, sender_name, comment_text, automation_id, like_status, public_reply_status, private_reply_status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'processing', 'processing', 'processing', datetime('now'), datetime('now'))",
            params![
                params.comment_id,
                params.page_id,
                params.post_id,
                params.sender_psid,
                params.sender_name,
                params.comment_text,
                rule.id
            ],
        )?;

        if insert_rows == 0 {
            // Already claimed / processed for this comment_id
            return Ok(None);
        }

        let log_id = conn.last_insert_rowid();

        // 5. Execute Auto-Like
        if rule.auto_like {
            let like_url = format!(
                "https://graph.facebook.com/v26.0/{}/likes",
                params.comment_id
            );
            let resp = self
                .client
                .post(&like_url)
                .header("Authorization", format!("Bearer {}", params.page_token))
                .send()
                .await;

            let like_status = match resp {
                Ok(r) if r.status().is_success() => "success",
                _ => "failed",
            };
            let _ = conn.execute(
                "UPDATE facebook_comment_logs SET like_status = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![like_status, log_id],
            );
        } else {
            let _ = conn.execute(
                "UPDATE facebook_comment_logs SET like_status = 'none', updated_at = datetime('now') WHERE id = ?1",
                params![log_id],
            );
        }

        // 6. Execute Public Reply
        if let Some(ref template) = rule.public_reply_template {
            if !template.trim().is_empty() {
                let public_reply_text = Self::render_template(template, params.sender_name);
                let reply_url = format!(
                    "https://graph.facebook.com/v26.0/{}/comments",
                    params.comment_id
                );
                let resp = self
                    .client
                    .post(&reply_url)
                    .header("Authorization", format!("Bearer {}", params.page_token))
                    .json(&json!({
                        "message": public_reply_text
                    }))
                    .send()
                    .await;

                let reply_status = match resp {
                    Ok(r) if r.status().is_success() => "success",
                    _ => "failed",
                };
                let _ = conn.execute(
                    "UPDATE facebook_comment_logs SET public_reply_status = ?1, updated_at = datetime('now') WHERE id = ?2",
                    params![reply_status, log_id],
                );
            } else {
                let _ = conn.execute(
                    "UPDATE facebook_comment_logs SET public_reply_status = 'none', updated_at = datetime('now') WHERE id = ?1",
                    params![log_id],
                );
            }
        } else {
            let _ = conn.execute(
                "UPDATE facebook_comment_logs SET public_reply_status = 'none', updated_at = datetime('now') WHERE id = ?1",
                params![log_id],
            );
        }

        // 7. Execute 1-reply Private Send API
        if let Some(ref template) = rule.private_reply_template {
            if !template.trim().is_empty() {
                let private_reply_text = Self::render_template(template, params.sender_name);
                let send_url = format!(
                    "https://graph.facebook.com/v26.0/{}/messages",
                    params.page_id
                );
                let resp = self
                    .client
                    .post(&send_url)
                    .header("Authorization", format!("Bearer {}", params.page_token))
                    .json(&json!({
                        "recipient": {
                            "comment_id": params.comment_id
                        },
                        "message": {
                            "text": private_reply_text
                        }
                    }))
                    .send()
                    .await;

                match resp {
                    Ok(r) => {
                        let status_code = r.status();
                        if status_code.is_success() {
                            let _ = conn.execute(
                                "UPDATE facebook_comment_logs SET private_reply_status = 'success', updated_at = datetime('now') WHERE id = ?1",
                                params![log_id],
                            );
                        } else if status_code.is_client_error() && status_code.as_u16() != 429 {
                            // Permanent client rejection (e.g. 1-reply limit reached, window expired, blocked)
                            let err_body: Value = r.json().await.unwrap_or_default();
                            let err_msg = err_body
                                .get("error")
                                .and_then(|e| e.get("message"))
                                .and_then(|m| m.as_str())
                                .unwrap_or("Client rejection on private reply");
                            let _ = conn.execute(
                                "UPDATE facebook_comment_logs SET private_reply_status = 'failed', error_message = ?1, updated_at = datetime('now') WHERE id = ?2",
                                params![err_msg, log_id],
                            );
                        } else {
                            // Transient 5xx or rate limit: mark delivery_unknown for reconciliation
                            let _ = conn.execute(
                                "UPDATE facebook_comment_logs SET private_reply_status = 'delivery_unknown', updated_at = datetime('now') WHERE id = ?1",
                                params![log_id],
                            );
                        }
                    }
                    Err(e) => {
                        // Network error: mark delivery_unknown
                        let _ = conn.execute(
                            "UPDATE facebook_comment_logs SET private_reply_status = 'delivery_unknown', error_message = ?1, updated_at = datetime('now') WHERE id = ?2",
                            params![e.to_string(), log_id],
                        );
                    }
                }
            } else {
                let _ = conn.execute(
                    "UPDATE facebook_comment_logs SET private_reply_status = 'none', updated_at = datetime('now') WHERE id = ?1",
                    params![log_id],
                );
            }
        } else {
            let _ = conn.execute(
                "UPDATE facebook_comment_logs SET private_reply_status = 'none', updated_at = datetime('now') WHERE id = ?1",
                params![log_id],
            );
        }

        Ok(Some(log_id))
    }

    pub async fn reconcile_comment_private_replies(
        &self,
        conn: &Connection,
        page_id: &str,
        page_token: &str,
    ) -> Result<usize> {
        let worker_owner = format!("worker_{}", uuid::Uuid::new_v4());

        // Claim delivery_unknown rows with 5-minute lease
        let claimed_rows = conn.execute(
            "UPDATE facebook_comment_logs
             SET owner_id = ?1,
                 claim_deadline = datetime('now', '+5 minutes'),
                 updated_at = datetime('now')
             WHERE page_id = ?2 AND private_reply_status = 'delivery_unknown' AND (owner_id IS NULL OR claim_deadline < datetime('now'))",
            params![worker_owner, page_id],
        )?;

        if claimed_rows == 0 {
            return Ok(0);
        }

        let mut stmt = conn.prepare(
            "SELECT id, comment_id
             FROM facebook_comment_logs
             WHERE page_id = ?1 AND owner_id = ?2 AND private_reply_status = 'delivery_unknown'",
        )?;

        let claimed_items: Vec<(i64, String)> = stmt
            .query_map(params![page_id, worker_owner], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut resolved = 0;
        for (log_id, comment_id) in claimed_items {
            let query_url = format!(
                "https://graph.facebook.com/v26.0/{}?fields=id,private_reply_status",
                comment_id
            );
            let resp = self
                .client
                .get(&query_url)
                .header("Authorization", format!("Bearer {}", page_token))
                .send()
                .await;

            if let Ok(r) = resp {
                if r.status().is_success() {
                    let body: Value = r.json().await.unwrap_or_default();
                    let reply_status = body
                        .get("private_reply_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let final_status = match reply_status {
                        "replied" => "success",
                        "can_reply" => "failed",
                        "cannot_reply" => "failed",
                        _ => "delivery_unknown",
                    };

                    if final_status != "delivery_unknown" {
                        let _ = conn.execute(
                            "UPDATE facebook_comment_logs
                             SET private_reply_status = ?1,
                                 owner_id = NULL,
                                 claim_deadline = NULL,
                                 updated_at = datetime('now')
                             WHERE id = ?2 AND owner_id = ?3",
                            params![final_status, log_id, worker_owner],
                        );
                        resolved += 1;
                        continue;
                    }
                }
            }

            // Inconclusive: release lease so future reconciliation cycles can retry
            let _ = conn.execute(
                "UPDATE facebook_comment_logs
                 SET owner_id = NULL,
                     claim_deadline = NULL,
                     updated_at = datetime('now')
                 WHERE id = ?1 AND owner_id = ?2",
                params![log_id, worker_owner],
            );
        }

        Ok(resolved)
    }

    fn render_template(template: &str, sender_name: Option<&str>) -> String {
        let name = sender_name.unwrap_or("bạn");
        template.replace("{{name}}", name).replace("{{user}}", name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_keywords_modes() {
        let text = "Xin chào shop giá bao nhiêu vậy ạ?";

        // contains
        assert!(FacebookAutomationEngine::matches_keywords(
            text,
            &["giá".to_string(), "tư vấn".to_string()],
            "contains"
        ));
        assert!(!FacebookAutomationEngine::matches_keywords(
            text,
            &["địa chỉ".to_string(), "hotline".to_string()],
            "contains"
        ));

        // all
        assert!(FacebookAutomationEngine::matches_keywords(
            text,
            &["giá".to_string(), "shop".to_string()],
            "all"
        ));
        assert!(!FacebookAutomationEngine::matches_keywords(
            text,
            &["giá".to_string(), "free ship".to_string()],
            "all"
        ));

        // exact
        assert!(FacebookAutomationEngine::matches_keywords(
            "inbox",
            &["inbox".to_string(), "pm".to_string()],
            "exact"
        ));
        assert!(!FacebookAutomationEngine::matches_keywords(
            text,
            &["giá".to_string()],
            "exact"
        ));

        // regex
        assert!(FacebookAutomationEngine::matches_keywords(
            text,
            &[r"giá\s+bao\s+nhiêu".to_string()],
            "regex"
        ));
        assert!(!FacebookAutomationEngine::matches_keywords(
            text,
            &[r"^\d{10}$".to_string()],
            "regex"
        ));
    }

    #[test]
    fn test_render_template_placeholders() {
        let rendered = FacebookAutomationEngine::render_template(
            "Chào {{name}}, shop đã gửi báo giá qua tin nhắn cho bạn!",
            Some("Minh Anh"),
        );
        assert_eq!(
            rendered,
            "Chào Minh Anh, shop đã gửi báo giá qua tin nhắn cho bạn!"
        );

        let rendered_fallback =
            FacebookAutomationEngine::render_template("Chào {{user}}, shop đã gửi tin nhắn!", None);
        assert_eq!(rendered_fallback, "Chào bạn, shop đã gửi tin nhắn!");
    }
}
