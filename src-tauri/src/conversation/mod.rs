pub mod adapter;
pub mod commands;
pub mod echo_cache;
pub mod error;
pub mod events;
pub mod forwarding;
pub mod mentions;
pub mod migration;
pub mod migration_v2;
pub mod outreach;
pub mod projection;
pub mod quick_templates;
pub mod reactions;
pub mod repository;
pub mod retry;
pub mod service;
pub mod staging;
pub mod types;
pub mod zalo;

#[cfg(test)]
mod commands_test;
#[cfg(test)]
mod forwarding_test;
#[cfg(test)]
mod mentions_test;
#[cfg(test)]
mod migration_test;
#[cfg(test)]
mod migration_v2_test;
#[cfg(test)]
mod outreach_test;
#[cfg(test)]
mod quick_templates_test;
#[cfg(test)]
mod reactions_test;
#[cfg(test)]
mod repository_test;
#[cfg(test)]
mod retry_test;
#[cfg(test)]
pub mod service_test;
#[cfg(test)]
mod staging_test;
