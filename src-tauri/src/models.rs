use serde::{Serialize, Deserialize};

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
