use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceMessage {
    pub id: i64,
    pub direction: String,
    pub sender_name: Option<String>,
    pub text_content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductEvidence {
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub category: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadEvidenceContext {
    pub lead_id: i32,
    pub lead_name: String,
    pub lead_company: String,
    pub lead_phone: String,
    pub lead_email: String,
    pub lead_status: String,
    pub lead_value: f64,
    pub lead_currency: String,
    pub lead_about: Option<String>,
    pub lead_address: Option<String>,
    pub deal_stage: Option<String>,
    pub deal_value: Option<f64>,
    pub deal_title: Option<String>,
    pub journey_stage: Option<String>,
    pub active_campaign_name: Option<String>,
    pub recent_messages: Vec<EvidenceMessage>,
    pub known_products: Vec<ProductEvidence>,
    pub relationship_status: Option<String>,
    pub categories: Vec<String>,
    pub verified_channels: Vec<String>,
}

impl LeadEvidenceContext {
    pub fn to_context_summary(&self) -> String {
        let mut summary = Vec::new();
        summary.push(format!("- Khách hàng: {} ({})", self.lead_name, self.lead_company));
        summary.push(format!("- Điện thoại: {} | Email: {}", self.lead_phone, self.lead_email));
        summary.push(format!("- Trạng thái Lead: {} | Giá trị: {:.0} {}", self.lead_status, self.lead_value, self.lead_currency));

        if let Some(ref stage) = self.deal_stage {
            let title = self.deal_title.as_deref().unwrap_or("Hợp đồng / Đơn hàng");
            let val = self.deal_value.unwrap_or(self.lead_value);
            summary.push(format!("- Deal hiện tại: {} (Giai đoạn: {}, Giá trị: {:.0} {})", title, stage, val, self.lead_currency));
        }

        if let Some(ref journey) = self.journey_stage {
            summary.push(format!("- Vòng đời chăm sóc (Journey Stage): {}", journey));
        }

        if let Some(ref camp) = self.active_campaign_name {
            summary.push(format!("- Đang tham gia chiến dịch: {}", camp));
        }

        if !self.categories.is_empty() {
            summary.push(format!("- Danh mục quan tâm: {}", self.categories.join(", ")));
        }

        if !self.known_products.is_empty() {
            let prods: Vec<String> = self.known_products
                .iter()
                .map(|p| format!("{} ({:.0} {})", p.name, p.price, p.currency))
                .collect();
            summary.push(format!("- Sản phẩm / Báo giá niêm yết: {}", prods.join("; ")));
        }

        if let Some(ref about) = self.lead_about {
            if !about.trim().is_empty() {
                summary.push(format!("- Ghi chú khách hàng: {}", about));
            }
        }

        if !self.recent_messages.is_empty() {
            summary.push("- Lịch sử hội thoại gần nhất:".to_string());
            for msg in &self.recent_messages {
                let sender = if msg.direction == "inbound" {
                    msg.sender_name.as_deref().unwrap_or("Khách hàng")
                } else if msg.direction == "outbound" {
                    "Sales Rep / Shop"
                } else {
                    "Hệ thống"
                };
                summary.push(format!("  [{}] {}: {}", msg.direction, sender, msg.text_content));
            }
        }

        summary.join("\n")
    }
}

pub struct EvidenceExtractor;

impl EvidenceExtractor {
    pub fn extract_lead_context(
        conn: &Connection,
        lead_id: i32,
        conversation_id: Option<i64>,
    ) -> Result<LeadEvidenceContext, String> {
        // 1. Fetch Lead
        let mut stmt = conn
            .prepare(
                "SELECT id, name, company, status, value, owner, email, phone, about, address,
                        has_zalo, zalo_reached, zalo_uid, zalo_status, relationship_status, currency
                 FROM leads WHERE id = ?1",
            )
            .map_err(|e| format!("DB error preparing lead query: {}", e))?;

        let lead_row = stmt
            .query_row([lead_id], |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, f64>(4)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(14)?,
                    row.get::<_, Option<String>>(15)?,
                ))
            })
            .optional()
            .map_err(|e| format!("DB error querying lead: {}", e))?
            .ok_or_else(|| format!("Lead with ID {} not found", lead_id))?;

        let (
            _id,
            name,
            company,
            status,
            value,
            email,
            phone,
            about,
            address,
            relationship_status,
            currency_opt,
        ) = lead_row;

        let currency = currency_opt.unwrap_or_else(|| "VND".to_string());

        // 2. Fetch Latest Deal if any
        let deal_info: Option<(String, f64, String)> = conn
            .query_row(
                "SELECT title, value, stage FROM deals WHERE lead_id = ?1 ORDER BY id DESC LIMIT 1",
                params![lead_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .unwrap_or(None);

        let (deal_title, deal_value, deal_stage) = match deal_info {
            Some((t, v, s)) => (Some(t), Some(v), Some(s)),
            None => (None, None, None),
        };

        // 3. Fetch Journey State and Active Campaign Name
        let journey_info: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT js.current_stage, c.name
                 FROM lead_journey_states js
                 LEFT JOIN campaigns c ON js.active_campaign_id = c.id
                 WHERE js.lead_id = ?1",
                params![lead_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .unwrap_or(None);

        let (journey_stage, active_campaign_name) = match journey_info {
            Some((js, cname)) => (Some(js), cname),
            None => (None, None),
        };

        // 4. Fetch Categories
        let mut categories = Vec::new();
        if let Ok(mut cat_stmt) = conn.prepare(
            "SELECT c.name FROM categories c
             JOIN lead_categories lc ON c.id = lc.category_id
             WHERE lc.lead_id = ?1",
        ) {
            if let Ok(rows) = cat_stmt.query_map([lead_id], |row| row.get::<_, String>(0)) {
                for r in rows.flatten() {
                    categories.push(r);
                }
            }
        }

        // 5. Fetch Verified Channels
        let mut verified_channels = Vec::new();
        if let Ok(mut chan_stmt) = conn.prepare(
            "SELECT DISTINCT channel FROM channel_identities WHERE lead_id = ?1",
        ) {
            if let Ok(rows) = chan_stmt.query_map([lead_id], |row| row.get::<_, String>(0)) {
                for r in rows.flatten() {
                    verified_channels.push(r);
                }
            }
        }

        // 6. Fetch Recent Messages
        let mut recent_messages = Vec::new();

        let target_conv_id = if let Some(cid) = conversation_id {
            Some(cid)
        } else {
            // Find most recent conversation linked to this lead
            conn.query_row(
                "SELECT c.id FROM channel_conversations c
                 JOIN channel_identities ci ON (c.channel_account_id = ci.channel_account_id)
                 WHERE ci.lead_id = ?1
                 ORDER BY c.last_message_at DESC, c.id DESC LIMIT 1",
                params![lead_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .unwrap_or(None)
            .or_else(|| {
                // Fallback: search via conversation_participants
                conn.query_row(
                    "SELECT cp.conversation_id FROM conversation_participants cp
                     JOIN channel_identities ci ON cp.identity_id = ci.id
                     WHERE ci.lead_id = ?1
                     ORDER BY cp.conversation_id DESC LIMIT 1",
                    params![lead_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .unwrap_or(None)
            })
        };

        if let Some(cid) = target_conv_id {
            if let Ok(mut msg_stmt) = conn.prepare(
                "SELECT m.id, m.direction, ci.display_name, m.text_content, COALESCE(m.sent_at, m.ingested_at)
                 FROM messages m
                 LEFT JOIN channel_identities ci ON m.sender_identity_id = ci.id
                 WHERE m.conversation_id = ?1
                 ORDER BY m.id DESC LIMIT 15",
            ) {
                if let Ok(rows) = msg_stmt.query_map([cid], |row| {
                    Ok(EvidenceMessage {
                        id: row.get(0)?,
                        direction: row.get(1)?,
                        sender_name: row.get(2)?,
                        text_content: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                        timestamp: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    })
                }) {
                    for r in rows.flatten() {
                        if !r.text_content.trim().is_empty() {
                            recent_messages.push(r);
                        }
                    }
                }
            }
            // Reverse so oldest to newest
            recent_messages.reverse();
        }

        // 7. Extract known product pricing evidence from deals / categories / lead value
        let mut known_products = Vec::new();
        if let (Some(ref title), Some(val)) = (&deal_title, deal_value) {
            known_products.push(ProductEvidence {
                name: title.clone(),
                price: val,
                currency: currency.clone(),
                category: categories.first().cloned(),
                description: Some(format!("Gói dịch vụ / Deal trong CRM: {}", title)),
            });
        } else if value > 0.0 {
            known_products.push(ProductEvidence {
                name: format!("Sản phẩm / Dịch vụ quan tâm ({})", company),
                price: value,
                currency: currency.clone(),
                category: categories.first().cloned(),
                description: about.clone(),
            });
        }

        Ok(LeadEvidenceContext {
            lead_id,
            lead_name: name,
            lead_company: company,
            lead_phone: phone,
            lead_email: email,
            lead_status: status,
            lead_value: value,
            lead_currency: currency,
            lead_about: about,
            lead_address: address,
            deal_stage,
            deal_value,
            deal_title,
            journey_stage,
            active_campaign_name,
            recent_messages,
            known_products,
            relationship_status,
            categories,
            verified_channels,
        })
    }

    pub fn log_evidence_decision(
        conn: &Connection,
        lead_id: Option<i32>,
        conversation_id: Option<i64>,
        action_type: &str,
        evidence_source: &str,
        evidence_data_json: &str,
        confidence_score: f64,
        decision: &str,
    ) -> Result<i64, String> {
        crate::db::insert_ai_evidence_log(
            conn,
            lead_id,
            conversation_id,
            action_type,
            evidence_source,
            evidence_data_json,
            confidence_score,
            decision,
        )
        .map_err(|e| format!("Failed to insert AI evidence log: {}", e))
    }
}
