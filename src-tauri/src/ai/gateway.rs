use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    OpenAi,
    Gemini,
    Claude,
    Ollama,
    Mock,
}

impl Default for LlmProvider {
    fn default() -> Self {
        Self::Mock
    }
}

impl LlmProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
            Self::Claude => "claude",
            Self::Ollama => "ollama",
            Self::Mock => "mock",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_name: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub timeout_seconds: u64,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::Mock,
            api_key: None,
            base_url: None,
            model_name: "mock-model".to_string(),
            temperature: 0.7,
            max_tokens: 1024,
            timeout_seconds: 15,
        }
    }
}

impl LlmConfig {
    pub fn from_env() -> Self {
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            if !key.trim().is_empty() {
                return Self {
                    provider: LlmProvider::OpenAi,
                    api_key: Some(key),
                    base_url: std::env::var("OPENAI_BASE_URL").ok(),
                    model_name: std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string()),
                    temperature: 0.7,
                    max_tokens: 1024,
                    timeout_seconds: 15,
                };
            }
        }

        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            if !key.trim().is_empty() {
                return Self {
                    provider: LlmProvider::Gemini,
                    api_key: Some(key),
                    base_url: std::env::var("GEMINI_BASE_URL").ok(),
                    model_name: std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-1.5-flash".to_string()),
                    temperature: 0.7,
                    max_tokens: 1024,
                    timeout_seconds: 15,
                };
            }
        }

        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if !key.trim().is_empty() {
                return Self {
                    provider: LlmProvider::Claude,
                    api_key: Some(key),
                    base_url: std::env::var("ANTHROPIC_BASE_URL").ok(),
                    model_name: std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-3-5-haiku-20241022".to_string()),
                    temperature: 0.7,
                    max_tokens: 1024,
                    timeout_seconds: 15,
                };
            }
        }

        if let Ok(host) = std::env::var("OLLAMA_HOST") {
            if !host.trim().is_empty() {
                return Self {
                    provider: LlmProvider::Ollama,
                    api_key: None,
                    base_url: Some(host),
                    model_name: std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_string()),
                    temperature: 0.7,
                    max_tokens: 1024,
                    timeout_seconds: 30,
                };
            }
        }

        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

impl LlmMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub provider: String,
    pub model: String,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub is_fallback: bool,
}

#[derive(Debug, Clone)]
pub struct LlmGateway {
    config: LlmConfig,
    client: reqwest::Client,
}

impl LlmGateway {
    pub fn new(config: LlmConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .unwrap_or_default();

        Self { config, client }
    }

    pub fn from_env() -> Self {
        Self::new(LlmConfig::from_env())
    }

    pub fn config(&self) -> &LlmConfig {
        &self.config
    }

    pub async fn chat_completion(
        &self,
        messages: &[LlmMessage],
        system_prompt: Option<&str>,
    ) -> Result<LlmResponse, String> {
        match self.config.provider {
            LlmProvider::Mock => Ok(self.generate_mock_response(messages, system_prompt)),
            LlmProvider::OpenAi => {
                match self.call_openai(messages, system_prompt).await {
                    Ok(resp) => Ok(resp),
                    Err(e) => {
                        eprintln!("[LlmGateway] OpenAI call failed: {}, falling back to offline mode", e);
                        Ok(self.generate_mock_response(messages, system_prompt))
                    }
                }
            }
            LlmProvider::Gemini => {
                match self.call_gemini(messages, system_prompt).await {
                    Ok(resp) => Ok(resp),
                    Err(e) => {
                        eprintln!("[LlmGateway] Gemini call failed: {}, falling back to offline mode", e);
                        Ok(self.generate_mock_response(messages, system_prompt))
                    }
                }
            }
            LlmProvider::Claude => {
                match self.call_claude(messages, system_prompt).await {
                    Ok(resp) => Ok(resp),
                    Err(e) => {
                        eprintln!("[LlmGateway] Claude call failed: {}, falling back to offline mode", e);
                        Ok(self.generate_mock_response(messages, system_prompt))
                    }
                }
            }
            LlmProvider::Ollama => {
                match self.call_ollama(messages, system_prompt).await {
                    Ok(resp) => Ok(resp),
                    Err(e) => {
                        eprintln!("[LlmGateway] Ollama call failed: {}, falling back to offline mode", e);
                        Ok(self.generate_mock_response(messages, system_prompt))
                    }
                }
            }
        }
    }

    async fn call_openai(
        &self,
        messages: &[LlmMessage],
        system_prompt: Option<&str>,
    ) -> Result<LlmResponse, String> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or_else(|| "OpenAI API key missing".to_string())?;

        let url = self
            .config
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1/chat/completions");

        let mut payload_messages = Vec::new();
        if let Some(sys) = system_prompt {
            payload_messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }
        for msg in messages {
            payload_messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        let body = serde_json::json!({
            "model": self.config.model_name,
            "messages": payload_messages,
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens
        });

        let resp = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("OpenAI HTTP {}: {}", status, text));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let prompt_tokens = json["usage"]["prompt_tokens"].as_u64().map(|v| v as u32);
        let completion_tokens = json["usage"]["completion_tokens"].as_u64().map(|v| v as u32);

