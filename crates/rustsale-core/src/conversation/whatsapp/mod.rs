pub mod campaign;
pub mod client;
pub mod login;
pub mod migration;
pub mod normalizer;
pub mod phone;
pub mod spintax;
pub mod supervisor;

#[cfg(test)]
mod campaign_test;
#[cfg(test)]
mod login_test;
#[cfg(test)]
mod migration_test;
#[cfg(test)]
mod normalizer_test;
#[cfg(test)]
mod phone_test;
#[cfg(test)]
mod spintax_test;
#[cfg(test)]
mod supervisor_test;
