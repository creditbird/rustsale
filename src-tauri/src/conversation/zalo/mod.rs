pub mod client;
pub mod normalizer;
pub mod probe;
pub mod supervisor;

pub(crate) const DIRECT_MESSAGE_WATERMARK_SCOPE: &str = "direct_messages";
pub(crate) const GROUP_MESSAGE_WATERMARK_SCOPE: &str = "group_messages";
pub(crate) const DIRECT_REACTION_WATERMARK_SCOPE: &str = "direct_reactions";
pub(crate) const GROUP_REACTION_WATERMARK_SCOPE: &str = "group_reactions";
pub(crate) const DIRECT_MESSAGE_HISTORY_SCOPE: &str = "direct_message_history";
pub(crate) const GROUP_MESSAGE_HISTORY_SCOPE: &str = "group_message_history";
pub(crate) const DIRECT_REACTION_HISTORY_SCOPE: &str = "direct_reaction_history";
pub(crate) const GROUP_REACTION_HISTORY_SCOPE: &str = "group_reaction_history";

#[cfg(test)]
mod client_test;
#[cfg(test)]
mod normalizer_test;
#[cfg(test)]
mod probe_test;
#[cfg(test)]
mod supervisor_test;
