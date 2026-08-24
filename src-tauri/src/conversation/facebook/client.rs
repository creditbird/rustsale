use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookAttachmentNode {
    pub id: Option<String>,
    pub mime_type: Option<String>,
    pub name: Option<String>,
    pub size: Option<i64>,
    pub file_url: Option<String>,
    pub image_data: Option<FacebookImageData>,
    pub video_data: Option<FacebookVideoData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookImageData {
    pub url: Option<String>,
    pub preview_url: Option<String>,
    pub max_width: Option<i64>,
    pub max_height: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookVideoData {
    pub url: Option<String>,
    pub preview_url: Option<String>,
    pub length: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookParticipantNode {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookMessageNode {
    pub id: String,
    pub created_time: String,
    pub from: Option<FacebookParticipantNode>,
    pub to: Option<FacebookParticipantsContainer>,
    pub message: Option<String>,
    pub attachments: Option<FacebookAttachmentsContainer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookParticipantsContainer {
    #[serde(default)]
    pub data: Vec<FacebookParticipantNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookAttachmentsContainer {
    #[serde(default)]
    pub data: Vec<FacebookAttachmentNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookMessagesContainer {
    #[serde(default)]
    pub data: Vec<FacebookMessageNode>,
    pub paging: Option<FacebookPaging>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookConversationThread {
    pub id: String,
    pub updated_time: String,
    pub participants: Option<FacebookParticipantsContainer>,
    pub messages: Option<FacebookMessagesContainer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookConversationsPage {
    #[serde(default)]
    pub data: Vec<FacebookConversationThread>,
    pub paging: Option<FacebookPaging>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookMessagesPage {
    #[serde(default)]
    pub data: Vec<FacebookMessageNode>,
    pub paging: Option<FacebookPaging>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookPaging {
    pub cursors: Option<FacebookCursors>,
    pub next: Option<String>,
    pub previous: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookCursors {
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendFacebookMessageResponse {
    pub recipient_id: String,
    pub message_id: String,
}

#[async_trait]
pub trait FacebookGraphClient: Send + Sync {
    async fn fetch_page_conversations(
        &self,
        page_id: &str,
        page_token: &str,
        limit: usize,
    ) -> Result<FacebookConversationsPage>;

    async fn fetch_thread_messages(
        &self,
        thread_id: &str,
        page_token: &str,
        limit: usize,
    ) -> Result<Vec<FacebookMessageNode>>;

    async fn send_messenger_message(
        &self,
        page_id: &str,
        page_token: &str,
        recipient_psid: &str,
        text: &str,
        message_tag: Option<&str>,
    ) -> Result<SendFacebookMessageResponse>;
}

pub struct RealFacebookGraphClient {
    client: Client,
}

impl Default for RealFacebookGraphClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RealFacebookGraphClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }
}

#[async_trait]
impl FacebookGraphClient for RealFacebookGraphClient {
    async fn fetch_page_conversations(
        &self,
        page_id: &str,
        page_token: &str,
        limit: usize,
    ) -> Result<FacebookConversationsPage> {
        let limit_clamped = limit.clamp(1, 100);
        let url = format!(
            "https://graph.facebook.com/v26.0/{}/conversations?fields=id,updated_time,participants,messages{{id,created_time,from,to,message,attachments}}&limit={}&access_token={}",
            urlencoding::encode(page_id),
            limit_clamped,
            urlencoding::encode(page_token)
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow!("Network error fetching conversations for page {}: {}", page_id, e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Meta Graph API error (status {}): {}",
                status,
                body
            ));
        }

        let parsed: FacebookConversationsPage = resp
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse conversations response JSON: {}", e))?;

        Ok(parsed)
    }

    async fn fetch_thread_messages(
        &self,
        thread_id: &str,
        page_token: &str,
        limit: usize,
    ) -> Result<Vec<FacebookMessageNode>> {
        let limit_clamped = limit.clamp(1, 100);
        let url = format!(
            "https://graph.facebook.com/v26.0/{}/messages?fields=id,created_time,from,to,message,attachments&limit={}&access_token={}",
            urlencoding::encode(thread_id),
            limit_clamped,
            urlencoding::encode(page_token)
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow!("Network error fetching thread {}: {}", thread_id, e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Meta Graph API error (status {}): {}",
                status,
                body
            ));
        }

        let page: FacebookMessagesPage = resp
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse thread messages response JSON: {}", e))?;

        Ok(page.data)
    }

    async fn send_messenger_message(
        &self,
        page_id: &str,
        page_token: &str,
        recipient_psid: &str,
        text: &str,
        message_tag: Option<&str>,
    ) -> Result<SendFacebookMessageResponse> {
        let url = format!(
            "https://graph.facebook.com/v26.0/{}/messages",
            urlencoding::encode(page_id)
        );

        let mut payload = serde_json::json!({
            "recipient": {
                "id": recipient_psid
            },
            "message": {
                "text": text
            }
        });

        if let Some(tag) = message_tag {
            if !tag.trim().is_empty() {
                payload["messaging_type"] = serde_json::json!("MESSAGE_TAG");
                payload["tag"] = serde_json::json!(tag.trim());
            }
        } else {
            payload["messaging_type"] = serde_json::json!("RESPONSE");
        }

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", page_token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| anyhow!("Network error sending message to {}: {}", recipient_psid, e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body: Value = resp.json().await.unwrap_or_else(|_| serde_json::json!({}));
            let error_msg = body
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown Meta API error");
            let error_code = body
                .get("error")
                .and_then(|e| e.get("code"))
                .and_then(|c| c.as_i64())
                .unwrap_or(status.as_u16() as i64);

            return Err(anyhow!(
                "Meta Graph API error (code {}): {}",
                error_code,
                error_msg
            ));
        }

        let body: Value = resp
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse send message response JSON: {}", e))?;

        let recipient_id = body
            .get("recipient_id")
            .and_then(|v| v.as_str())
            .unwrap_or(recipient_psid)
            .to_string();

        let message_id = body
            .get("message_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing message_id in Meta send response"))?
            .to_string();

        Ok(SendFacebookMessageResponse {
            recipient_id,
            message_id,
        })
    }
}
