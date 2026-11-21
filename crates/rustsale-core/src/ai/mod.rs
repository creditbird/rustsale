pub mod commands;
pub mod evidence;
pub mod gateway;
pub mod generator;
pub mod intent;
pub mod policy;

#[cfg(test)]
mod tests;

pub use commands::*;
pub use evidence::{EvidenceExtractor, LeadEvidenceContext, ProductEvidence};
pub use gateway::{LlmConfig, LlmGateway, LlmMessage, LlmProvider, LlmResponse};
pub use generator::{SmartReplyGenerator, SmartReplyOutput, SmartReplyVariant};
pub use intent::{CustomerIntent, IntentClassifier, IntentResult};
pub use policy::{PolicyConfig, PolicyDecision, PolicyGate};
