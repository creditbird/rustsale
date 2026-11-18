//! Per-client bearer tokens. The clients file stores only SHA-256 hashes and is
//! re-read whenever its mtime changes, so revoke/rotate needs no restart.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const SCOPE_INBOX: &str = "inbox";
pub const SCOPE_TIMELINE: &str = "timeline";
pub const SCOPE_READ: &str = "read";
pub const SCOPE_SEND: &str = "send";
const ALL_SCOPES: [&str; 4] = [SCOPE_INBOX, SCOPE_TIMELINE, SCOPE_READ, SCOPE_SEND];
pub const LEGACY_CLIENT: &str = "legacy-env";

#[derive(Debug, Clone, Deserialize)]
pub struct ClientEntry {
    pub name: String,
    pub sha256: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Clone)]
pub struct Client {
    pub name: String,
    pub scopes: Vec<String>,
}

impl Client {
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

#[derive(Default)]
struct Cache {
    mtime: Option<SystemTime>,
    entries: Vec<ClientEntry>,
}

pub struct ClientStore {
    path: Option<PathBuf>,
    legacy_hash: Option<[u8; 32]>,
    cache: Mutex<Cache>,
}

pub fn sha256(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn decode_hex32(hex: &str) -> Option<[u8; 32]> {
    let hex = hex.trim();
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

impl ClientStore {
    pub fn new(path: Option<PathBuf>, legacy_token: Option<&str>) -> Self {
        Self {
            path,
            legacy_hash: legacy_token
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(sha256),
            cache: Mutex::new(Cache::default()),
        }
    }

    fn refresh(&self, cache: &mut Cache) {
        let Some(path) = &self.path else { return };
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if mtime == cache.mtime {
            return;
        }
        cache.mtime = mtime;
        cache.entries = match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<Vec<ClientEntry>>(&bytes) {
                Ok(entries) => {
                    tracing::info!(target: "local_api", clients = entries.len(), "clients file loaded");
                    entries
                }
                Err(error) => {
                    tracing::error!(target: "local_api", %error, "clients file invalid; no file clients active");
                    Vec::new()
                }
            },
            Err(_) => Vec::new(),
        };
    }

    /// Returns the client owning `token`, or `None` if unknown/disabled.
    pub fn authenticate(&self, token: &str) -> Option<Client> {
        let hash = sha256(token);
        // Check every entry (no early exit) to keep timing independent of position.
        let mut found: Option<Client> = None;
        {
            let mut cache = match self.cache.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            self.refresh(&mut cache);
            for entry in &cache.entries {
                let matches = decode_hex32(&entry.sha256)
                    .map(|stored| constant_time_eq(&stored, &hash))
                    .unwrap_or(false);
                if matches && !entry.disabled && found.is_none() {
                    found = Some(Client {
                        name: entry.name.clone(),
                        scopes: entry.scopes.clone(),
                    });
                }
            }
        }
        if found.is_some() {
            return found;
        }
        match &self.legacy_hash {
            Some(legacy) if constant_time_eq(legacy, &hash) => Some(Client {
                name: LEGACY_CLIENT.to_string(),
                scopes: ALL_SCOPES.iter().map(|s| (*s).to_string()).collect(),
            }),
            _ => None,
        }
    }
}
