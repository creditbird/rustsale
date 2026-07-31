use super::types::Channel;
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSendRequest {
    pub external_thread_id: String,
    pub is_group: bool,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSendResult {
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
}
