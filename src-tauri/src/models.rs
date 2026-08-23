use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Lead {
    pub id: Option<i32>,
    pub name: String,
    pub company: String,
    pub status: String,
    pub value: f64,
    pub last_activity: String,
    pub owner: String,
    pub email: String,
    pub phone: String,
    pub about: Option<String>,
    pub address: Option<String>,
    pub has_zalo: Option<i32>,
    pub zalo_reached: i32,
    pub zalo_uid: Option<String>,
    pub zalo_status: Option<String>,
    pub relationship_status: Option<String>,
    pub last_lookup_attempt: Option<String>,
    #[serde(default = "default_currency")]
    pub currency: Option<String>,
}

fn default_currency() -> Option<String> {
    Some("VND".to_string())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Category {
    pub id: Option<i32>,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OutreachHistory {
    pub id: Option<i32>,
    pub lead_id: i32,
    pub template_id: i32,
    pub status: String,
    pub error_message: Option<String>,
    pub sent_at: String,
    pub session_imei: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Activity {
    pub id: Option<i32>,
    pub lead_id: i32,
    pub activity_type: String,
    pub content: String,
    pub created_at: String,
    pub performed_by: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Deal {
    pub id: Option<i32>,
    pub lead_id: i32,
    pub title: String,
    pub value: f64,
    pub stage: String,
    pub created_at: String,
    #[serde(default = "default_currency")]
    pub currency: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OutreachTemplate {
    pub id: Option<i32>,
    pub name: String,
    pub content: String,
    pub image_url: Option<String>,
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZaloSession {
    pub imei: String,
    pub user_agent: String,
    pub cookies_json: String,
    pub language: String,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub zalo_uid: Option<String>,
    pub channel_account_id: Option<i64>,
}

impl ZaloSession {
    pub fn provider_account_key(&self) -> &str {
        self.zalo_uid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.imei)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ZaloSessionStatus {
    pub imei: String,
    pub is_valid: bool,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedZaloAccount {
    pub account_key: String,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub auth_status: String,
    pub auth_checked_at: Option<String>,
    pub auth_error: Option<String>,
    pub runtime_status: String,
    pub last_connected_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedFacebookAccount {
    pub channel_account_id: i64,
    pub page_id: String,
    pub page_name: Option<String>,
    pub avatar_url: Option<String>,
    pub token_status: String,
    pub granted_scopes: Vec<String>,
    pub runtime_status: String,
    pub last_connected_at: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSmtpAccount {
    pub id: i32,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub from_email: String,
    pub from_name: String,
    pub security: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmtpServer {
    pub id: Option<i32>,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
    pub security: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailTemplate {
    pub id: Option<i32>,
    pub name: String,
    pub subject: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailOutreachHistory {
    pub id: Option<i32>,
    pub lead_id: i32,
    pub template_id: i32,
    pub smtp_id: i32,
    pub status: String,
    pub error_message: Option<String>,
    pub sent_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct EmailOutreachProgressPayload {
    pub progress: f64,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct EmailOutreachLogPayload {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct EmailOutreachCompletePayload {
    pub total_sent: i32,
    pub message: String,
}
