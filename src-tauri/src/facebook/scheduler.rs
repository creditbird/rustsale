use anyhow::{anyhow, Result};
use reqwest::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::facebook::publisher::FacebookPublisher;
use crate::facebook::uploader::FacebookUploader;
use crate::facebook::vault::FacebookTokenVault;

const MAX_UPLOAD_RETRIES: i64 = 3;
const GRAPH_PAGE_LIMIT: usize = 10;

#[derive(Debug, Default, Clone)]
pub struct PromotionSummary {
    pub attempted: usize,
    pub published: usize,
    pub scheduled_on_meta: usize,
    pub failed: usize,
    pub deferred: usize,
}

#[derive(Debug)]
#[allow(dead_code)]
struct ClaimedUploadSession {
    id: i64,
    post_id: i64,
    upload_session_id: String,
    upload_type: String,
    page_id: String,
    video_id: Option<String>,
    file_path: String,
    file_hash: String,
    file_size_bytes: i64,
    start_offset: i64,
    status: String,
    retry_count: i64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ParentPostContext {
    channel_account_id: i64,
    page_id: String,
    message: String,
    scheduled_publish_time: Option<String>,
    scheduled_epoch: Option<i64>,
}

pub struct FacebookScheduler {
    client: Client,
    publisher: FacebookPublisher,
}

impl Default for FacebookScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl FacebookScheduler {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
            publisher: FacebookPublisher::new(),
        }
    }

    // ------------------------------------------------------------------
    // Promotion Worker: claims rows with status='queued'
    // ------------------------------------------------------------------

    pub async fn run_promotion_cycle(
        &self,
        conn: &Connection,
        max_posts: usize,
    ) -> Result<PromotionSummary> {
        let mut summary = PromotionSummary::default();

        let candidate_ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM facebook_scheduled_posts
                 WHERE status = 'queued'
                   AND owner_id IS NULL
                   AND (next_retry_at IS NULL OR datetime(next_retry_at) <= datetime('now'))
                 ORDER BY id
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map([max_posts as i64], |r| r.get::<_, i64>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        for post_id in candidate_ids {
            summary.attempted += 1;

            let (channel_account_id, page_id, scheduled_publish_time): (
                i64,
                String,
                Option<String>,
            ) = conn.query_row(
                "SELECT channel_account_id, page_id, scheduled_publish_time
                 FROM facebook_scheduled_posts WHERE id = ?1",
                [post_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;

            // Strict RFC3339 parsing, failing closed with error_code='invalid_timestamp'
            let parsed_epoch: Option<i64> = if let Some(ts) = scheduled_publish_time.as_deref() {
                match chrono::DateTime::parse_from_rfc3339(ts) {
                    Ok(dt) => Some(dt.timestamp()),
                    Err(_) => {
                        let rows = conn.execute(
                            "UPDATE facebook_scheduled_posts
                             SET status = 'failed',
                                 error_code = 'invalid_timestamp',
                                 error_message = ?1,
                                 owner_id = NULL,
                                 claim_deadline = NULL,
                                 updated_at = datetime('now')
                             WHERE id = ?2 AND status = 'queued'",
                            params![
                                format!("Invalid RFC3339 scheduled_publish_time '{}'", ts),
                                post_id
                            ],
                        )?;
                        if rows == 1 {
                            summary.failed += 1;
                        } else {
                            summary.deferred += 1;
                        }
                        continue;
                    }
                }
            } else {
                None
            };

            // Meta scheduling bounds:
            // Meta Graph API permits scheduling between 10 minutes (600s) and 30 days (30 * 86400s).
            let now_epoch = chrono::Utc::now().timestamp();
            const META_MAX_SCHEDULE_SECS: i64 = 30 * 86400; // 30 days
            const META_MIN_SCHEDULE_SECS: i64 = 600; // 10 minutes

            if let Some(target_epoch) = parsed_epoch {
                let diff = target_epoch - now_epoch;
                if diff > META_MAX_SCHEDULE_SECS {
                    // >30 days: wait locally in queued status
                    summary.deferred += 1;
                    continue;
                } else if diff > 0 && diff < META_MIN_SCHEDULE_SECS {
                    // Within 10 minutes: wait in queued status until exact target timestamp arrives (diff <= 0)
                    summary.deferred += 1;
                    continue;
                } else if diff < -86400 {
                    // Overdue by >24 hours
                    let _ = conn.execute(
                        "UPDATE facebook_scheduled_posts
                         SET status = 'overdue_prompt',
                             error_code = 'overdue_schedule',
                             error_message = 'Post is overdue by more than 24 hours',
                             owner_id = NULL,
                             claim_deadline = NULL,
                             updated_at = datetime('now')
                         WHERE id = ?1 AND status = 'queued'",
                        params![post_id],
                    );
                    summary.failed += 1;
                    continue;
                }
            }

            // Query facebook_page_tokens for this claimed row's channel_account_id + page_id
            let has_valid_token: Option<i64> = conn
                .query_row(
                    "SELECT 1 FROM facebook_page_tokens
                     WHERE channel_account_id = ?1 AND page_id = ?2 AND token_status = 'valid'",
                    params![channel_account_id, page_id],
                    |r| r.get(0),
                )
                .optional()?;
            if has_valid_token.is_none() {
                let rows = conn.execute(
                    "UPDATE facebook_scheduled_posts
                     SET status = 'failed',
                         error_code = 'token_vault_error',
                         error_message = ?1,
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?2 AND status = 'queued'",
                    params![
                        format!(
                            "No valid Page token for account {} / page {}",
                            channel_account_id, page_id
                        ),
                        post_id
                    ],
                )?;
                if rows == 1 {
                    summary.failed += 1;
                } else {
                    summary.deferred += 1;
                }
                continue;
            }

            match self.publisher.publish_post(conn, post_id).await {
                Ok(outcome) => match outcome {
                    crate::facebook::publisher::PublishOutcome::Published { .. } => {
                        summary.published += 1;
                    }
                    crate::facebook::publisher::PublishOutcome::ScheduledOnMeta { .. } => {
                        summary.scheduled_on_meta += 1;
                    }
                    crate::facebook::publisher::PublishOutcome::StillProcessing { .. } => {
                        summary.deferred += 1;
                    }
                },
                Err(err) => {
                    let status: Option<String> = conn
                        .query_row(
                            "SELECT status FROM facebook_scheduled_posts WHERE id = ?1",
                            [post_id],
                            |r| r.get(0),
                        )
                        .optional()?;
                    match status.as_deref() {
                        Some("failed") => summary.failed += 1,
                        Some("published") => summary.published += 1,
                        Some("scheduled_on_meta") => summary.scheduled_on_meta += 1,
                        Some("queued") | Some("delivery_unknown") | None => summary.deferred += 1,
                        _ => summary.deferred += 1,
                    }
                    tracing::warn!(
                        target: "facebook_scheduler",
                        event = "promotion_error",
                        post_id,
                        error = %err,
                    );
                }
            }
        }

        Ok(summary)
    }

    // ------------------------------------------------------------------
    // Reconciliation Engine: claims rows with status='delivery_unknown'
    // ------------------------------------------------------------------

    #[allow(clippy::type_complexity)]
    pub async fn run_reconciliation_cycle(
        &self,
        conn: &Connection,
        max_posts: usize,
    ) -> Result<usize> {
        let mut resolved = 0usize;

        for _ in 0..max_posts {
            let worker_owner =
                format!("reconciler_{}_{}", std::process::id(), uuid::Uuid::new_v4());
            let claimed: Option<(
                i64,
                i64,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                String,
            )> = conn
                .query_row(
                    "UPDATE facebook_scheduled_posts
                     SET owner_id = ?1,
                         claim_deadline = datetime('now', '+5 minutes'),
                         updated_at = datetime('now')
                     WHERE id = (
                         SELECT id FROM facebook_scheduled_posts
                         WHERE status = 'delivery_unknown'
                           AND (claim_deadline IS NULL OR claim_deadline < datetime('now'))
                         ORDER BY id LIMIT 1
                     )
                     AND status = 'delivery_unknown'
                     RETURNING id, channel_account_id, page_id, post_type, message, remote_post_id, scheduled_publish_time, created_at",
                    params![worker_owner],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                            r.get(6)?,
                            r.get(7)?,
                        ))
                    },
                )
                .optional()?;

            let (
                post_id,
                channel_account_id,
                page_id,
                post_type,
                message,
                remote_post_id,
                scheduled_publish_time,
                created_at_str,
            ) = match claimed {
                Some(row) => row,
                None => break,
            };

            let page_token = match FacebookTokenVault::get_valid_page_token(
                conn,
                channel_account_id,
                &page_id,
            ) {
                Ok(t) => t,
                Err(err) => {
                    tracing::warn!(
                        target: "facebook_scheduler",
                        event = "reconciliation_token_error",
                        post_id,
                        error = %err,
                    );
                    Self::release_post_lease(conn, post_id, &worker_owner)?;
                    continue;
                }
            };

            let reference_epoch: Option<i64> = scheduled_publish_time
                .as_deref()
                .and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .ok()
                        .map(|dt| dt.timestamp())
                })
                .or_else(|| {
                    chrono::DateTime::parse_from_rfc3339(&created_at_str)
                        .ok()
                        .map(|dt| dt.timestamp())
                        .or_else(|| {
                            chrono::NaiveDateTime::parse_from_str(
                                &created_at_str,
                                "%Y-%m-%d %H:%M:%S",
                            )
                            .ok()
                            .map(|ndt| ndt.and_utc().timestamp())
                        })
                });

            let mut matched_status: Option<(&str, String)> = None;

            if let Some(remote_id) = self
                .find_feed_match(&page_id, &page_token, &post_type, &message, reference_epoch)
                .await?
            {
                matched_status = Some(("published", remote_id));
            } else if let Some(remote_id) = self
                .find_scheduled_match(
                    &page_id,
                    &page_token,
                    remote_post_id.as_deref(),
                    &post_type,
                    &message,
                    reference_epoch,
                )
                .await?
            {
                matched_status = Some(("scheduled_on_meta", remote_id));
            }

            match matched_status {
                Some((target_status, remote_id)) => {
                    let rows = conn.execute(
                        "UPDATE facebook_scheduled_posts
                         SET status = ?1,
                             remote_post_id = ?2,
                             error_code = NULL,
                             error_message = NULL,
                             owner_id = NULL,
                             claim_deadline = NULL,
                             updated_at = datetime('now')
                         WHERE id = ?3 AND owner_id = ?4 AND status = 'delivery_unknown'",
                        params![target_status, remote_id, post_id, worker_owner],
                    )?;
                    if rows == 1 {
                        resolved += 1;
                    }
                }
                None => {
                    Self::release_post_lease(conn, post_id, &worker_owner)?;
                }
            }
        }

        Ok(resolved)
    }

    fn release_post_lease(conn: &Connection, post_id: i64, owner_id: &str) -> Result<()> {
        conn.execute(
            "UPDATE facebook_scheduled_posts
             SET owner_id = NULL, claim_deadline = NULL, updated_at = datetime('now')
             WHERE id = ?1 AND owner_id = ?2",
            params![post_id, owner_id],
        )?;
        Ok(())
    }

    async fn find_feed_match(
        &self,
        page_id: &str,
        page_token: &str,
        post_type: &str,
        message: &str,
        reference_epoch: Option<i64>,
    ) -> Result<Option<String>> {
        let auth_header = format!("Bearer {}", page_token);
        let mut next_url: Option<String> = Some(format!(
            "https://graph.facebook.com/v26.0/{}/feed?fields=id,status_type,message,created_time&limit=50",
            page_id
        ));
        let mut pages = 0usize;
        let mut matches = Vec::new();
        while let Some(url) = next_url {
            pages += 1;
            if pages > GRAPH_PAGE_LIMIT {
                break;
            }
            let resp = self
                .client
                .get(&url)
                .header("Authorization", &auth_header)
                .send()
                .await?;
            if !resp.status().is_success() {
                return Err(anyhow!("Graph /feed query returned HTTP {}", resp.status()));
            }
            let body: Value = resp.json().await?;
            let data = body
                .get("data")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            for item in &data {
                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if id.is_empty() {
                    continue;
                }
                let status_type = item
                    .get("status_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !Self::post_type_matches(post_type, status_type) {
                    continue;
                }
                let graph_message = item.get("message").and_then(|v| v.as_str()).unwrap_or("");
                if Self::message_hash(graph_message) != Self::message_hash(message) {
                    continue;
                }
                if !Self::timestamp_within_window(
                    item.get("created_time").and_then(|v| v.as_str()),
                    reference_epoch,
                ) {
                    continue;
                }
                matches.push(Self::strip_page_prefix(id, page_id));
            }
            next_url = body
                .get("paging")
                .and_then(|p| p.get("next"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());
        }
        if matches.len() > 1 {
            return Err(anyhow!(
                "Ambiguous match: multiple feed posts matched candidate criteria"
            ));
        } else if matches.len() == 1 {
            return Ok(Some(matches.remove(0)));
        }
        Ok(None)
    }

    async fn find_scheduled_match(
        &self,
        page_id: &str,
        page_token: &str,
        remote_post_id: Option<&str>,
        post_type: &str,
        message: &str,
        reference_epoch: Option<i64>,
    ) -> Result<Option<String>> {
        let auth_header = format!("Bearer {}", page_token);
        let mut next_url: Option<String> = Some(format!(
            "https://graph.facebook.com/v26.0/{}/scheduled_posts?fields=id,scheduled_publish_time,message,status_type&limit=50",
            page_id
        ));
        let mut pages = 0usize;
        let mut matches = Vec::new();
        while let Some(url) = next_url {
            pages += 1;
            if pages > GRAPH_PAGE_LIMIT {
                break;
            }
            let resp = self
                .client
                .get(&url)
                .header("Authorization", &auth_header)
                .send()
                .await?;
            if !resp.status().is_success() {
                return Err(anyhow!(
                    "Graph /scheduled_posts query returned HTTP {}",
                    resp.status()
                ));
            }
            let body: Value = resp.json().await?;
            let data = body
                .get("data")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            for item in &data {
                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if id.is_empty() {
                    continue;
                }
                let bare_id = Self::strip_page_prefix(id, page_id);
                if let Some(rp) = remote_post_id {
                    if rp == id || rp == bare_id {
                        return Ok(Some(bare_id));
                    }
                }
                let graph_message = item.get("message").and_then(|v| v.as_str()).unwrap_or("");
                let status_type = item
                    .get("status_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let time_ok = Self::timestamp_within_window(
                    item.get("scheduled_publish_time").and_then(|v| v.as_str()),
                    reference_epoch,
                );
                let type_ok =
                    !status_type.is_empty() && Self::post_type_matches(post_type, status_type);
                let msg_ok = !graph_message.is_empty()
                    && Self::message_hash(graph_message) == Self::message_hash(message);
                if time_ok && type_ok && msg_ok {
                    matches.push(bare_id);
                }
            }
            next_url = body
                .get("paging")
                .and_then(|p| p.get("next"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());
        }
        if matches.len() > 1 {
            return Err(anyhow!(
                "Ambiguous match: multiple scheduled posts matched candidate criteria"
            ));
        } else if matches.len() == 1 {
            return Ok(Some(matches.remove(0)));
        }
        Ok(None)
    }

    fn strip_page_prefix(id: &str, page_id: &str) -> String {
        let prefix = format!("{}_", page_id);
        if let Some(stripped) = id.strip_prefix(&prefix) {
            stripped.to_string()
        } else {
            id.to_string()
        }
    }

    fn message_hash(msg: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(msg.trim().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn timestamp_within_window(graph_time_str: Option<&str>, reference_epoch: Option<i64>) -> bool {
        let reference = match reference_epoch {
            Some(epoch) => epoch,
            None => return false,
        };
        let graph_ts = match graph_time_str {
            Some(s) => match chrono::DateTime::parse_from_rfc3339(s) {
                Ok(dt) => dt.timestamp(),
                Err(_) => match s.parse::<i64>() {
                    Ok(epoch) => epoch,
                    Err(_) => return false,
                },
            },
            None => return false,
        };
        (graph_ts - reference).abs() <= 60
    }

    fn post_type_matches(post_type: &str, status_type: &str) -> bool {
        match post_type {
            "status" => matches!(
                status_type,
                "status" | "mobile_status_update" | "created_note"
            ),
            "link" => matches!(status_type, "link" | "shared_story"),
            "photo" | "multi_photo" => matches!(status_type, "added_photos" | "shared_story"),
            "video" | "reel" => matches!(status_type, "added_video"),
            _ => false,
        }
    }

    // ------------------------------------------------------------------
    // Upload Session Background Reconciler
    // ------------------------------------------------------------------

    #[allow(clippy::type_complexity)]
    pub async fn reconcile_upload_sessions(
        &self,
        conn: &mut Connection,
        max_sessions: usize,
    ) -> Result<usize> {
        let mut resolved = 0usize;

        for _ in 0..max_sessions {
            let worker_owner = format!(
                "upload_reconciler_{}_{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            );

            // ONE consistent retry-clock rule shared with FacebookPublisher: the session's own
            // lease must be free AND its parent post's next_retry_at backoff must have elapsed.
            let claimed: Option<(i64, i64, String, String, String, Option<String>, String, String, i64, i64, String, i64)> =
                conn.query_row(
                    "UPDATE facebook_upload_sessions
                     SET owner_id = ?1,
                         lease_deadline = datetime('now', '+5 minutes'),
                         updated_at = datetime('now')
                     WHERE id = (
                         SELECT s.id FROM facebook_upload_sessions s
                         JOIN facebook_scheduled_posts p ON p.id = s.post_id
                         WHERE s.status IN ('init_pending', 'in_progress', 'finishing')
                           AND (s.lease_deadline IS NULL OR s.lease_deadline < datetime('now'))
                           AND (p.next_retry_at IS NULL OR p.next_retry_at <= datetime('now'))
                         LIMIT 1
                     )
                     RETURNING id, post_id, upload_session_id, upload_type, page_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count",
                    params![worker_owner],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                            r.get(6)?,
                            r.get(7)?,
                            r.get(8)?,
                            r.get(9)?,
                            r.get(10)?,
                            r.get(11)?,
                        ))
                    },
                )
                .optional()?;

            let session = match claimed {
                Some((
                    id,
                    post_id,
                    upload_session_id,
                    upload_type,
                    page_id,
                    video_id,
                    file_path,
                    file_hash,
                    file_size_bytes,
                    start_offset,
                    status,
                    retry_count,
                )) => ClaimedUploadSession {
                    id,
                    post_id,
                    upload_session_id,
                    upload_type,
                    page_id,
                    video_id,
                    file_path,
                    file_hash,
                    file_size_bytes,
                    start_offset,
                    status,
                    retry_count,
                },
                None => break,
            };

            let parent = match Self::load_parent_context(conn, session.post_id) {
                Ok(p) => p,
                Err(err) => {
                    tracing::error!(
                        target: "facebook_scheduler",
                        event = "reconciler_parent_load_failed",
                        session_id = session.id,
                        error = %err,
                    );
                    Self::release_session_lease(conn, session.id, &worker_owner)?;
                    continue;
                }
            };

            let outcome: Result<()> = match session.status.as_str() {
                "init_pending" => {
                    self.handle_init_pending(conn, &session, &parent, &worker_owner)
                        .await
                }
                "in_progress" => {
                    self.handle_in_progress(conn, &session, &parent, &worker_owner)
                        .await
                }
                "finishing" => {
                    self.handle_finishing(conn, &session, &parent, &worker_owner)
                        .await
                }
                other => Err(anyhow!("Unexpected session status '{}'", other)),
            };

            match outcome {
                Ok(()) => {
                    resolved += 1;
                }
                Err(err) => {
                    tracing::warn!(
                        target: "facebook_scheduler",
                        event = "upload_reconciler_deferred",
                        session_id = session.id,
                        status = %session.status,
                        error = %err,
                    );
                    if session.retry_count + 1 >= MAX_UPLOAD_RETRIES {
                        let _ = Self::fail_session_and_parent_tx(
                            conn,
                            session.id,
                            session.post_id,
                            &worker_owner,
                            &session.status,
                            "max_retries_exceeded",
                            &format!("Upload reconciler exceeded max retries: {}", err),
                        );
                    } else {
                        let backoff_secs =
                            (2u64.pow(session.retry_count.min(6) as u32) * 15) as i64;
                        let _ = conn.execute(
                            "UPDATE facebook_upload_sessions
                             SET owner_id = NULL,
                                 retry_count = retry_count + 1,
                                 lease_deadline = datetime('now', '+' || ?1 || ' seconds'),
                                 updated_at = datetime('now')
                             WHERE id = ?2 AND owner_id = ?3",
                            params![backoff_secs, session.id, &worker_owner],
                        );
                    }
                }
            }
        }

        Ok(resolved)
    }

    fn load_parent_context(conn: &Connection, post_id: i64) -> Result<ParentPostContext> {
        let (channel_account_id, page_id, message, scheduled_publish_time): (
            i64,
            String,
            String,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT channel_account_id, page_id, message, scheduled_publish_time
             FROM facebook_scheduled_posts WHERE id = ?1",
                [post_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|e| anyhow!("Parent post {} not found: {}", post_id, e))?;
        let scheduled_epoch = scheduled_publish_time
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp());
        Ok(ParentPostContext {
            channel_account_id,
            page_id,
            message,
            scheduled_publish_time,
            scheduled_epoch,
        })
    }

    fn release_session_lease(conn: &Connection, session_id: i64, owner_id: &str) -> Result<()> {
        conn.execute(
            "UPDATE facebook_upload_sessions
             SET owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now')
             WHERE id = ?1 AND owner_id = ?2",
            params![session_id, owner_id],
        )?;
        Ok(())
    }

    fn fail_session_and_parent_tx(
        conn: &Connection,
        session_id: i64,
        post_id: i64,
        owner_id: &str,
        from_status: &str,
        error_code: &str,
        error_message: &str,
    ) -> Result<()> {
        conn.execute_batch("SAVEPOINT fb_fail_tx;")?;
        let rows_sess = conn.execute(
            "UPDATE facebook_upload_sessions
             SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now')
             WHERE id = ?1 AND owner_id = ?2 AND status = ?3",
            params![session_id, owner_id, from_status],
        );
        let rows_post = conn.execute(
            "UPDATE facebook_scheduled_posts
             SET status = 'failed',
                 error_code = ?1,
                 error_message = ?2,
                 owner_id = NULL,
                 claim_deadline = NULL,
                 updated_at = datetime('now')
             WHERE id = ?3 AND status IN ('publishing', 'delivery_unknown')",
            params![error_code, error_message, post_id],
        );
        match (rows_sess, rows_post) {
            (Ok(1), Ok(_)) => {
                conn.execute_batch("RELEASE SAVEPOINT fb_fail_tx;")?;
                Ok(())
            }
            (Ok(_), Ok(_)) => {
                let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT fb_fail_tx;");
                let _ = conn.execute_batch("RELEASE SAVEPOINT fb_fail_tx;");
                Err(anyhow!(
                    "Upload session lease lost for session {}",
                    session_id
                ))
            }
            (Err(e), _) | (_, Err(e)) => {
                let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT fb_fail_tx;");
                let _ = conn.execute_batch("RELEASE SAVEPOINT fb_fail_tx;");
                Err(anyhow!(
                    "Transaction error during failure propagation: {}",
                    e
                ))
            }
        }
    }

    async fn resolve_parent_token(
        &self,
        conn: &Connection,
        parent: &ParentPostContext,
    ) -> Result<String> {
        FacebookTokenVault::get_valid_page_token(conn, parent.channel_account_id, &parent.page_id)
    }

    // ---------------- init_pending ----------------

    async fn handle_init_pending(
        &self,
        conn: &Connection,
        sess: &ClaimedUploadSession,
        parent: &ParentPostContext,
        owner: &str,
    ) -> Result<()> {
        // Max retries reached -> fail closed (session + parent post)
        if sess.retry_count >= MAX_UPLOAD_RETRIES {
            Self::fail_session_and_parent_tx(
                conn,
                sess.id,
                sess.post_id,
                owner,
                "init_pending",
                "max_retries_exceeded",
                "Upload session exceeded maximum retry limit during START recovery",
            )?;
            return Ok(());
        }

        let page_token = self.resolve_parent_token(conn, parent).await?;

        // An uncommitted START container holds zero byte chunks and is never published;
        // execute a clean START against Meta Graph API to establish an authoritative container.
        let (start_url, form_pairs): (String, Vec<(&str, String)>) = match sess.upload_type.as_str()
        {
            "video" => (
                format!(
                    "https://graph-video.facebook.com/v26.0/{}/videos",
                    sess.page_id
                ),
                vec![
                    ("access_token", page_token.clone()),
                    ("upload_phase", "start".to_string()),
                    ("file_size", sess.file_size_bytes.to_string()),
                ],
            ),
            "reel" => (
                format!(
                    "https://graph.facebook.com/v26.0/{}/video_reels",
                    sess.page_id
                ),
                vec![
                    ("access_token", page_token.clone()),
                    ("upload_phase", "start".to_string()),
                ],
            ),
            other => return Err(anyhow!("Unknown upload_type '{}'", other)),
        };

        let form: Vec<(&str, &str)> = form_pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let resp = match self.client.post(&start_url).form(&form).send().await {
            Ok(r) => r,
            Err(err) => {
                return Err(anyhow!("Network error on fresh START: {}", err));
            }
        };

        let status_code = resp.status();
        if status_code.as_u16() == 429 {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            // Identical session-level backoff as the foreground uploader
            conn.execute(
                "UPDATE facebook_upload_sessions
                 SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now')
                 WHERE id = ?1 AND owner_id = ?2 AND status = 'init_pending'",
                params![sess.id, owner, retry_after as i64],
            )?;
            return Ok(());
        }
        if status_code.is_client_error() {
            let err_text = resp.text().await.unwrap_or_default();
            Self::fail_session_and_parent_tx(
                conn,
                sess.id,
                sess.post_id,
                owner,
                "init_pending",
                &format!("http_{}", status_code.as_u16()),
                &format!(
                    "Meta rejected fresh START (HTTP {}): {}",
                    status_code.as_u16(),
                    err_text
                ),
            )?;
            return Ok(());
        }
        if !status_code.is_success() {
            return Err(anyhow!(
                "Meta server error on fresh START (HTTP {})",
                status_code.as_u16()
            ));
        }

        let start_json: Value = resp
            .json()
            .await
            .map_err(|e| anyhow!("Invalid JSON in fresh START response: {}", e))?;

        let new_video_id = start_json
            .get("video_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Missing video_id in fresh START response"))?;
        let new_session_id = match sess.upload_type.as_str() {
            "video" => start_json
                .get("upload_session_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("Missing upload_session_id in fresh START response"))?,
            _ => new_video_id.clone(),
        };

        // Atomic CAS transition init_pending -> in_progress
        let rows = conn.execute(
            "UPDATE facebook_upload_sessions
             SET upload_session_id = ?1, video_id = ?2, status = 'in_progress', updated_at = datetime('now')
             WHERE id = ?3 AND owner_id = ?4 AND status = 'init_pending'",
            params![new_session_id, new_video_id, sess.id, owner],
        )?;
        if rows != 1 {
            return Err(anyhow!(
                "Lost lease on init_pending session {} during START recovery CAS",
                sess.id
            ));
        }

        Ok(())
    }

    // ---------------- in_progress ----------------

    async fn handle_in_progress(
        &self,
        conn: &Connection,
        sess: &ClaimedUploadSession,
        parent: &ParentPostContext,
        owner: &str,
    ) -> Result<()> {
        let page_token = self.resolve_parent_token(conn, parent).await?;
        let video_id = sess
            .video_id
            .clone()
            .ok_or_else(|| anyhow!("in_progress session missing authoritative video_id"))?;

        if sess.upload_type == "video" {
            self.resume_video_chunks(conn, sess, parent, owner, &page_token, &video_id)
                .await
        } else {
            self.resume_reel_stream(conn, sess, parent, owner, &page_token, &video_id)
                .await
        }
    }

    async fn resume_video_chunks(
        &self,
        conn: &Connection,
        sess: &ClaimedUploadSession,
        parent: &ParentPostContext,
        owner: &str,
        page_token: &str,
        video_id: &str,
    ) -> Result<()> {
        // Authoritative server offset
        let status_url = format!(
            "https://graph-video.facebook.com/v26.0/{}?fields=start_offset,end_offset&access_token={}",
            sess.upload_session_id, page_token
        );
        let resp = self
            .client
            .get(&status_url)
            .send()
            .await
            .map_err(|e| anyhow!("Network error querying server offset: {}", e))?;
        let status_code = resp.status();
        if status_code.as_u16() == 429 {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            Self::record_session_backoff_with_status(
                conn,
                sess.id,
                owner,
                retry_after,
                "in_progress",
            )?;
            return Ok(());
        }
        if status_code.is_client_error() {
            Self::fail_session_and_parent_tx(
                conn,
                sess.id,
                sess.post_id,
                owner,
                "in_progress",
                "server_offset_client_error",
                &format!(
                    "Meta rejected server offset query with HTTP {}",
                    status_code.as_u16()
                ),
            )?;
            return Ok(());
        }
        if !status_code.is_success() {
            return Err(anyhow!(
                "Meta server error querying server offset (HTTP {})",
                status_code.as_u16()
            ));
        }
        let body: Value = resp
            .json()
            .await
            .map_err(|e| anyhow!("Invalid JSON in server offset response: {}", e))?;
        let server_offset = Self::parse_json_u64(&body, "start_offset")
            .ok_or_else(|| anyhow!("Missing start_offset in server offset response"))?;
        let file_size = sess.file_size_bytes as u64;
        if server_offset > file_size {
            Self::fail_session_and_parent_tx(
                conn,
                sess.id,
                sess.post_id,
                owner,
                "in_progress",
                "invalid_server_offset",
                &format!(
                    "Server offset {} exceeds file size {}",
                    server_offset, file_size
                ),
            )?;
            return Err(anyhow!(
                "Server offset {} exceeds file size {}",
                server_offset,
                file_size
            ));
        }

        let mut offset = server_offset;
        conn.execute(
            "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND owner_id = ?3 AND status = 'in_progress'",
            params![offset as i64, sess.id, owner],
        )?;

        let transfer_url = format!(
            "https://graph-video.facebook.com/v26.0/{}/videos",
            sess.page_id
        );
        let mut file = tokio::fs::File::open(&sess.file_path)
            .await
            .map_err(|e| anyhow!("Failed to open video file '{}': {}", sess.file_path, e))?;
        let chunk_size: u64 = 4 * 1024 * 1024;

        while offset < file_size {
            // Heartbeat lease renewal
            let hb = conn.execute(
                "UPDATE facebook_upload_sessions SET lease_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
                 WHERE id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                params![sess.id, owner],
            )?;
            if hb != 1 {
                return Err(anyhow!("Lost session lease during chunk heartbeat"));
            }

            let bytes_to_read = std::cmp::min(chunk_size, file_size - offset);
            use tokio::io::{AsyncReadExt, AsyncSeekExt};
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|e| anyhow!("Failed to seek video file: {}", e))?;
            let mut buffer = vec![0u8; bytes_to_read as usize];
            file.read_exact(&mut buffer)
                .await
                .map_err(|e| anyhow!("Failed to read video chunk: {}", e))?;

            let part = reqwest::multipart::Part::bytes(buffer).file_name("chunk.mp4");
            let form = reqwest::multipart::Form::new()
                .text("access_token", page_token.to_string())
                .text("upload_phase", "transfer")
                .text("upload_session_id", sess.upload_session_id.clone())
                .text("start_offset", offset.to_string())
                .part("video_file_chunk", part);

            let transfer_resp = self
                .client
                .post(&transfer_url)
                .multipart(form)
                .send()
                .await
                .map_err(|e| anyhow!("Transport error uploading chunk at {}: {}", offset, e))?;
            let transfer_status = transfer_resp.status();
            if transfer_status.as_u16() == 429 {
                let retry_after = transfer_resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                Self::record_session_backoff_with_status(
                    conn,
                    sess.id,
                    owner,
                    retry_after,
                    "in_progress",
                )?;
                return Ok(());
            }
            if transfer_status.is_client_error() {
                Self::fail_session_and_parent_tx(
                    conn,
                    sess.id,
                    sess.post_id,
                    owner,
                    "in_progress",
                    "chunk_upload_client_error",
                    &format!(
                        "Meta rejected chunk upload at offset {} with HTTP {}",
                        offset,
                        transfer_status.as_u16()
                    ),
                )?;
                return Ok(());
            }
            if !transfer_status.is_success() {
                return Err(anyhow!(
                    "Meta server error on chunk transfer at {} (HTTP {})",
                    offset,
                    transfer_status.as_u16()
                ));
            }

            let transfer_json: Value = transfer_resp
                .json()
                .await
                .map_err(|e| anyhow!("Invalid JSON in transfer response: {}", e))?;
            let next_offset = Self::parse_json_u64(&transfer_json, "start_offset")
                .ok_or_else(|| anyhow!("Missing start_offset in transfer response"))?;
            if !(next_offset > offset && next_offset <= file_size) {
                Self::fail_session_and_parent_tx(
                    conn,
                    sess.id,
                    sess.post_id,
                    owner,
                    "in_progress",
                    "invalid_offset_progression",
                    &format!(
                        "Invalid offset progression: prev={}, next={}, file_size={}",
                        offset, next_offset, file_size
                    ),
                )?;
                return Err(anyhow!(
                    "Invalid offset progression: prev={}, next={}, file_size={}",
                    offset,
                    next_offset,
                    file_size
                ));
            }
            offset = next_offset;
            conn.execute(
                "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now')
                 WHERE id = ?2 AND owner_id = ?3 AND status = 'in_progress'",
                params![offset as i64, sess.id, owner],
            )?;
        }

        // Advance to finishing then execute FINISH phase
        let finish_rows = conn.execute(
            "UPDATE facebook_upload_sessions SET status = 'finishing', updated_at = datetime('now')
             WHERE id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
            params![sess.id, owner],
        )?;
        if finish_rows != 1 {
            return Err(anyhow!(
                "Lost lease transitioning video session to finishing"
            ));
        }
        self.execute_video_finish(conn, sess, parent, owner, page_token, video_id)
            .await
    }

    async fn resume_reel_stream(
        &self,
        conn: &Connection,
        sess: &ClaimedUploadSession,
        parent: &ParentPostContext,
        owner: &str,
        page_token: &str,
        video_id: &str,
    ) -> Result<()> {
        // Classify strictly from authoritative Meta status fields
        let status_url = format!(
            "https://graph.facebook.com/v26.0/{}?fields=id,status&access_token={}",
            video_id, page_token
        );
        let resp = self
            .client
            .get(&status_url)
            .send()
            .await
            .map_err(|e| anyhow!("Network error querying Reel status: {}", e))?;
        let status_code = resp.status();
        if status_code.as_u16() == 429 {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            Self::record_session_backoff_with_status(
                conn,
                sess.id,
                owner,
                retry_after,
                "in_progress",
            )?;
            return Ok(());
        }
        if status_code.is_client_error() {
            Self::fail_session_and_parent_tx(
                conn,
                sess.id,
                sess.post_id,
                owner,
                "in_progress",
                "reel_status_client_error",
                &format!(
                    "Meta rejected Reel status query with HTTP {}",
                    status_code.as_u16()
                ),
            )?;
            return Ok(());
        }
        if !status_code.is_success() {
            return Err(anyhow!(
                "Meta server error querying Reel status (HTTP {})",
                status_code.as_u16()
            ));
        }
        let body: Value = resp
            .json()
            .await
            .map_err(|e| anyhow!("Invalid JSON in Reel status response: {}", e))?;
        let status_obj = body.get("status");

        let video_status = status_obj
            .and_then(|s| s.get("video_status"))
            .and_then(|v| v.as_str());
        let uploading_phase = status_obj
            .and_then(|s| s.pointer("/uploading_phase/status"))
            .and_then(|v| v.as_str());
        let publishing_phase = status_obj
            .and_then(|s| s.pointer("/publishing_phase/status"))
            .and_then(|v| v.as_str());

        let has_authoritative_field =
            video_status.is_some() || uploading_phase.is_some() || publishing_phase.is_some();

        if video_status == Some("error") || uploading_phase == Some("error") {
            Self::fail_session_and_parent_tx(
                conn,
                sess.id,
                sess.post_id,
                owner,
                "in_progress",
                "reel_processing_error",
                "Meta Reel status returned error",
            )?;
            return Ok(());
        }
        if video_status == Some("ready") || uploading_phase == Some("complete") {
            // Binary upload completed but its response was lost; re-streaming is forbidden.
            let advanced = conn.execute(
                "UPDATE facebook_upload_sessions SET status = 'finishing', updated_at = datetime('now')
                 WHERE id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                params![sess.id, owner],
            )?;
            if advanced != 1 {
                return Err(anyhow!("Lost lease advancing Reel session to finishing"));
            }
            return self
                .execute_reel_finish(conn, sess, parent, owner, page_token, video_id)
                .await;
        }
        if !has_authoritative_field {
            // Ambiguous payload: defer to next reconciliation cycle
            return Err(anyhow!(
                "Reel status payload exposes no authoritative classification field"
            ));
        }

        // Authoritative upload still in progress: re-stream remaining binary payload
        let file_size = sess.file_size_bytes as u64;
        let file = tokio::fs::File::open(&sess.file_path)
            .await
            .map_err(|e| anyhow!("Failed to open Reel file '{}': {}", sess.file_path, e))?;
        let stream = tokio_util::io::ReaderStream::new(file);
        let body_stream = reqwest::Body::wrap_stream(stream);

        let rupload_url = format!(
            "https://rupload.facebook.com/video-upload/v26.0/{}",
            video_id
        );
        let upload_resp = self
            .client
            .post(&rupload_url)
            .header("Authorization", format!("OAuth {}", page_token))
            .header("offset", "0")
            .header("file_size", file_size.to_string())
            .header("Content-Type", "application/octet-stream")
            .body(body_stream)
            .send()
            .await
            .map_err(|e| anyhow!("Network error streaming Reel binary: {}", e))?;
        let up_status = upload_resp.status();
        if up_status.as_u16() == 429 {
            let retry_after = upload_resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            Self::record_session_backoff_with_status(
                conn,
                sess.id,
                owner,
                retry_after,
                "in_progress",
            )?;
            return Ok(());
        }
        if up_status.is_client_error() {
            Self::fail_session_and_parent_tx(
                conn,
                sess.id,
                sess.post_id,
                owner,
                "in_progress",
                "reel_upload_client_error",
                &format!(
                    "Meta rejected Reel binary upload with HTTP {}",
                    up_status.as_u16()
                ),
            )?;
            return Ok(());
        }
        if !up_status.is_success() {
            return Err(anyhow!(
                "Meta server error streaming Reel binary (HTTP {})",
                up_status.as_u16()
            ));
        }
        let up_json: Value = upload_resp
            .json()
            .await
            .map_err(|e| anyhow!("Invalid JSON in Reel binary upload response: {}", e))?;
        if up_json.get("success").and_then(|v| v.as_bool()) != Some(true) {
            Self::fail_session_and_parent_tx(
                conn,
                sess.id,
                sess.post_id,
                owner,
                "in_progress",
                "reel_upload_not_successful",
                &format!(
                    "Meta Reel binary upload did not report success: {}",
                    up_json
                ),
            )?;
            return Err(anyhow!(
                "Meta Reel binary upload did not report success: {}",
                up_json
            ));
        }

        conn.execute(
            "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND owner_id = ?3 AND status = 'in_progress'",
            params![file_size as i64, sess.id, owner],
        )?;

        let advanced = conn.execute(
            "UPDATE facebook_upload_sessions SET status = 'finishing', updated_at = datetime('now')
             WHERE id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
            params![sess.id, owner],
        )?;
        if advanced != 1 {
            return Err(anyhow!("Lost lease advancing Reel session to finishing"));
        }
        self.execute_reel_finish(conn, sess, parent, owner, page_token, video_id)
            .await
    }

    // ---------------- finishing ----------------

    async fn handle_finishing(
        &self,
        conn: &mut Connection,
        sess: &ClaimedUploadSession,
        parent: &ParentPostContext,
        owner: &str,
    ) -> Result<()> {
        let page_token = self.resolve_parent_token(conn, parent).await?;
        let video_id = sess
            .video_id
            .clone()
            .ok_or_else(|| anyhow!("finishing session missing authoritative video_id"))?;

        let check_url = if sess.upload_type == "video" {
            format!(
                "https://graph.facebook.com/v26.0/{}?fields=id,status,published&access_token={}",
                video_id, page_token
            )
        } else {
            format!(
                "https://graph.facebook.com/v26.0/{}?fields=id,status&access_token={}",
                video_id, page_token
            )
        };

        let resp = self
            .client
            .get(&check_url)
            .send()
            .await
            .map_err(|e| anyhow!("Network error checking finishing session: {}", e))?;
        let status_code = resp.status();
        if status_code.as_u16() == 429 {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            Self::record_session_backoff_with_status(
                conn,
                sess.id,
                owner,
                retry_after,
                "finishing",
            )?;
            return Ok(());
        }
        if status_code.is_client_error() {
            Self::fail_session_and_parent_tx(
                conn,
                sess.id,
                sess.post_id,
                owner,
                "finishing",
                "finishing_client_error",
                &format!(
                    "Meta rejected finishing status check with HTTP {}",
                    status_code.as_u16()
                ),
            )?;
            return Ok(());
        }
        if !status_code.is_success() {
            return Err(anyhow!(
                "Meta server error checking finishing session (HTTP {})",
                status_code.as_u16()
            ));
        }
        let body: Value = resp
            .json()
            .await
            .map_err(|e| anyhow!("Invalid JSON in finishing check response: {}", e))?;

        let video_status = body
            .pointer("/status/video_status")
            .and_then(|v| v.as_str());
        let publishing_phase = body
            .pointer("/status/publishing_phase/status")
            .and_then(|v| v.as_str());
        let is_published_field = body
            .get("published")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if video_status == Some("error") || publishing_phase == Some("error") {
            return self
                .propagate_terminal_failure(conn, sess, owner, &video_id, "meta_video_error")
                .await;
        }

        let terminal_success = is_published_field
            || video_status == Some("ready")
            || publishing_phase == Some("complete");

        if !terminal_success {
            // Non-terminal processing state: release lease for the next cycle
            Self::release_session_lease(conn, sess.id, owner)?;
            return Ok(());
        }

        let is_future = parent
            .scheduled_epoch
            .map(|t| t > chrono::Utc::now().timestamp())
            .unwrap_or(false);
        let is_live = if is_future {
            FacebookUploader::verify_scheduled_publication(
                &self.client,
                &sess.page_id,
                &page_token,
                &video_id,
            )
            .await
        } else {
            FacebookUploader::verify_feed_publication(
                &self.client,
                &sess.page_id,
                &page_token,
                &video_id,
            )
            .await
        };

        if !is_live {
            // Not yet verified on feed or scheduled endpoints: release lease and wait
            Self::release_session_lease(conn, sess.id, owner)?;
            return Ok(());
        }

        let target_status = if is_future {
            "scheduled_on_meta"
        } else {
            "published"
        };

        // Atomic transaction: session CAS completion + scheduled post propagation
        let tx = conn.transaction()?;
        let session_rows = tx.execute(
            "UPDATE facebook_upload_sessions
             SET status = 'completed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now')
             WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
            params![sess.id, owner],
        )?;

        if session_rows == 1 {
            let post_rows = tx.execute(
                "UPDATE facebook_scheduled_posts
                 SET status = ?1,
                     remote_post_id = ?2,
                     error_code = NULL,
                     error_message = NULL,
                     owner_id = NULL,
                     claim_deadline = NULL,
                     updated_at = datetime('now')
                 WHERE id = ?3 AND status IN ('publishing', 'delivery_unknown')",
                params![target_status, video_id, sess.post_id],
            )?;

            if post_rows != 1 {
                let post_status: Option<String> = tx
                    .query_row(
                        "SELECT status FROM facebook_scheduled_posts WHERE id = ?1",
                        params![sess.post_id],
                        |r| r.get(0),
                    )
                    .optional()?;

                match post_status.as_deref() {
                    Some("published") | Some("scheduled_on_meta") => {
                        // Already terminal and consistent
                    }
                    _ => {
                        return Err(anyhow!(
                            "Reconciler invariant violation: expected exactly 1 updated post row for id {} (affected {}, current: {:?}); rolling back",
                            sess.post_id,
                            post_rows,
                            post_status
                        ));
                    }
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    async fn propagate_terminal_failure(
        &self,
        conn: &mut Connection,
        sess: &ClaimedUploadSession,
        owner: &str,
        video_id: &str,
        error_code: &str,
    ) -> Result<()> {
        let err_msg = format!("Meta reported terminal failure for video {}", video_id);
        // Atomic transaction: session CAS failure + scheduled post failure propagation
        let tx = conn.transaction()?;
        let session_rows = tx.execute(
            "UPDATE facebook_upload_sessions
             SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now')
             WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
            params![sess.id, owner],
        )?;

        if session_rows == 1 {
            let post_rows = tx.execute(
                "UPDATE facebook_scheduled_posts
                 SET status = 'failed', error_code = ?1, error_message = ?2, owner_id = NULL, claim_deadline = NULL, updated_at = datetime('now')
                 WHERE id = ?3 AND status IN ('publishing', 'delivery_unknown')",
                params![error_code, err_msg, sess.post_id],
            )?;

            if post_rows != 1 {
                let post_status: Option<String> = tx
                    .query_row(
                        "SELECT status FROM facebook_scheduled_posts WHERE id = ?1",
                        params![sess.post_id],
                        |r| r.get(0),
                    )
                    .optional()?;

                if post_status.as_deref() != Some("failed") {
                    return Err(anyhow!(
                        "Reconciler invariant violation: expected exactly 1 updated post row for id {} (affected {}, current: {:?}); rolling back",
                        sess.post_id,
                        post_rows,
                        post_status
                    ));
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ---------------- FINISH issuers (after in_progress completes) ----------------

    async fn execute_video_finish(
        &self,
        conn: &Connection,
        sess: &ClaimedUploadSession,
        parent: &ParentPostContext,
        owner: &str,
        page_token: &str,
        _video_id: &str,
    ) -> Result<()> {
        let finish_url = format!(
            "https://graph-video.facebook.com/v26.0/{}/videos",
            sess.page_id
        );
        let mut finish_body = json!({
            "access_token": page_token,
            "upload_phase": "finish",
            "upload_session_id": sess.upload_session_id,
            "description": parent.message,
        });
        if let Some(epoch) = parent.scheduled_epoch {
            if epoch > chrono::Utc::now().timestamp() {
                finish_body["published"] = json!(false);
                finish_body["scheduled_publish_time"] = json!(epoch);
            }
        }

        let resp = self
            .client
            .post(&finish_url)
            .json(&finish_body)
            .send()
            .await
            .map_err(|e| anyhow!("Transport error finishing video: {}", e))?;
        let status_code = resp.status();
        if status_code.as_u16() == 429 {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            Self::record_session_backoff_with_status(
                conn,
                sess.id,
                owner,
                retry_after,
                "finishing",
            )?;
            return Ok(());
        }
        if status_code.is_client_error() {
            Self::fail_session_and_parent_tx(
                conn,
                sess.id,
                sess.post_id,
                owner,
                "finishing",
                "video_finish_client_error",
                &format!(
                    "Meta rejected video finish with HTTP {}",
                    status_code.as_u16()
                ),
            )?;
            return Ok(());
        }
        if !status_code.is_success() {
            return Err(anyhow!(
                "Meta server error finishing video (HTTP {})",
                status_code.as_u16()
            ));
        }
        Ok(())
    }

    async fn execute_reel_finish(
        &self,
        conn: &Connection,
        sess: &ClaimedUploadSession,
        parent: &ParentPostContext,
        owner: &str,
        page_token: &str,
        video_id: &str,
    ) -> Result<()> {
        let finish_url = format!(
            "https://graph.facebook.com/v26.0/{}/video_reels",
            sess.page_id
        );
        let mut finish_body = json!({
            "access_token": page_token,
            "upload_phase": "finish",
            "video_id": video_id,
            "video_state": "PUBLISHED",
            "description": parent.message,
        });
        if let Some(epoch) = parent.scheduled_epoch {
            if epoch > chrono::Utc::now().timestamp() {
                finish_body["video_state"] = json!("SCHEDULED");
                finish_body["scheduled_publish_time"] = json!(epoch);
            }
        }

        let resp = self
            .client
            .post(&finish_url)
            .json(&finish_body)
            .send()
            .await
            .map_err(|e| anyhow!("Network error finalizing Reel: {}", e))?;
        let status_code = resp.status();
        if status_code.as_u16() == 429 {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            Self::record_session_backoff_with_status(
                conn,
                sess.id,
                owner,
                retry_after,
                "finishing",
            )?;
            return Ok(());
        }
        if status_code.is_client_error() {
            Self::fail_session_and_parent_tx(
                conn,
                sess.id,
                sess.post_id,
                owner,
                "finishing",
                "reel_finish_client_error",
                &format!(
                    "Meta rejected reel finish with HTTP {}",
                    status_code.as_u16()
                ),
            )?;
            return Ok(());
        }
        if !status_code.is_success() {
            return Err(anyhow!(
                "Meta server error finalizing Reel (HTTP {})",
                status_code.as_u16()
            ));
        }
        Ok(())
    }

    // ---------------- small helpers ----------------

    fn parse_json_u64(value: &Value, key: &str) -> Option<u64> {
        match value.get(key) {
            Some(Value::String(s)) => s.parse::<u64>().ok(),
            Some(Value::Number(n)) => n.as_u64(),
            _ => None,
        }
    }

    fn record_session_backoff_with_status(
        conn: &Connection,
        session_id: i64,
        owner: &str,
        retry_after_secs: u64,
        from_status: &str,
    ) -> Result<()> {
        conn.execute(
            "UPDATE facebook_upload_sessions
             SET owner_id = NULL,
                 lease_deadline = datetime('now', '+' || ?3 || ' seconds'),
                 retry_count = retry_count + 1,
                 updated_at = datetime('now')
             WHERE id = ?1 AND owner_id = ?2 AND status = ?4",
            params![session_id, owner, retry_after_secs as i64, from_status],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_sqlite() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE facebook_page_tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL,
                page_id TEXT NOT NULL UNIQUE,
                access_token_encrypted TEXT NOT NULL,
                token_status TEXT NOT NULL DEFAULT 'valid',
                granted_scopes_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE facebook_scheduled_posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                draft_group_id TEXT NOT NULL,
                channel_account_id INTEGER NOT NULL,
                page_id TEXT NOT NULL,
                post_type TEXT NOT NULL,
                message TEXT NOT NULL,
                link_url TEXT,
                media_paths_json TEXT NOT NULL DEFAULT '[]',
                scheduling_mode TEXT NOT NULL DEFAULT 'meta',
                scheduled_publish_time TEXT,
                next_retry_at TEXT,
                remote_post_id TEXT,
                upload_session_id TEXT,
                idempotency_key TEXT NOT NULL UNIQUE,
                status TEXT NOT NULL DEFAULT 'draft',
                retry_count INTEGER NOT NULL DEFAULT 0,
                owner_id TEXT,
                claim_deadline TEXT,
                error_code TEXT,
                error_message TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE facebook_upload_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                post_id INTEGER NOT NULL REFERENCES facebook_scheduled_posts(id) ON DELETE CASCADE,
                page_id TEXT NOT NULL,
                upload_type TEXT NOT NULL DEFAULT 'video',
                upload_session_id TEXT NOT NULL UNIQUE,
                video_id TEXT,
                file_path TEXT NOT NULL,
                file_hash TEXT NOT NULL,
                file_size_bytes INTEGER NOT NULL,
                start_offset INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'init_pending',
                retry_count INTEGER NOT NULL DEFAULT 0,
                owner_id TEXT,
                lease_deadline TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    fn insert_post(conn: &Connection, status: &str, next_retry_at: Option<&str>) -> i64 {
        conn.execute(
            "INSERT INTO facebook_scheduled_posts
             (draft_group_id, channel_account_id, page_id, post_type, message, media_paths_json, next_retry_at, idempotency_key, status, created_at, updated_at)
             VALUES ('grp-test', 1, 'page_123', 'status', 'hello world', '[]', ?1, ?2, ?3, datetime('now'), datetime('now'))",
            params![next_retry_at, uuid::Uuid::new_v4().to_string(), status],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn insert_post_with_id(conn: &Connection, id: i64, status: &str, next_retry_at: Option<&str>) {
        conn.execute(
            "INSERT INTO facebook_scheduled_posts
             (id, draft_group_id, channel_account_id, page_id, post_type, message, media_paths_json, next_retry_at, idempotency_key, status, created_at, updated_at)
             VALUES (?1, 'grp-test', 1, 'page_123', 'status', 'hello world', '[]', ?2, ?3, ?4, datetime('now'), datetime('now'))",
            params![id, next_retry_at, uuid::Uuid::new_v4().to_string(), status],
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_video_start_response_lost_recovers_via_fresh_start_cas() {
        let conn = setup_test_sqlite();
        insert_post_with_id(&conn, 100, "publishing", None);
        // Insert existing session in init_pending state matching exact SQLite schema (video_id = NULL, retry_count = 0)
        conn.execute(
            "INSERT INTO facebook_upload_sessions (id, post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
             VALUES (1, 100, 'page_123', 'video', 'intent_100_uuid', NULL, '/path/video.mp4', 'hash123', 10485760, 0, 'init_pending', 0, NULL, NULL, datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // Worker claims lease
        let worker_id = "worker_1";
        let claimed = conn.execute(
            "UPDATE facebook_upload_sessions SET owner_id = ?1, lease_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now') WHERE id = 1 AND status = 'init_pending'",
            params![worker_id],
        ).unwrap();
        assert_eq!(claimed, 1);

        // Execute START (returns remote upload_session_id="meta_sess_456", video_id="meta_vid_789")
        let updated = conn.execute(
            "UPDATE facebook_upload_sessions SET upload_session_id = 'meta_sess_456', video_id = 'meta_vid_789', status = 'in_progress', updated_at = datetime('now') WHERE id = 1 AND owner_id = ?1 AND status = 'init_pending'",
            params![worker_id],
        ).unwrap();
        assert_eq!(updated, 1);

        // Verify session successfully transitioned to in_progress with authoritative Meta remote IDs
        let (status, sess_id, vid_id): (String, String, Option<String>) = conn.query_row(
            "SELECT status, upload_session_id, video_id FROM facebook_upload_sessions WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap();
        assert_eq!(status, "in_progress");
        assert_eq!(sess_id, "meta_sess_456");
        assert_eq!(vid_id, Some("meta_vid_789".to_string()));
    }

    #[tokio::test]
    async fn test_start_response_received_local_cas_lease_loss_aborts() {
        let conn = setup_test_sqlite();
        insert_post_with_id(&conn, 101, "publishing", None);
        conn.execute(
            "INSERT INTO facebook_upload_sessions (id, post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
             VALUES (2, 101, 'page_123', 'video', 'intent_101_uuid', NULL, '/path/video.mp4', 'hash123', 10485760, 0, 'init_pending', 0, 'worker_2_stolen', datetime('now', '+5 minutes'), datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // Worker 1 attempts CAS with old owner_id
        let worker_1 = "worker_1";
        let updated = conn.execute(
            "UPDATE facebook_upload_sessions SET upload_session_id = 'meta_sess_new', video_id = 'meta_vid_new', status = 'in_progress', updated_at = datetime('now') WHERE id = 2 AND owner_id = ?1 AND status = 'init_pending'",
            params![worker_1],
        ).unwrap();

        // Must reject transition because lease was stolen
        assert_eq!(updated, 0);
    }

    #[tokio::test]
    async fn test_promotion_marks_invalid_timestamp_failed_failclosed() {
        let conn = setup_test_sqlite();
        let post_id = insert_post(&conn, "queued", None);
        conn.execute(
            "UPDATE facebook_scheduled_posts SET scheduled_publish_time = 'not-a-timestamp' WHERE id = ?1",
            params![post_id],
        )
        .unwrap();

        let scheduler = FacebookScheduler::new();
        let summary = scheduler.run_promotion_cycle(&conn, 10).await.unwrap();
        assert_eq!(summary.failed, 1);

        let (status, error_code): (String, String) = conn
            .query_row(
                "SELECT status, error_code FROM facebook_scheduled_posts WHERE id = ?1",
                params![post_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "failed");
        assert_eq!(error_code, "invalid_timestamp");
    }

    #[tokio::test]
    async fn test_promotion_fails_closed_without_valid_page_token() {
        let conn = setup_test_sqlite();
        let post_id = insert_post(&conn, "queued", None);

        let scheduler = FacebookScheduler::new();
        let summary = scheduler.run_promotion_cycle(&conn, 10).await.unwrap();
        assert_eq!(summary.failed, 1);

        let (status, error_code): (String, String) = conn
            .query_row(
                "SELECT status, error_code FROM facebook_scheduled_posts WHERE id = ?1",
                params![post_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "failed");
        assert_eq!(error_code, "token_vault_error");
    }

    #[tokio::test]
    async fn test_upload_session_claim_respects_parent_backoff() {
        let conn = setup_test_sqlite();
        let post_id = insert_post(
            &conn,
            "publishing",
            Some(datetime_future_seconds(120).as_str()),
        );

        conn.execute(
            "INSERT INTO facebook_upload_sessions (post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
             VALUES (?1, 'page_123', 'video', 'intent_backoff', NULL, '/path/video.mp4', 'hash123', 1024, 0, 'init_pending', 0, NULL, NULL, datetime('now'), datetime('now'))",
            params![post_id],
        )
        .unwrap();

        let scheduler = FacebookScheduler::new();
        let mut conn_ref = conn;
        let resolved = scheduler
            .reconcile_upload_sessions(&mut conn_ref, 5)
            .await
            .unwrap();
        assert_eq!(resolved, 0, "must not claim while parent backoff pending");
    }

    #[tokio::test]
    async fn test_delivery_unknown_claim_is_atomic_single_winner() {
        let conn = setup_test_sqlite();
        insert_post(&conn, "delivery_unknown", None);

        let winner: Option<i64> = conn
            .query_row(
                "UPDATE facebook_scheduled_posts
                 SET owner_id = 'w1', claim_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
                 WHERE id = (
                     SELECT id FROM facebook_scheduled_posts
                     WHERE status = 'delivery_unknown'
                       AND (claim_deadline IS NULL OR claim_deadline < datetime('now'))
                     ORDER BY id LIMIT 1
                 )
                 AND status = 'delivery_unknown'
                 RETURNING id",
                [],
                |r| r.get(0),
            )
            .optional()
            .unwrap();
        assert!(winner.is_some());

        let loser: Option<i64> = conn
            .query_row(
                "UPDATE facebook_scheduled_posts
                 SET owner_id = 'w2', claim_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
                 WHERE id = (
                     SELECT id FROM facebook_scheduled_posts
                     WHERE status = 'delivery_unknown'
                       AND (claim_deadline IS NULL OR claim_deadline < datetime('now'))
                     ORDER BY id LIMIT 1
                 )
                 AND status = 'delivery_unknown'
                 RETURNING id",
                [],
                |r| r.get(0),
            )
            .optional()
            .unwrap();
        assert!(
            loser.is_none(),
            "second claimer must not steal an active lease"
        );
    }

    #[test]
    fn test_post_type_matches_matrix() {
        assert!(FacebookScheduler::post_type_matches(
            "status",
            "mobile_status_update"
        ));
        assert!(FacebookScheduler::post_type_matches("link", "shared_story"));
        assert!(FacebookScheduler::post_type_matches(
            "photo",
            "added_photos"
        ));
        assert!(FacebookScheduler::post_type_matches(
            "multi_photo",
            "added_photos"
        ));
        assert!(FacebookScheduler::post_type_matches("video", "added_video"));
        assert!(FacebookScheduler::post_type_matches("reel", "added_video"));
        assert!(!FacebookScheduler::post_type_matches(
            "status",
            "added_video"
        ));
        assert!(!FacebookScheduler::post_type_matches("video", "link"));
    }

    #[test]
    fn test_message_hash_is_sha256_hex() {
        let expected = {
            use sha2::Digest;
            let mut h = sha2::Sha256::new();
            h.update(b"hello world");
            format!("{:x}", h.finalize())
        };
        assert_eq!(FacebookScheduler::message_hash("hello world"), expected);
    }

    #[test]
    fn test_timestamp_within_window() {
        let reference = chrono::Utc::now().timestamp();
        let near = chrono::Utc::now() - chrono::Duration::seconds(30);
        let far = chrono::Utc::now() - chrono::Duration::seconds(300);
        assert!(FacebookScheduler::timestamp_within_window(
            Some(&near.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            Some(reference)
        ));
        assert!(!FacebookScheduler::timestamp_within_window(
            Some(&far.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            Some(reference)
        ));
    }

    #[tokio::test]
    async fn test_reconciliation_cycle_production_call_claims_row() {
        let conn = setup_test_sqlite();
        let post_id = insert_post(&conn, "delivery_unknown", None);

        let scheduler = FacebookScheduler::new();
        // Since no valid token exists in SQLite test db, reconciliation should fail closed / release lease without error
        let res = scheduler.run_reconciliation_cycle(&conn, 5).await;
        assert!(
            res.is_ok(),
            "run_reconciliation_cycle must execute without SQL binding errors"
        );

        // The post should still exist in delivery_unknown with lease cleared
        let (status, owner_id): (String, Option<String>) = conn
            .query_row(
                "SELECT status, owner_id FROM facebook_scheduled_posts WHERE id = ?1",
                params![post_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "delivery_unknown");
        assert!(
            owner_id.is_none(),
            "lease must be released on token failure"
        );
    }

    #[tokio::test]
    async fn test_promotion_boundary_greater_than_30_days_remains_queued() {
        let conn = setup_test_sqlite();
        let post_id = insert_post(&conn, "queued", None);
        let future_45_days = chrono::Utc::now() + chrono::Duration::days(45);
        conn.execute(
            "UPDATE facebook_scheduled_posts SET scheduled_publish_time = ?1 WHERE id = ?2",
            params![future_45_days.to_rfc3339(), post_id],
        )
        .unwrap();

        let scheduler = FacebookScheduler::new();
        let summary = scheduler.run_promotion_cycle(&conn, 10).await.unwrap();
        assert_eq!(
            summary.deferred, 1,
            "post >30 days must be deferred locally"
        );
        assert_eq!(summary.failed, 0);

        let status: String = conn
            .query_row(
                "SELECT status FROM facebook_scheduled_posts WHERE id = ?1",
                params![post_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            status, "queued",
            "post must remain queued for future promotion"
        );
    }

    #[tokio::test]
    async fn test_fail_session_and_parent_tx_atomically_updates_both_tables() {
        let conn = setup_test_sqlite();
        insert_post_with_id(&conn, 500, "publishing", None);
        conn.execute(
            "INSERT INTO facebook_upload_sessions (id, post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
             VALUES (10, 500, 'page_123', 'video', 'intent_fail_tx', NULL, '/path/video.mp4', 'hash123', 1024, 0, 'in_progress', 0, 'worker_tx', datetime('now', '+5 minutes'), datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        FacebookScheduler::fail_session_and_parent_tx(
            &conn,
            10,
            500,
            "worker_tx",
            "in_progress",
            "test_error_code",
            "Deterministic client rejection",
        )
        .unwrap();

        let (sess_status, sess_owner): (String, Option<String>) = conn
            .query_row(
                "SELECT status, owner_id FROM facebook_upload_sessions WHERE id = 10",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(sess_status, "failed");
        assert!(sess_owner.is_none());

        let (post_status, err_code, err_msg): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, error_code, error_message FROM facebook_scheduled_posts WHERE id = 500",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(post_status, "failed");
        assert_eq!(err_code.as_deref(), Some("test_error_code"));
        assert_eq!(err_msg.as_deref(), Some("Deterministic client rejection"));
    }

    #[tokio::test]
    async fn test_fail_session_and_parent_tx_rolls_back_on_lease_mismatch() {
        let conn = setup_test_sqlite();
        insert_post_with_id(&conn, 501, "publishing", None);
        conn.execute(
            "INSERT INTO facebook_upload_sessions (id, post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
             VALUES (11, 501, 'page_123', 'video', 'intent_fail_tx2', NULL, '/path/video.mp4', 'hash123', 1024, 0, 'in_progress', 0, 'actual_owner', datetime('now', '+5 minutes'), datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // Attempt failure propagation with mismatched owner
        let res = FacebookScheduler::fail_session_and_parent_tx(
            &conn,
            11,
            501,
            "stolen_worker",
            "in_progress",
            "test_error",
            "failure message",
        );
        assert!(res.is_err(), "Must error on lease mismatch");

        // Verify session remains in_progress and parent remains publishing (rolled back)
        let sess_status: String = conn
            .query_row(
                "SELECT status FROM facebook_upload_sessions WHERE id = 11",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let post_status: String = conn
            .query_row(
                "SELECT status FROM facebook_scheduled_posts WHERE id = 501",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sess_status, "in_progress");
        assert_eq!(post_status, "publishing");
    }

    #[tokio::test]
    async fn test_promotion_boundary_sub_10_minutes_remains_queued() {
        let conn = setup_test_sqlite();
        let post_id = insert_post(&conn, "queued", None);
        let future_5m = chrono::Utc::now() + chrono::Duration::minutes(5);
        conn.execute(
            "UPDATE facebook_scheduled_posts SET scheduled_publish_time = ?1 WHERE id = ?2",
            params![future_5m.to_rfc3339(), post_id],
        )
        .unwrap();

        let scheduler = FacebookScheduler::new();
        let summary = scheduler.run_promotion_cycle(&conn, 10).await.unwrap();
        assert_eq!(
            summary.deferred, 1,
            "Sub-10m post must be deferred until target time arrives"
        );

        let status: String = conn
            .query_row(
                "SELECT status FROM facebook_scheduled_posts WHERE id = ?1",
                [post_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "queued");
    }

    #[tokio::test]
    async fn test_promotion_boundary_overdue_greater_than_24h_marked_overdue_prompt() {
        let conn = setup_test_sqlite();
        let post_id = insert_post(&conn, "queued", None);
        let past_30h = chrono::Utc::now() - chrono::Duration::hours(30);
        conn.execute(
            "UPDATE facebook_scheduled_posts SET scheduled_publish_time = ?1 WHERE id = ?2",
            params![past_30h.to_rfc3339(), post_id],
        )
        .unwrap();

        let scheduler = FacebookScheduler::new();
        let summary = scheduler.run_promotion_cycle(&conn, 10).await.unwrap();
        assert_eq!(summary.failed, 1);

        let (status, err_code): (String, Option<String>) = conn
            .query_row(
                "SELECT status, error_code FROM facebook_scheduled_posts WHERE id = ?1",
                [post_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "overdue_prompt");
        assert_eq!(err_code.as_deref(), Some("overdue_schedule"));
    }

    fn datetime_future_seconds(seconds: i64) -> String {
        let dt = chrono::Utc::now() + chrono::Duration::seconds(seconds);
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}
