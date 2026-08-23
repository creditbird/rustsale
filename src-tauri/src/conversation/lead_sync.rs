use anyhow::{anyhow, Result};
use reqwest::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadAdData {
    pub leadgen_id: String,
    pub form_id: Option<String>,
    pub created_time: Option<String>,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub company: String,
    pub about: String,
    pub ad_name: Option<String>,
    pub campaign_name: Option<String>,
}

pub struct FacebookLeadSync {
    client: Client,
}

impl Default for FacebookLeadSync {
    fn default() -> Self {
        Self::new()
    }
}

impl FacebookLeadSync {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    pub fn parse_field_data(body: &Value) -> LeadAdData {
        let leadgen_id = body
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let form_id = body
            .get("form_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let created_time = body
            .get("created_time")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let ad_name = body
            .get("ad_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let campaign_name = body
            .get("campaign_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut name = String::new();
        let mut email = String::new();
        let mut phone = String::new();
        let mut company = String::new();
        let mut custom_notes = Vec::new();

        if let Some(field_data) = body.get("field_data").and_then(|v| v.as_array()) {
            for item in field_data {
                let name_key = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let values = item.get("values").and_then(|v| v.as_array());
                let first_val = values
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim();

                if first_val.is_empty() {
                    continue;
                }

                match name_key {
                    "full_name" | "name" | "your_name" => {
                        if name.is_empty() {
                            name = first_val.to_string();
                        }
                    }
                    "email" | "user_email" => {
                        if email.is_empty() {
                            email = first_val.to_string();
                        }
                    }
                    "phone_number" | "phone" => {
                        if phone.is_empty() {
                            phone = first_val.to_string();
                        }
                    }
                    "company_name" | "company" => {
                        if company.is_empty() {
                            company = first_val.to_string();
                        }
                    }
                    _ => {
                        custom_notes.push(format!("{}: {}", name_key, first_val));
                    }
                }
            }
        }

        if name.is_empty() {
            name = format!("Facebook Lead {}", leadgen_id);
        }
        if company.is_empty() {
            company = "Individual".to_string();
        }

        let mut about_parts = Vec::new();
        if let Some(ref c) = campaign_name {
            about_parts.push(format!("Campaign: {}", c));
        }
        if let Some(ref a) = ad_name {
            about_parts.push(format!("Ad: {}", a));
        }
        if let Some(ref f) = form_id {
            about_parts.push(format!("Form ID: {}", f));
        }
        about_parts.extend(custom_notes);

        let about = about_parts.join(" | ");

        LeadAdData {
            leadgen_id,
            form_id,
            created_time,
            name,
            email,
            phone,
            company,
            about,
            ad_name,
            campaign_name,
        }
    }

    pub async fn fetch_lead_details(
        &self,
        leadgen_id: &str,
        page_token: &str,
    ) -> Result<LeadAdData> {
        let url = format!(
            "https://graph.facebook.com/v26.0/{}?fields=id,created_time,field_data,form_id,ad_id,ad_name,adset_name,campaign_name",
            leadgen_id
        );

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", page_token))
            .send()
            .await
            .map_err(|e| anyhow!("Transport error fetching leadgen details: {}", e))?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "Meta Graph API returned HTTP {} for leadgen_id {}",
                resp.status(),
                leadgen_id
            ));
        }

        let body: Value = resp
            .json()
            .await
            .map_err(|e| anyhow!("Invalid JSON in leadgen details response: {}", e))?;

        Ok(Self::parse_field_data(&body))
    }

    pub fn insert_or_ignore_lead(conn: &Connection, lead: &LeadAdData) -> Result<Option<i64>> {
        // Check if already exists by checking about field or email/phone match
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM leads WHERE about LIKE ?1",
                [format!("%{}%", lead.leadgen_id)],
                |r| r.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            return Ok(Some(id));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let about_with_id = format!("[LeadGen ID: {}] {}", lead.leadgen_id, lead.about);

        conn.execute(
            "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about)
             VALUES (?1, ?2, 'new', 0.0, ?3, 'Facebook Lead Ads', ?4, ?5, ?6)",
            params![
                lead.name,
                lead.company,
                now,
                lead.email,
                lead.phone,
                about_with_id
            ],
        )?;

        let lead_id = conn.last_insert_rowid();
        Ok(Some(lead_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn setup_test_leads_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE leads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                company TEXT NOT NULL,
                status TEXT NOT NULL,
                value REAL NOT NULL DEFAULT 0.0,
                last_activity TEXT NOT NULL,
                owner TEXT NOT NULL,
                email TEXT NOT NULL,
                phone TEXT NOT NULL,
                about TEXT
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_parse_field_data_extracts_standard_and_custom_fields() {
        let payload = json!({
            "id": "lead_123456",
            "created_time": "2026-08-23T02:00:00+0000",
            "form_id": "form_789",
            "ad_name": "Summer Sale Promo",
            "campaign_name": "Brand Awareness 2026",
            "field_data": [
                {
                    "name": "full_name",
                    "values": ["Nguyen Van A"]
                },
                {
                    "name": "email",
                    "values": ["nguyenvana@example.com"]
                },
                {
                    "name": "phone_number",
                    "values": ["+84901234567"]
                },
                {
                    "name": "company_name",
                    "values": ["Acme Corp"]
                },
                {
                    "name": "budget_range",
                    "values": ["$5000 - $10000"]
                }
            ]
        });

        let lead = FacebookLeadSync::parse_field_data(&payload);
        assert_eq!(lead.leadgen_id, "lead_123456");
        assert_eq!(lead.name, "Nguyen Van A");
        assert_eq!(lead.email, "nguyenvana@example.com");
        assert_eq!(lead.phone, "+84901234567");
        assert_eq!(lead.company, "Acme Corp");
        assert!(lead.about.contains("Campaign: Brand Awareness 2026"));
        assert!(lead.about.contains("budget_range: $5000 - $10000"));
    }

    #[test]
    fn test_insert_or_ignore_lead_is_idempotent() {
        let conn = setup_test_leads_db();

        let lead = LeadAdData {
            leadgen_id: "lead_999".to_string(),
            form_id: Some("form_1".to_string()),
            created_time: Some("2026-08-23T02:00:00Z".to_string()),
            name: "Tran Van B".to_string(),
            email: "tranb@example.com".to_string(),
            phone: "+84987654321".to_string(),
            company: "Tech JSC".to_string(),
            about: "Interested in CRM".to_string(),
            ad_name: Some("Ad 1".to_string()),
            campaign_name: Some("Camp 1".to_string()),
        };

        let id1 = FacebookLeadSync::insert_or_ignore_lead(&conn, &lead).unwrap();
        assert!(id1.is_some());

        let id2 = FacebookLeadSync::insert_or_ignore_lead(&conn, &lead).unwrap();
        assert_eq!(
            id1, id2,
            "Duplicate leadgen_id must return existing row id without re-inserting"
        );

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM leads", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