        Ok(LlmResponse {
            content,
            provider: "openai".to_string(),
            model: self.config.model_name.clone(),
            prompt_tokens,
            completion_tokens,
            is_fallback: false,
        })
    }

    async fn call_gemini(
        &self,
        messages: &[LlmMessage],
        system_prompt: Option<&str>,
    ) -> Result<LlmResponse, String> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or_else(|| "Gemini API key missing".to_string())?;

        let base = self
            .config
            .base_url
            .as_deref()
            .unwrap_or("https://generativelanguage.googleapis.com/v1beta/models");

        let url = format!(
            "{}/{}:generateContent?key={}",
            base.trim_end_matches('/'),
            self.config.model_name,
            api_key
        );

        let mut parts = Vec::new();
        if let Some(sys) = system_prompt {
            parts.push(serde_json::json!({ "text": format!("System instructions: {}\n", sys) }));
        }
        for msg in messages {
            parts.push(serde_json::json!({
                "text": format!("{}: {}", msg.role, msg.content)
            }));
        }

        let body = serde_json::json!({
            "contents": [{
                "parts": parts
            }],
            "generationConfig": {
                "temperature": self.config.temperature,
                "maxOutputTokens": self.config.max_tokens
            }
        });

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Gemini network error: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Gemini HTTP {}: {}", status, text));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Gemini response: {}", e))?;

        let content = json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(LlmResponse {
            content,
            provider: "gemini".to_string(),
            model: self.config.model_name.clone(),
            prompt_tokens: None,
            completion_tokens: None,
            is_fallback: false,
        })
    }

    async fn call_claude(
        &self,
        messages: &[LlmMessage],
        system_prompt: Option<&str>,
    ) -> Result<LlmResponse, String> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or_else(|| "Claude API key missing".to_string())?;

        let url = self
            .config
            .base_url
            .as_deref()
            .unwrap_or("https://api.anthropic.com/v1/messages");

        let mut payload_messages = Vec::new();
        for msg in messages {
            let role = if msg.role == "assistant" { "assistant" } else { "user" };
            payload_messages.push(serde_json::json!({
                "role": role,
                "content": msg.content
            }));
        }

        let mut body = serde_json::json!({
            "model": self.config.model_name,
            "messages": payload_messages,
            "max_tokens": self.config.max_tokens,
            "temperature": self.config.temperature
        });

        if let Some(sys) = system_prompt {
            body["system"] = serde_json::json!(sys);
        }

        let resp = self
            .client
            .post(url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Claude network error: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Claude HTTP {}: {}", status, text));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Claude response: {}", e))?;

        let content = json["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let prompt_tokens = json["usage"]["input_tokens"].as_u64().map(|v| v as u32);
        let completion_tokens = json["usage"]["output_tokens"].as_u64().map(|v| v as u32);

        Ok(LlmResponse {
            content,
            provider: "claude".to_string(),
            model: self.config.model_name.clone(),
            prompt_tokens,
            completion_tokens,
            is_fallback: false,
        })
    }

    async fn call_ollama(
        &self,
        messages: &[LlmMessage],
        system_prompt: Option<&str>,
    ) -> Result<LlmResponse, String> {
        let base = self
            .config
            .base_url
            .as_deref()
            .unwrap_or("http://localhost:11434");

        let url = format!("{}/api/chat", base.trim_end_matches('/'));

        let mut payload_messages = Vec::new();
        if let Some(sys) = system_prompt {
            payload_messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }
        for msg in messages {
            payload_messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        let body = serde_json::json!({
            "model": self.config.model_name,
            "messages": payload_messages,
            "stream": false,
            "options": {
                "temperature": self.config.temperature
            }
        });

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Ollama network error: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Ollama HTTP {}: {}", status, text));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        let content = json["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(LlmResponse {
            content,
            provider: "ollama".to_string(),
            model: self.config.model_name.clone(),
            prompt_tokens: None,
            completion_tokens: None,
            is_fallback: false,
        })
    }

    fn generate_mock_response(
        &self,
        messages: &[LlmMessage],
        _system_prompt: Option<&str>,
    ) -> LlmResponse {
        let last_user_message = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("");

        let mock_json = serde_json::json!({
            "variants": [
                {
                    "variant_type": "brief",
                    "title": "Ngắn gọn / Thân thiện",
                    "content": "Dạ em chào anh/chị, em đã nhận được thông tin và sẽ hỗ trợ ngay ạ!",
                    "suggested_action": "Gửi tin nhắn nhanh",
                    "confidence": 0.95
                },
                {
                    "variant_type": "detailed",
                    "title": "Chi tiết / Đầy đủ",
                    "content": format!("Dạ em chào anh/chị, về yêu cầu \"{}\", em xin phép gửi thông tin chi tiết kèm báo giá chuẩn qua tin nhắn này để anh/chị tham khảo ạ.", last_user_message),
                    "suggested_action": "Gửi kèm bảng giá",
                    "confidence": 0.90
                },
                {
                    "variant_type": "promotional",
                    "title": "Khuyến mãi / Kêu gọi hành động",
                    "content": "Dạ hiện bên em đang có ưu đãi chiết khấu đặc biệt trong tuần này. Anh/chị cho em xin lịch hẹn hoặc số điện thoại để chuyên viên tư vấn chi tiết cho mình nhé!",
                    "suggested_action": "Đặt lịch hẹn tư vấn",
                    "confidence": 0.88
                }
            ]
        });

        LlmResponse {
            content: mock_json.to_string(),
            provider: "mock".to_string(),
            model: "mock-offline-engine".to_string(),
            prompt_tokens: Some(100),
            completion_tokens: Some(250),
            is_fallback: true,
        }
    }
}
