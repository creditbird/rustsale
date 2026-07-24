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
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OutreachTemplate {
    pub id: Option<i32>,
    pub name: String,
    pub content: String,
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZaloSession {
    pub imei: String,
    pub user_agent: String,
    pub cookies_json: String,
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ZaloSessionStatus {
    pub imei: String,
    pub is_valid: bool,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub error_message: Option<String>,
}

