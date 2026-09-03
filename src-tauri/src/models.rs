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
pub struct SaveFacebookAccountRequest {
    pub page_id: String,
    pub page_name: Option<String>,
    pub avatar_url: Option<String>,
    pub access_token: String,
    pub granted_scopes: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredFacebookPage {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub avatar_url: Option<String>,
    pub access_token: String,
    pub permissions: Vec<String>,
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

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Campaign {
    pub id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub channel_type: String,
    pub status: String,
    pub audience_filter_json: Option<String>,
    pub schedule_config_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CampaignStep {
    pub id: Option<i64>,
    pub campaign_id: i64,
    pub step_order: i32,
    pub channel: String,
    pub delay_minutes: i32,
    pub condition_type: String,
    pub template_content: String,
    pub ai_prompt_override: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub template_id: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CampaignExecution {
    pub id: Option<i64>,
    pub campaign_id: i64,
    pub step_id: i64,
    pub lead_id: i32,
    pub target_phone: Option<String>,
    pub target_account_id: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub scheduled_at: String,
    pub executed_at: Option<String>,
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LeadJourneyState {
    pub id: Option<i64>,
    pub lead_id: i32,
    pub current_stage: String,
    pub active_campaign_id: Option<i64>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub ai_intent_tag: Option<String>,
    pub ai_qualification_score: Option<f64>,
    pub notes: Option<String>,
    pub updated_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AiEvidenceLog {
    pub id: Option<i64>,
    pub lead_id: Option<i32>,
    pub conversation_id: Option<i64>,
    pub action_type: String,
    pub evidence_source: String,
    pub evidence_data_json: String,
    pub confidence_score: f64,
    pub decision: String,
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AiActionProposal {
    pub id: Option<i64>,
    pub lead_id: Option<i32>,
    pub conversation_id: Option<i64>,
    pub platform: String,
    pub proposed_content: String,
    pub reasoning: String,
    pub status: String,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TemplateFolderRecord {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TemplateRecord {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub name: String,
    pub channel: String,
    pub stage: Option<String>,
    pub subject: Option<String>,
    pub content: String,
    pub media_url: Option<String>,
    pub ai_prompt: Option<String>,
    pub tags_json: Option<String>,
    pub usage_count: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateTemplateInput {
    pub folder_id: Option<i64>,
    pub name: String,
    pub channel: String,
    pub stage: Option<String>,
    pub subject: Option<String>,
    pub content: String,
    pub media_url: Option<String>,
    pub ai_prompt: Option<String>,
    pub tags_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateTemplateInput {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub name: String,
    pub channel: String,
    pub stage: Option<String>,
    pub subject: Option<String>,
    pub content: String,
    pub media_url: Option<String>,
    pub ai_prompt: Option<String>,
    pub tags_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TemplateFilter {
    pub channel: Option<String>,
    pub folder_id: Option<i64>,
    pub stage: Option<String>,
    pub search_query: Option<String>,
    pub tag: Option<String>,
}

