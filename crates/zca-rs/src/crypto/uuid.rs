use md5::{Digest, Md5};
use uuid::Uuid;

/// Generates a unique Zalo UUID based on the User-Agent.
pub fn generate_zalo_uuid(user_agent: &str) -> String {
    let u = Uuid::new_v4();
    let mut hasher = Md5::new();
    hasher.update(user_agent.as_bytes());
    let digest = hasher.finalize();
    format!("{}-{:x}", u, digest)
}
