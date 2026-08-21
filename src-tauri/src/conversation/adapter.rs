use super::types::{Channel, ConversationReactionCode};
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSendRequest {
    pub external_thread_id: String,
    pub is_group: bool,
    pub text: String,
    pub message_tag: Option<String>,
}

impl ProviderSendRequest {
    pub fn new(external_thread_id: impl Into<String>, is_group: bool, text: impl Into<String>) -> Self {
        Self {
            external_thread_id: external_thread_id.into(),
            is_group,
            text: text.into(),
            message_tag: None,
        }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.message_tag = Some(tag.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSendResult {
    pub provider_message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderReactionRequest {
    pub external_thread_id: String,
    pub is_group: bool,
    pub provider_message_id: String,
    pub reaction_code: Option<ConversationReactionCode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderReactionResult {
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderForwardRequest {
    pub target_external_thread_id: String,
    pub is_group: bool,
    pub source_provider_message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderForwardResult {
    pub provider_message_id: String,
}

#[async_trait]
pub trait ChannelSender: Send + Sync {
    fn channel(&self) -> Channel;

    async fn send(
        &self,
        provider_account_key: &str,
        request: ProviderSendRequest,
    ) -> anyhow::Result<ProviderSendResult>;

    async fn set_reaction(
        &self,
        _provider_account_key: &str,
        _request: ProviderReactionRequest,
    ) -> anyhow::Result<ProviderReactionResult> {
        anyhow::bail!(
            "reactions are not supported for channel {}",
            self.channel().as_str()
        );
    }

    async fn forward_message(
        &self,
        _provider_account_key: &str,
        _request: ProviderForwardRequest,
    ) -> anyhow::Result<ProviderForwardResult> {
        anyhow::bail!(
            "message forwarding is not supported for channel {}",
            self.channel().as_str()
        );
    }
}
