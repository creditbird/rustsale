use anyhow::Result;
use base64::Engine as _;
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::phone;
use crate::conversation::repository::{
    NormalizedAttachment, NormalizedIdentity, NormalizedMessage, NormalizedReplyReference,
};
use crate::conversation::types::{Channel, ConversationKind, MessageDirection, MessageStatus};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawWhatsAppAttachment {
    pub media_type: String, // "image", "video", "audio", "file" / "document", "sticker"
    pub mime_type: Option<String>,
    pub file_name: Option<String>,
    pub size_bytes: Option<u64>,
    pub url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub direct_path: Option<String>,
    #[serde(default)]
    pub media_key: Option<String>,
    #[serde(default)]
    pub file_sha256: Option<String>,
    #[serde(default)]
    pub file_enc_sha256: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawWhatsAppQuote {
    pub stanza_id: String,
    pub participant_jid: String,
    pub text: Option<String>,
    pub media_type: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawWhatsAppMessage {
    pub id: String,
    pub chat_jid: String,
    pub sender_jid: Option<String>,
    pub sender_name: Option<String>,
    pub text: Option<String>,
    pub timestamp: Option<String>,
    pub is_from_me: bool,
    #[serde(default)]
    pub attachments: Vec<RawWhatsAppAttachment>,
    pub quote: Option<RawWhatsAppQuote>,
    pub raw_json: Option<serde_json::Value>,
    #[serde(default)]
    pub mentions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawWhatsAppReaction {
    pub message_id: String,
    pub chat_jid: String,
    pub sender_jid: String,
    pub emoji: String,
    pub timestamp: Option<String>,
    pub is_from_me: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawWhatsAppReceipt {
    pub message_ids: Vec<String>,
    pub chat_jid: String,
    pub sender_jid: Option<String>,
    pub receipt_type: String, // "delivered", "read", "played", "sent", "failed"
    pub timestamp: Option<String>,
}

#[derive(Debug)]
struct StoredWhatsAppAttachment {
    id: i64,
    mime_type: Option<String>,
    file_name: Option<String>,
    size_bytes: Option<i64>,
    remote_url: Option<String>,
    local_path: Option<String>,
    thumbnail_url: Option<String>,
    metadata_json: Option<String>,
}

fn canonical_whatsapp_media_type(media_type: &str) -> &'static str {
    match media_type.to_lowercase().as_str() {
        "image" | "photo" => "image",
        "video" => "video",
        "audio" | "voice" | "ptt" => "audio",
        "sticker" => "sticker",
        _ => "file",
    }
}

fn default_whatsapp_mime_type(media_type: &str) -> &'static str {
    match media_type {
        "image" => "image/jpeg",
        "video" => "video/mp4",
        "audio" => "audio/ogg",
        "sticker" => "image/webp",
        _ => "application/octet-stream",
    }
}

fn put_non_empty_string(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        object.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
}

fn whatsapp_attachment_metadata(
    attachment: &RawWhatsAppAttachment,
    media_type: &str,
    mime_type: &str,
) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    object.insert(
        "media_type".to_string(),
        serde_json::Value::String(media_type.to_string()),
    );
    object.insert(
        "mime_type".to_string(),
        serde_json::Value::String(mime_type.to_string()),
    );
    put_non_empty_string(
        &mut object,
        "direct_path",
        attachment.direct_path.as_deref(),
    );
    put_non_empty_string(&mut object, "media_key", attachment.media_key.as_deref());
    put_non_empty_string(
        &mut object,
        "file_sha256",
        attachment.file_sha256.as_deref(),
    );
    put_non_empty_string(
        &mut object,
        "file_enc_sha256",
        attachment.file_enc_sha256.as_deref(),
    );
    if let Some(width) = attachment.width {
        object.insert("width".to_string(), serde_json::json!(width));
    }
    if let Some(height) = attachment.height {
        object.insert("height".to_string(), serde_json::json!(height));
    }
    serde_json::Value::Object(object)
}

/// Applies a JSON overlay without treating `null` as a delete operation.
/// WhatsApp history snapshots can omit media secrets that were present on a live event, so
/// RFC 7396 `json_patch` semantics would otherwise erase the only decryptable copy.
fn merge_non_null_json(target: &mut serde_json::Value, patch: &serde_json::Value) {
    match patch {
        serde_json::Value::Null => {}
        serde_json::Value::Object(patch_object) => {
            if !target.is_object() {
                *target = serde_json::Value::Object(serde_json::Map::new());
            }
            if let Some(target_object) = target.as_object_mut() {
                for (key, patch_value) in patch_object {
                    if patch_value.is_null() {
                        continue;
                    }
                    if let Some(target_value) = target_object.get_mut(key) {
                        merge_non_null_json(target_value, patch_value);
                    } else {
                        target_object.insert(key.clone(), patch_value.clone());
                    }
                }
            }
        }
        serde_json::Value::Array(patch_items) => {
            if let Some(target_items) = target.as_array_mut() {
                for (index, patch_item) in patch_items.iter().enumerate() {
                    if let Some(target_item) = target_items.get_mut(index) {
                        merge_non_null_json(target_item, patch_item);
                    } else if !patch_item.is_null() {
                        target_items.push(patch_item.clone());
                    }
                }
            } else {
                *target = patch.clone();
            }
        }
        _ => *target = patch.clone(),
    }
}

fn merge_raw_payload_json(existing: Option<&str>, incoming: &str) -> String {
    let Some(existing) = existing else {
        return incoming.to_string();
    };
    let Ok(mut merged) = serde_json::from_str::<serde_json::Value>(existing) else {
        return incoming.to_string();
    };
    let Ok(incoming_value) = serde_json::from_str::<serde_json::Value>(incoming) else {
        return existing.to_string();
    };
    merge_non_null_json(&mut merged, &incoming_value);
    merged.to_string()
}

fn is_whatsapp_remote_reference(path: &str) -> bool {
    let normalized = path.trim().to_ascii_lowercase();
    normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("/o1/")
        || normalized.starts_with("/v/t62")
        || normalized.starts_with("/mms/")
        || normalized.contains("mmg.whatsapp.net")
}

fn upsert_whatsapp_attachments(
    tx: &Transaction<'_>,
    message_id: i64,
    attachments: &[RawWhatsAppAttachment],
    now: &str,
) -> Result<()> {
    for attachment in attachments {
        let media_type = canonical_whatsapp_media_type(&attachment.media_type);
        let mime_type = attachment
            .mime_type
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_whatsapp_mime_type(media_type));

        // A WhatsApp stanza contains at most one top-level media payload. Matching the canonical
        // row by stanza + media type lets a refreshed CDN URL enrich, rather than duplicate, it.
        let stored = {
            let mut statement = tx.prepare(
                "SELECT id, mime_type, file_name, size_bytes, remote_url, local_path,
                        thumbnail_url, metadata_json
                 FROM message_attachments
                 WHERE message_id = ?1 AND attachment_type = ?2
                 ORDER BY id",
            )?;
            let rows = statement.query_map(params![message_id, media_type], |row| {
                Ok(StoredWhatsAppAttachment {
                    id: row.get(0)?,
                    mime_type: row.get(1)?,
                    file_name: row.get(2)?,
                    size_bytes: row.get(3)?,
                    remote_url: row.get(4)?,
                    local_path: row.get(5)?,
                    thumbnail_url: row.get(6)?,
                    metadata_json: row.get(7)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut merged_metadata = serde_json::json!({});
        let mut merged_mime_type: Option<String> = None;
        let mut merged_file_name: Option<String> = None;
        let mut merged_size_bytes: Option<i64> = None;
        let mut merged_remote_url: Option<String> = None;
        let mut merged_local_path: Option<String> = None;
        let mut merged_thumbnail_url: Option<String> = None;

        for row in &stored {
            if merged_mime_type.is_none() {
                merged_mime_type = row.mime_type.clone();
            }
            if merged_file_name.is_none() {
                merged_file_name = row.file_name.clone();
            }
            if merged_size_bytes.is_none() {
                merged_size_bytes = row.size_bytes;
            }
            if merged_remote_url.is_none() {
                merged_remote_url = row.remote_url.clone();
            }
            if merged_local_path.is_none() {
                merged_local_path = row
                    .local_path
                    .as_ref()
                    .filter(|path| !is_whatsapp_remote_reference(path))
                    .cloned();
            }
            if merged_thumbnail_url.is_none() {
                merged_thumbnail_url = row.thumbnail_url.clone();
            }
            if let Some(metadata_json) = row.metadata_json.as_deref() {
                if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(metadata_json) {
                    merge_non_null_json(&mut merged_metadata, &metadata);
                }
            }
        }

        merged_mime_type = Some(mime_type.to_string());
        if let Some(file_name) = attachment
            .file_name
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            merged_file_name = Some(file_name.clone());
        }
        if let Some(size_bytes) = attachment.size_bytes {
            merged_size_bytes = i64::try_from(size_bytes).ok();
        }
        if let Some(remote_url) = attachment
            .url
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            merged_remote_url = Some(remote_url.clone());
        }
        if let Some(thumbnail_url) = attachment
            .thumbnail_url
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            merged_thumbnail_url = Some(thumbnail_url.clone());
        }
        merge_non_null_json(
            &mut merged_metadata,
            &whatsapp_attachment_metadata(attachment, media_type, mime_type),
        );
        let merged_metadata_json = merged_metadata.to_string();

        if let Some(canonical) = stored.first() {
            // Remove rows created by the old URL-based matching bug before replacing the URL on
            // the stable attachment id, avoiding the partial unique index on `(message_id, url)`.
            for duplicate in stored.iter().skip(1) {
                tx.execute(
                    "DELETE FROM message_attachments WHERE id = ?1",
                    [duplicate.id],
                )?;
            }
            tx.execute(
                "UPDATE message_attachments
                 SET mime_type = ?2,
                     file_name = ?3,
                     size_bytes = ?4,
                     remote_url = ?5,
                     local_path = ?6,
                     thumbnail_url = ?7,
                     metadata_json = ?8
                 WHERE id = ?1",
                params![
                    canonical.id,
                    merged_mime_type,
                    merged_file_name,
                    merged_size_bytes,
                    merged_remote_url,
                    merged_local_path,
                    merged_thumbnail_url,
                    merged_metadata_json,
                ],
            )?;
        } else {
            tx.execute(
                "INSERT INTO message_attachments (
                    message_id, provider_attachment_id, attachment_type, file_name,
                    mime_type, size_bytes, remote_url, local_path, thumbnail_url,
                    metadata_json, created_at
                 ) VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9)",
                params![
                    message_id,
                    media_type,
                    merged_file_name,
                    merged_mime_type,
                    merged_size_bytes,
                    merged_remote_url,
                    merged_thumbnail_url,
                    merged_metadata_json,
                    now,
                ],
            )?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct AppliedWhatsAppMessage {
    pub conversation_id: i64,
    pub lead_id: Option<i64>,
    pub message_id: i64,
    pub delta_event: Option<crate::conversation::events::ConversationMessageDeltaEvent>,
}

impl PartialEq for AppliedWhatsAppMessage {
    fn eq(&self, other: &Self) -> bool {
        self.conversation_id == other.conversation_id
            && self.lead_id == other.lead_id
            && self.message_id == other.message_id
    }
}

impl std::ops::Deref for AppliedWhatsAppMessage {
    type Target = i64;
    fn deref(&self) -> &Self::Target {
        &self.conversation_id
    }
}

impl rusqlite::ToSql for AppliedWhatsAppMessage {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::from(self.conversation_id))
    }
}

impl PartialEq<i64> for AppliedWhatsAppMessage {
    fn eq(&self, other: &i64) -> bool {
        self.conversation_id == *other
    }
}

impl PartialEq<AppliedWhatsAppMessage> for i64 {
    fn eq(&self, other: &AppliedWhatsAppMessage) -> bool {
        *self == other.conversation_id
    }
}

impl PartialOrd<i64> for AppliedWhatsAppMessage {
    fn partial_cmp(&self, other: &i64) -> Option<std::cmp::Ordering> {
        self.conversation_id.partial_cmp(other)
    }
}

impl PartialOrd<AppliedWhatsAppMessage> for i64 {
    fn partial_cmp(&self, other: &AppliedWhatsAppMessage) -> Option<std::cmp::Ordering> {
        self.partial_cmp(&other.conversation_id)
    }
}

impl From<AppliedWhatsAppMessage> for i64 {
    fn from(msg: AppliedWhatsAppMessage) -> Self {
        msg.conversation_id
    }
}

impl From<&AppliedWhatsAppMessage> for i64 {
    fn from(msg: &AppliedWhatsAppMessage) -> Self {
        msg.conversation_id
    }
}

/// A CRM lead's phone, prepared once so many lookups do not redo the work.
#[derive(Debug, Clone)]
pub struct LeadPhone {
    id: i64,
    /// ASCII digits of the phone (never empty).
    digits: String,
    /// The phone as E.164 digits with the default Vietnam prefix, when it is a phone.
    e164: Option<String>,
}

pub struct WhatsAppNormalizer;

impl WhatsAppNormalizer {
    /// Normalizes an incoming timestamp into ISO-8601 UTC string (RFC 3339 with millis).
    pub fn normalize_iso_timestamp(ts_str: Option<&str>) -> String {
        if let Some(ts) = ts_str {
            let ts = ts.trim();
            if !ts.is_empty() {
                if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
                    return dt
                        .with_timezone(&Utc)
                        .to_rfc3339_opts(SecondsFormat::Millis, true);
                }
                // Check if numeric unix timestamp in milliseconds or seconds
                if let Ok(num) = ts.parse::<i64>() {
                    if num > 1_000_000_000_000 {
                        // Milliseconds
                        if let Some(dt) = DateTime::from_timestamp_millis(num) {
                            return dt
                                .with_timezone(&Utc)
                                .to_rfc3339_opts(SecondsFormat::Millis, true);
                        }
                    } else if num > 0 {
                        // Seconds
                        if let Some(dt) = DateTime::from_timestamp(num, 0) {
                            return dt
                                .with_timezone(&Utc)
                                .to_rfc3339_opts(SecondsFormat::Millis, true);
                        }
                    }
                }
            }
        }
        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    /// Detects canonical message content_type from RawWhatsAppMessage.
    pub fn detect_content_type(msg: &RawWhatsAppMessage) -> &'static str {
        if let Some(first_att) = msg.attachments.first() {
            match first_att.media_type.to_lowercase().as_str() {
                "image" | "photo" => "image",
                "video" => "video",
                "audio" | "voice" | "ptt" => "audio",
                "sticker" => "sticker",
                _ => "file",
            }
        } else {
            "text"
        }
    }

    /// Resolves a CRM lead ID by normalized phone number with strict safety invariants:
    /// - Exactly 1 matching lead -> Some(lead_id)
    /// - 0 matching leads -> None
    /// - >1 matching leads (duplicate phone in CRM) -> None (never guess arbitrarily).
    pub fn resolve_lead_by_phone(conn: &Connection, phone_raw: &str) -> Result<Option<i64>> {
        let Some(target_digits) = Self::lookup_digits(phone_raw) else {
            return Ok(None);
        };
        let leads = Self::load_lead_phones(conn)?;
        Ok(Self::match_lead_phones(&leads, &target_digits))
    }

    /// Same rules as [`Self::resolve_lead_by_phone`] over a list loaded once with
    /// [`Self::load_lead_phones`], so a loop over many numbers reads `leads` once.
    pub fn resolve_lead_by_phone_in(leads: &[LeadPhone], phone_raw: &str) -> Option<i64> {
        let target_digits = Self::lookup_digits(phone_raw)?;
        Self::match_lead_phones(leads, &target_digits)
    }

    /// Leads that have a non-empty phone, with the digit forms the matcher needs
    /// worked out once.
    pub fn load_lead_phones(conn: &Connection) -> rusqlite::Result<Vec<LeadPhone>> {
        let mut stmt = conn
            .prepare("SELECT id, phone FROM leads WHERE phone IS NOT NULL AND TRIM(phone) != ''")?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let phone: String = row.get(1)?;
            Ok((id, phone))
        })?;
        let mut leads = Vec::new();
        for row in rows {
            let (id, phone) = row?;
            let phone = phone.trim();
            if phone.is_empty() {
                continue;
            }
            let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                continue;
            }
            leads.push(LeadPhone {
                id,
                e164: phone::normalize_e164(phone, Some("84")),
                digits,
            });
        }
        Ok(leads)
    }

    /// Digits of the number to look up, or `None` when it is blank or not a phone.
    fn lookup_digits(phone_raw: &str) -> Option<String> {
        let phone_raw = phone_raw.trim();
        if phone_raw.is_empty() {
            return None;
        }

        // Clean user digits or normalized E.164
        let target_e164 = phone::normalize_e164(phone_raw, Some("84"))
            .or_else(|| phone::jid_to_phone(phone_raw))?;
        let target_digits: String = target_e164.chars().filter(|c| c.is_ascii_digit()).collect();
        if target_digits.is_empty() {
            None
        } else {
            Some(target_digits)
        }
    }

    fn match_lead_phones(leads: &[LeadPhone], target_digits: &str) -> Option<i64> {
        let mut matching_lead_ids = HashSet::new();

        for lead in leads {
            let id = lead.id;
            let lead_digits = lead.digits.as_str();

            // 1. Direct digit equality
            if lead_digits == target_digits {
                matching_lead_ids.insert(id);
                continue;
            }

            // 2. Normalized E.164 equality with default Vietnam "84"
            if let Some(norm_lead) = lead.e164.as_deref() {
                if norm_lead == target_digits {
                    matching_lead_ids.insert(id);
                    continue;
                }
            }

            // 3. International prefix "00" stripping (e.g. 0086... -> 86...)
            if let Some(rest) = lead_digits.strip_prefix("00") {
                if rest == target_digits {
                    matching_lead_ids.insert(id);
                    continue;
                }
            }

            // 4. National / domestic format with leading 0 (e.g. 0901234567, 013926842913, 13926842913)
            let sub_lead = lead_digits.strip_prefix('0').unwrap_or(lead_digits);
            if sub_lead.len() >= 7 && target_digits.ends_with(sub_lead) {
                let cc_len = target_digits.len() - sub_lead.len();
                if (1..=4).contains(&cc_len) {
                    matching_lead_ids.insert(id);
                    continue;
                }
            }

            // 5. Vice-versa: target has leading 0 or national format, lead has full international format
            let sub_target = target_digits.strip_prefix('0').unwrap_or(target_digits);
            if sub_target.len() >= 7 && lead_digits.ends_with(sub_target) {
                let cc_len = lead_digits.len() - sub_target.len();
                if (1..=4).contains(&cc_len) {
                    matching_lead_ids.insert(id);
                    continue;
                }
            }
        }

        if matching_lead_ids.len() == 1 {
            matching_lead_ids.into_iter().next()
        } else {
            // 0 or >1 matches (ambiguous) -> None
            None
        }
    }

    /// Converts a `RawWhatsAppMessage` into a unified `NormalizedMessage`.
    pub fn normalize_message(
        provider_account_key: &str,
        account_display_name: Option<&str>,
        msg: &RawWhatsAppMessage,
    ) -> Result<NormalizedMessage> {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let occurred_at = Self::normalize_iso_timestamp(msg.timestamp.as_deref());

        let is_group = phone::is_group_jid(&msg.chat_jid);
        let (conversation_kind, external_thread_id, conversation_title) = if is_group {
            (
                ConversationKind::Group,
                msg.chat_jid.trim().to_string(),
                None,
            )
        } else {
            let peer_phone = phone::jid_to_phone(&msg.chat_jid)
                .unwrap_or_else(|| msg.chat_jid.trim().to_string());
            let normalized_phone =
                phone::normalize_e164(&peer_phone, Some("84")).unwrap_or(peer_phone);
            (
                ConversationKind::Direct,
                phone::phone_to_direct_jid(&normalized_phone),
                msg.sender_name
                    .clone()
                    .or_else(|| Some(format!("+{}", normalized_phone))),
            )
        };

        let participant = if !msg.is_from_me {
            let sender_jid = if is_group {
                msg.sender_jid
                    .as_deref()
                    .filter(|s| !phone::is_group_jid(s))
            } else {
                msg.sender_jid
                    .as_deref()
                    .filter(|s| !phone::is_group_jid(s))
                    .or_else(|| {
                        if phone::is_group_jid(&msg.chat_jid) {
                            None
                        } else {
                            Some(msg.chat_jid.as_str())
                        }
                    })
            };
            if let Some(s_jid) = sender_jid {
                let sender_phone =
                    phone::jid_to_phone(s_jid).or_else(|| phone::normalize_e164(s_jid, Some("84")));
                let ext_id = sender_phone.unwrap_or_else(|| s_jid.to_string());
                Some(NormalizedIdentity {
                    identity_namespace: "phone".to_string(),
                    external_id: ext_id,
                    display_name: msg.sender_name.clone(),
                    avatar_url: None,
                    lead_id: None,
                })
            } else {
                None
            }
        } else {
            None
        };

        let content_type = Self::detect_content_type(msg).to_string();

        let attachments: Vec<NormalizedAttachment> = msg
            .attachments
            .iter()
            .map(|att| {
                let default_mime = match att.media_type.as_str() {
                    "image" | "photo" => "image/jpeg",
                    "video" => "video/mp4",
                    "audio" | "voice" | "ptt" => "audio/ogg",
                    "sticker" => "image/webp",
                    _ => "application/octet-stream",
                };
                NormalizedAttachment {
                    provider_attachment_id: None,
                    media_type: att.media_type.clone(),
                    mime_type: att
                        .mime_type
                        .clone()
                        .unwrap_or_else(|| default_mime.to_string()),
                    file_name: att.file_name.clone(),
                    size_bytes: att.size_bytes,
                    remote_url: att.url.clone(),
                    thumbnail_url: att.thumbnail_url.clone(),
                    metadata_json: Some(serde_json::json!({
                        "direct_path": att.direct_path,
                        "media_key": att.media_key,
                        "file_sha256": att.file_sha256,
                        "file_enc_sha256": att.file_enc_sha256,
                        "width": att.width,
                        "height": att.height,
                    })),
                }
            })
            .collect();

        let reply = msg.quote.as_ref().map(|q| NormalizedReplyReference {
            provider_message_id: q.stanza_id.clone(),
            provider_client_message_id: String::new(),
            sender_external_id: q.participant_jid.clone(),
            occurred_at: Self::normalize_iso_timestamp(q.timestamp.as_deref()),
            text_content: q.text.clone(),
            content_type: q.media_type.clone().unwrap_or_else(|| "text".to_string()),
            raw_quote: serde_json::to_value(q).unwrap_or_default(),
        });

        let content_json = serde_json::json!({
            "text": msg.text.as_deref().unwrap_or(""),
        });

        Ok(NormalizedMessage {
            channel: Channel::WhatsApp,
            provider_account_key: provider_account_key.to_string(),
            account_display_name: account_display_name.map(str::to_string),
            participant,
            sender_is_participant: !msg.is_from_me,
            external_thread_id,
            conversation_kind,
            conversation_title,
            direction: if msg.is_from_me {
                MessageDirection::Outbound
            } else {
                MessageDirection::Inbound
            },
            provider_message_id: (!msg.id.is_empty()).then_some(msg.id.clone()),
            provider_client_message_id: None,
            local_request_id: None,
            content_type,
            text_content: msg.text.clone(),
            content: content_json,
            attachments,
            reply,
            mentions: vec![],
            raw_payload: msg.raw_json.clone(),
            provider_timestamp: Some(occurred_at),
            ingested_at: now,
            status: if msg.is_from_me {
                MessageStatus::Sent
            } else {
                MessageStatus::Received
            },
        })
    }

    /// Ingests an incoming or synchronized WhatsApp message into the SQLite database.
    pub fn ingest_incoming_message(
        conn: &mut Connection,
        channel_account_id: i64,
        msg: &RawWhatsAppMessage,
    ) -> Result<Option<AppliedWhatsAppMessage>> {
        Self::ingest_message_transactional(conn, channel_account_id, msg)
    }

    /// Transactionally ingests a raw WhatsApp message, creating conversations and identities as needed.
    pub fn ingest_message_transactional(
        conn: &mut Connection,
        channel_account_id: i64,
        msg: &RawWhatsAppMessage,
    ) -> Result<Option<AppliedWhatsAppMessage>> {
        let tx = conn.transaction()?;
        let now_str = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let event_time = Self::normalize_iso_timestamp(msg.timestamp.as_deref());

        let is_group = phone::is_group_jid(&msg.chat_jid);

        // Auto-detect and record LID-PN mapping if present in message fields
        if let Some(ref sender_jid) = msg.sender_jid {
            if phone::is_lid_jid(&msg.chat_jid) && phone::is_direct_jid(sender_jid) {
                if let Some(pn) = phone::jid_to_phone(sender_jid) {
                    let _ = phone::save_lid_pn_mapping(&tx, &msg.chat_jid, &pn);
                }
            } else if phone::is_direct_jid(&msg.chat_jid) && phone::is_lid_jid(sender_jid) {
                if let Some(pn) = phone::jid_to_phone(&msg.chat_jid) {
                    let _ = phone::save_lid_pn_mapping(&tx, sender_jid, &pn);
                }
            }
        }

        // 1. Thread & Conversation Resolution
        let (external_thread_id, kind, default_title) = if is_group {
            (
                msg.chat_jid.trim().to_string(),
                "group",
                "WhatsApp Group".to_string(),
            )
        } else {
            let peer_phone = phone::resolve_lid_or_jid_to_phone(&tx, &msg.chat_jid)
                .or_else(|| {
                    msg.sender_jid
                        .as_ref()
                        .and_then(|sj| phone::resolve_lid_or_jid_to_phone(&tx, sj))
                })
                .or_else(|| phone::jid_to_phone(&msg.chat_jid))
                .or_else(|| {
                    msg.sender_jid
                        .as_ref()
                        .and_then(|sj| phone::jid_to_phone(sj))
                })
                .unwrap_or_else(|| msg.chat_jid.trim().to_string());
            let normalized_phone = phone::normalize_e164(&peer_phone, Some("84"))
                .unwrap_or_else(|| peer_phone.clone());
            let direct_thread_id = if phone::is_direct_jid(&msg.chat_jid) {
                msg.chat_jid.trim().to_string()
            } else if normalized_phone.chars().all(|c| c.is_ascii_digit())
                && normalized_phone.len() >= 7
            {
                phone::phone_to_direct_jid(&normalized_phone)
            } else {
                msg.chat_jid.trim().to_string()
            };
            let formatted_title = msg.sender_name.clone().unwrap_or_else(|| {
                if normalized_phone.chars().all(|c| c.is_ascii_digit())
                    && normalized_phone.len() >= 7
                {
                    format!("+{}", normalized_phone)
                } else {
                    normalized_phone.clone()
                }
            });
            (direct_thread_id, "direct", formatted_title)
        };

        // Upsert channel_conversations
        tx.execute(
            "INSERT INTO channel_conversations (
                channel_account_id, external_thread_id, kind, title, avatar_url, status, last_message_at, last_synced_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, NULL, 'active', ?5, ?6, ?6, ?6)
            ON CONFLICT(channel_account_id, external_thread_id, kind) DO UPDATE SET
                title = CASE
                    WHEN EXCLUDED.title IS NOT NULL AND EXCLUDED.title != 'WhatsApp Group' THEN EXCLUDED.title
                    ELSE channel_conversations.title
                END,
                last_message_at = CASE
                    WHEN EXCLUDED.last_message_at IS NOT NULL AND (channel_conversations.last_message_at IS NULL OR EXCLUDED.last_message_at > channel_conversations.last_message_at)
                    THEN EXCLUDED.last_message_at
                    ELSE channel_conversations.last_message_at
                END,
                last_synced_at = EXCLUDED.last_synced_at,
                updated_at = EXCLUDED.updated_at",
            params![
                channel_account_id,
                external_thread_id,
                kind,
                default_title,
                event_time,
                now_str
            ],
        )?;

        let conversation_id: i64 = tx.query_row(
            "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = ?3",
            params![channel_account_id, external_thread_id, kind],
            |row| row.get(0),
        )?;

        // 2. Identity & Lead Resolution
        let mut sender_identity_id: Option<i64> = None;

        if !is_group {
            let peer_phone = phone::resolve_lid_or_jid_to_phone(&tx, &msg.chat_jid)
                .or_else(|| {
                    msg.sender_jid
                        .as_ref()
                        .and_then(|sj| phone::resolve_lid_or_jid_to_phone(&tx, sj))
                })
                .or_else(|| phone::jid_to_phone(&msg.chat_jid))
                .or_else(|| {
                    msg.sender_jid
                        .as_ref()
                        .and_then(|sj| phone::jid_to_phone(sj))
                })
                .unwrap_or_else(|| msg.chat_jid.trim().to_string());
            let normalized_phone =
                phone::normalize_e164(&peer_phone, Some("84")).unwrap_or(peer_phone);

            let lead_id = Self::resolve_lead_by_phone(&tx, &normalized_phone)?;
            let link_source = if lead_id.is_some() {
                "phone_match"
            } else {
                "unlinked"
            };

            let display_name = msg
                .sender_name
                .clone()
                .unwrap_or_else(|| format!("+{}", normalized_phone));

            tx.execute(
                "INSERT INTO channel_identities (
                    lead_id, channel, channel_account_id, identity_namespace, external_id,
                    normalized_address, display_name, link_source, created_at, updated_at
                ) VALUES (
                    ?1, 'whatsapp', ?2, 'phone', ?3,
                    ?4, ?5, ?6, ?7, ?7
                )
                ON CONFLICT(channel, channel_account_id, identity_namespace, external_id) WHERE channel_account_id IS NOT NULL DO UPDATE SET
                    lead_id = EXCLUDED.lead_id,
                    display_name = COALESCE(EXCLUDED.display_name, channel_identities.display_name),
                    link_source = EXCLUDED.link_source,
                    updated_at = EXCLUDED.updated_at",
                params![
                    lead_id,
                    channel_account_id,
                    normalized_phone,
                    normalized_phone,
                    display_name,
                    link_source,
                    now_str
                ],
            )?;

            let identity_id: i64 = tx.query_row(
                "SELECT id FROM channel_identities
                 WHERE channel = 'whatsapp' AND channel_account_id = ?1 AND identity_namespace = 'phone' AND external_id = ?2",
                params![channel_account_id, normalized_phone],
                |row| row.get(0),
            )?;

            // Ensure participant
            tx.execute(
                "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
                 VALUES (?1, ?2, 'member', ?3)
                 ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                params![conversation_id, identity_id, now_str],
            )?;

            if !msg.is_from_me {
                sender_identity_id = Some(identity_id);
            }
        } else if let Some(ref sender_jid) = msg.sender_jid {
            if !msg.is_from_me {
                let is_lid = phone::is_lid_jid(sender_jid);
                let sender_phone = phone::resolve_lid_or_jid_to_phone(&tx, sender_jid)
                    .or_else(|| {
                        if !is_lid {
                            phone::jid_to_phone(sender_jid)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        if !is_lid {
                            phone::normalize_e164(sender_jid, Some("84"))
                        } else {
                            None
                        }
                    });
                let ext_id = sender_phone.clone().unwrap_or_else(|| sender_jid.clone());

                let lead_id = sender_phone
                    .as_deref()
                    .and_then(|p| Self::resolve_lead_by_phone(&tx, p).ok().flatten());
                let link_source = if lead_id.is_some() {
                    "phone_match"
                } else {
                    "unlinked"
                };

                let is_valid_name = msg.sender_name.as_ref().is_some_and(|n| {
                    let t = n.trim();
                    !t.is_empty() && !t.starts_with("12036") && !t.starts_with("+12036")
                });

                let display_name =
                    if let Some(name) = msg.sender_name.clone().filter(|_| is_valid_name) {
                        name
                    } else if let Some(ref p) = sender_phone {
                        format!("+{}", p)
                    } else {
                        "Member".to_string()
                    };

                tx.execute(
                    "INSERT INTO channel_identities (
                        lead_id, channel, channel_account_id, identity_namespace, external_id,
                        normalized_address, display_name, link_source, created_at, updated_at
                    ) VALUES (
                        ?1, 'whatsapp', ?2, 'phone', ?3,
                        ?4, ?5, ?6, ?7, ?7
                    )
                    ON CONFLICT(channel, channel_account_id, identity_namespace, external_id) WHERE channel_account_id IS NOT NULL DO UPDATE SET
                        lead_id = COALESCE(EXCLUDED.lead_id, channel_identities.lead_id),
                        display_name = CASE
                            WHEN EXCLUDED.display_name != 'Member'
                             AND EXCLUDED.display_name NOT LIKE '+12036%'
                             AND EXCLUDED.display_name NOT LIKE '12036%'
                            THEN EXCLUDED.display_name
                            WHEN channel_identities.display_name IS NOT NULL
                             AND channel_identities.display_name != 'Member'
                             AND channel_identities.display_name NOT LIKE '+12036%'
                             AND channel_identities.display_name NOT LIKE '12036%'
                            THEN channel_identities.display_name
                            ELSE EXCLUDED.display_name
                        END,
                        link_source = EXCLUDED.link_source,
                        updated_at = EXCLUDED.updated_at",
                    params![
                        lead_id,
                        channel_account_id,
                        ext_id,
                        ext_id,
                        display_name,
                        link_source,
                        now_str
                    ],
                )?;

                let member_identity_id: i64 = tx.query_row(
                    "SELECT id FROM channel_identities
                     WHERE channel = 'whatsapp' AND channel_account_id = ?1 AND identity_namespace = 'phone' AND external_id = ?2",
                    params![channel_account_id, ext_id],
                    |row| row.get(0),
                )?;

                tx.execute(
                    "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
                     VALUES (?1, ?2, 'member', ?3)
                     ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                    params![conversation_id, member_identity_id, now_str],
                )?;

                sender_identity_id = Some(member_identity_id);
            }
        }

        // 3. Message Ingestion
        let (direction, status) = if msg.is_from_me {
            ("outbound", "sent")
        } else {
            ("inbound", "received")
        };

        let content_type = Self::detect_content_type(msg);
        let text_content = msg.text.as_deref().unwrap_or("");
        let raw_payload = msg
            .raw_json
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| serde_json::to_string(msg).unwrap_or_else(|_| "{}".to_string()));

        let mut reply_to_message_id: Option<i64> = None;
        let mut reply_sender_identity_id: Option<i64> = None;

        if let Some(ref q) = msg.quote {
            reply_to_message_id = tx
                .query_row(
                    "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                    params![conversation_id, q.stanza_id],
                    |r| r.get(0),
                )
                .optional()?;

            if !q.participant_jid.trim().is_empty() {
                let q_phone = phone::jid_to_phone(&q.participant_jid)
                    .unwrap_or_else(|| q.participant_jid.trim().to_string());
                reply_sender_identity_id = tx
                    .query_row(
                        "SELECT id FROM channel_identities WHERE channel = 'whatsapp' AND channel_account_id = ?1 AND external_id = ?2",
                        params![channel_account_id, q_phone],
                        |r| r.get(0),
                    )
                    .optional()?;
            }
        }

        let existing_message: Option<(i64, Option<String>)> = tx
            .query_row(
                "SELECT id, raw_payload_json
                 FROM messages
                 WHERE conversation_id = ?1 AND provider_message_id = ?2",
                params![conversation_id, msg.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        // Handle revoke/deleted message
        if msg
            .raw_json
            .as_ref()
            .and_then(|v| v.get("is_revoke"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            tx.execute(
                "UPDATE messages
                 SET status = 'removed',
                     removed_at = COALESCE(removed_at, ?2),
                     text_content = 'This message was deleted'
                 WHERE conversation_id = ?1 AND provider_message_id = ?3",
                params![conversation_id, now_str, msg.id],
            )?;
            let lead_id_opt: Option<i64> = tx
                .query_row(
                    "SELECT ci.lead_id FROM channel_identities ci
                     JOIN conversation_participants cp ON cp.identity_id = ci.id
                     WHERE cp.conversation_id = ?1 AND ci.lead_id IS NOT NULL LIMIT 1",
                    [conversation_id],
                    |r| r.get(0),
                )
                .optional()?;
            let msg_id: i64 = tx
                .query_row(
                    "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                    params![conversation_id, msg.id],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or(0);
            let delta_event = if msg_id > 0 {
                crate::conversation::repository::ConversationRepository::record_message_delta_tx(
                    &tx,
                    conversation_id,
                    lead_id_opt,
                    crate::conversation::types::Channel::WhatsApp,
                    msg_id,
                )?
            } else {
                None
            };
            tx.commit()?;
            return Ok(Some(AppliedWhatsAppMessage {
                conversation_id,
                lead_id: lead_id_opt,
                message_id: msg_id,
                delta_event,
            }));
        }

        let _message_id = match existing_message {
            Some((mid, existing_raw_payload)) => {
                let merged_raw_payload =
                    merge_raw_payload_json(existing_raw_payload.as_deref(), &raw_payload);
                tx.execute(
                    "UPDATE messages
                     SET raw_payload_json = ?2,
                         provider_timestamp = COALESCE(provider_timestamp, ?3),
                         reply_to_message_id = COALESCE(reply_to_message_id, ?4),
                         reply_sender_identity_id = COALESCE(reply_sender_identity_id, ?5)
                     WHERE id = ?1",
                    params![
                        mid,
                        merged_raw_payload,
                        event_time,
                        reply_to_message_id,
                        reply_sender_identity_id
                    ],
                )?;

                upsert_whatsapp_attachments(&tx, mid, &msg.attachments, &now_str)?;

                mid
            }
            None => {
                tx.execute(
                    "INSERT INTO messages (
                        conversation_id, sender_identity_id, direction, provider_message_id,
                        content_type, text_content, content_json, raw_payload_json,
                        provider_timestamp, ingested_at, status, reply_to_message_id, reply_sender_identity_id
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, '{}', ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        conversation_id,
                        sender_identity_id,
                        direction,
                        msg.id,
                        content_type,
                        text_content,
                        raw_payload,
                        event_time,
                        now_str,
                        status,
                        reply_to_message_id,
                        reply_sender_identity_id
                    ],
                )?;
                let mid = tx.last_insert_rowid();

                upsert_whatsapp_attachments(&tx, mid, &msg.attachments, &now_str)?;

                // Check and apply any pending reactions
                let mut pending_stmt = tx.prepare(
                    "SELECT id, actor_external_id, reaction, removed, occurred_at
                     FROM pending_message_reactions
                     WHERE channel_account_id = ?1 AND provider_message_id = ?2",
                )?;

                let pending_reactions: Vec<(i64, String, String, i64, String)> = pending_stmt
                    .query_map(params![channel_account_id, msg.id], |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    })?
                    .filter_map(Result::ok)
                    .collect();

                for (pending_id, actor, emoji, removed, reacted_at) in pending_reactions {
                    // WhatsApp semantics: each actor has at most 1 active reaction per message.
                    tx.execute(
                        "DELETE FROM message_reactions WHERE message_id = ?1 AND actor_external_id = ?2",
                        params![mid, actor],
                    )?;
                    if removed != 1 && !emoji.trim().is_empty() {
                        tx.execute(
                            "INSERT INTO message_reactions (message_id, actor_external_id, reaction, count, reacted_at, removed_at)
                             VALUES (?1, ?2, ?3, 1, ?4, NULL)
                             ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET
                                 count = 1,
                                 reacted_at = EXCLUDED.reacted_at,
                                 removed_at = NULL",
                            params![mid, actor, emoji, reacted_at],
                        )?;
                    }
                    tx.execute(
                        "DELETE FROM pending_message_reactions WHERE id = ?1",
                        params![pending_id],
                    )?;
                }

                mid
            }
        };

        // Ingest mentions into message_mentions table
        if let Some(ref text) = msg.text {
            for mentioned_jid in &msg.mentions {
                let m_phone = phone::resolve_lid_or_jid_to_phone(&tx, mentioned_jid)
                    .or_else(|| phone::jid_to_phone(mentioned_jid))
                    .unwrap_or_else(|| mentioned_jid.trim().to_string());
                let clean_digits: String = m_phone.chars().filter(|c| c.is_ascii_digit()).collect();

                let (target_id, display_name): (Option<i64>, Option<String>) = tx
                    .query_row(
                        "SELECT id, display_name FROM channel_identities
                         WHERE channel = 'whatsapp' AND channel_account_id = ?1 AND (external_id = ?2 OR external_id = ?3)",
                        params![channel_account_id, m_phone, clean_digits],
                        |r| Ok((Some(r.get(0)?), r.get(1)?)),
                    )
                    .optional()?
                    .unwrap_or((None, None));

                let name_tag = display_name.unwrap_or_else(|| format!("+{}", clean_digits));
                let display_text = if name_tag.starts_with('~') || name_tag.starts_with('+') {
                    format!("@{}", name_tag)
                } else {
                    format!("@~{}", name_tag)
                };

                let jid_user = phone::extract_lid_user(mentioned_jid)
                    .or_else(|| phone::jid_to_phone(mentioned_jid))
                    .unwrap_or_else(|| clean_digits.clone());

                let patterns = [
                    format!("@{}", jid_user),
                    format!("@{}", clean_digits),
                    format!("@{}", m_phone),
                ];

                for pattern in &patterns {
                    if let Some(idx) = text.find(pattern) {
                        let start_utf16 = text[..idx].encode_utf16().count();
                        let length_utf16 = pattern.encode_utf16().count();

                        let _ = tx.execute(
                            "INSERT INTO message_mentions (
                                message_id, identity_id, external_id, display_text, start_utf16, length_utf16, created_at
                            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                            ON CONFLICT(message_id, start_utf16, length_utf16) DO UPDATE SET
                                identity_id = COALESCE(EXCLUDED.identity_id, message_mentions.identity_id),
                                external_id = EXCLUDED.external_id,
                                display_text = EXCLUDED.display_text",
                            params![_message_id, target_id, clean_digits, display_text, start_utf16 as i64, length_utf16 as i64, now_str],
                        );
                        break;
                    }
                }
            }
        }

        let lead_id_opt: Option<i64> = tx
            .query_row(
                "SELECT ci.lead_id FROM channel_identities ci
                 JOIN conversation_participants cp ON cp.identity_id = ci.id
                 WHERE cp.conversation_id = ?1 AND ci.lead_id IS NOT NULL LIMIT 1",
                [conversation_id],
                |r| r.get(0),
            )
            .optional()?;

        let delta_event =
            crate::conversation::repository::ConversationRepository::record_message_delta_tx(
                &tx,
                conversation_id,
                lead_id_opt,
                crate::conversation::types::Channel::WhatsApp,
                _message_id,
            )?;

        tx.commit()?;
        Ok(Some(AppliedWhatsAppMessage {
            conversation_id,
            lead_id: lead_id_opt,
            message_id: _message_id,
            delta_event,
        }))
    }

    /// Ingests a WhatsApp reaction event.
    pub fn ingest_reaction(
        conn: &mut Connection,
        channel_account_id: i64,
        reaction: &RawWhatsAppReaction,
    ) -> Result<()> {
        let tx = conn.transaction()?;
        let now_str = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let occurred_at = Self::normalize_iso_timestamp(reaction.timestamp.as_deref());

        let is_group = phone::is_group_jid(&reaction.chat_jid);
        let external_thread_id = if is_group {
            reaction.chat_jid.trim().to_string()
        } else {
            let phone = phone::jid_to_phone(&reaction.chat_jid)
                .unwrap_or_else(|| reaction.chat_jid.trim().to_string());
            let normalized = phone::normalize_e164(&phone, Some("84")).unwrap_or(phone);
            phone::phone_to_direct_jid(&normalized)
        };

        let actor_phone = if reaction.is_from_me {
            let self_phone: Option<String> = tx
                .query_row(
                    "SELECT self_external_id FROM channel_accounts WHERE id = ?1",
                    params![channel_account_id],
                    |r| r.get(0),
                )
                .optional()?
                .flatten();
            self_phone.unwrap_or_else(|| {
                phone::resolve_lid_or_jid_to_phone(&tx, &reaction.sender_jid)
                    .or_else(|| phone::jid_to_phone(&reaction.sender_jid))
                    .unwrap_or_else(|| reaction.sender_jid.trim().to_string())
            })
        } else {
            phone::resolve_lid_or_jid_to_phone(&tx, &reaction.sender_jid)
                .or_else(|| phone::jid_to_phone(&reaction.sender_jid))
                .unwrap_or_else(|| reaction.sender_jid.trim().to_string())
        };

        let target_message_id: Option<i64> = tx
            .query_row(
                "SELECT m.id FROM messages m
                 JOIN channel_conversations cc ON cc.id = m.conversation_id
                 WHERE cc.channel_account_id = ?1 AND m.provider_message_id = ?2
                 LIMIT 1",
                params![channel_account_id, reaction.message_id],
                |r| r.get(0),
            )
            .optional()?;

        let is_removed = reaction.emoji.trim().is_empty();

        if let Some(msg_id) = target_message_id {
            // WhatsApp semantics: single-active reaction per actor. Always delete previous reactions for this actor first.
            tx.execute(
                "DELETE FROM message_reactions WHERE message_id = ?1 AND actor_external_id = ?2",
                params![msg_id, actor_phone],
            )?;
            if !is_removed {
                tx.execute(
                    "INSERT INTO message_reactions (message_id, actor_external_id, reaction, count, reacted_at, removed_at)
                     VALUES (?1, ?2, ?3, 1, ?4, NULL)
                     ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET
                         count = 1,
                         reacted_at = EXCLUDED.reacted_at,
                         removed_at = NULL",
                    params![msg_id, actor_phone, reaction.emoji, occurred_at],
                )?;
            }
        } else {
            // Save as pending reaction
            let conversation_kind = if is_group { "group" } else { "direct" };
            tx.execute(
                "INSERT INTO pending_message_reactions (
                    channel_account_id, external_thread_id, conversation_kind,
                    provider_event_id, provider_message_id, provider_client_message_id,
                    actor_external_id, reaction, removed, occurred_at, raw_payload_json, created_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, '', ?4, '', ?5, ?6, ?7, ?8, '{}', ?9, ?9
                )
                ON CONFLICT DO NOTHING",
                params![
                    channel_account_id,
                    external_thread_id,
                    conversation_kind,
                    reaction.message_id,
                    actor_phone,
                    reaction.emoji,
                    if is_removed { 1 } else { 0 },
                    occurred_at,
                    now_str
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Ingests a WhatsApp delivery or read receipt, returning list of (conversation_id, message_id) updated.
    pub fn ingest_receipt(
        conn: &mut Connection,
        channel_account_id: i64,
        receipt: &RawWhatsAppReceipt,
    ) -> Result<Vec<(i64, i64)>> {
        let tx = conn.transaction()?;
        let occurred_at = Self::normalize_iso_timestamp(receipt.timestamp.as_deref());

        let recipient_phone = receipt
            .sender_jid
            .as_deref()
            .and_then(phone::jid_to_phone)
            .unwrap_or_default();

        let mut updated = Vec::new();

        for mid in &receipt.message_ids {
            let msg_info: Option<(i64, i64, String, String, String)> = tx
                .query_row(
                    "SELECT m.id, m.conversation_id, m.status, cc.external_thread_id, cc.kind FROM messages m
                     JOIN channel_conversations cc ON cc.id = m.conversation_id
                     WHERE cc.channel_account_id = ?1
                       AND m.direction = 'outbound'
                       AND (m.provider_message_id = ?2 OR m.provider_client_message_id = ?2)
                     LIMIT 1",
                    params![channel_account_id, mid],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()?;

            if let Some((id, conv_id, current_status, conv_thread_id, conv_kind)) = msg_info {
                // Verify that receipt.chat_jid matches the message's conversation if chat_jid is specified
                if !receipt.chat_jid.trim().is_empty() {
                    let chat_jid = receipt.chat_jid.trim();
                    let matches_conv = if conv_thread_id == chat_jid {
                        true
                    } else if conv_kind == "group" {
                        match (conv_thread_id.split('@').next(), chat_jid.split('@').next()) {
                            (Some(a), Some(b)) => a == b,
                            _ => false,
                        }
                    } else {
                        let resolved_chat_phone = phone::resolve_lid_or_jid_to_phone(&tx, chat_jid)
                            .or_else(|| phone::jid_to_phone(chat_jid))
                            .unwrap_or_else(|| chat_jid.to_string());
                        let normalized_chat_phone =
                            phone::normalize_e164(&resolved_chat_phone, Some("84"))
                                .unwrap_or_else(|| resolved_chat_phone.clone());

                        let resolved_conv_phone =
                            phone::resolve_lid_or_jid_to_phone(&tx, &conv_thread_id)
                                .or_else(|| phone::jid_to_phone(&conv_thread_id))
                                .unwrap_or_else(|| conv_thread_id.clone());
                        let normalized_conv_phone =
                            phone::normalize_e164(&resolved_conv_phone, Some("84"))
                                .unwrap_or_else(|| resolved_conv_phone.clone());

                        if normalized_chat_phone == normalized_conv_phone
                            || phone::phone_to_direct_jid(&normalized_chat_phone) == conv_thread_id
                        {
                            true
                        } else {
                            // Check identity association
                            tx.query_row(
                                "SELECT 1 FROM channel_identities ci
                                 JOIN conversation_participants cp ON cp.identity_id = ci.id
                                 WHERE cp.conversation_id = ?1
                                   AND (ci.external_id = ?2 OR ci.normalized_address = ?3 OR ci.external_id = ?3)
                                 LIMIT 1",
                                params![conv_id, chat_jid, normalized_chat_phone],
                                |_| Ok(true),
                            ).unwrap_or(false)
                        }
                    };

                    if !matches_conv {
                        continue;
                    }
                }

                let r_type = receipt.receipt_type.to_lowercase();
                let db_receipt_type = match r_type.as_str() {
                    "read" | "played" => "read",
                    "delivered" => "delivered",
                    "sent" | "accepted" => "sent",
                    "failed" => "failed",
                    _ => continue,
                };

                let mut status_changed = false;
                if current_status != "removed" {
                    if db_receipt_type == "failed" {
                        // failed can only be set if not already delivered or read, and not already failed
                        if current_status == "sending"
                            || current_status == "queued"
                            || current_status == "pending"
                            || current_status == "sent"
                        {
                            tx.execute(
                                "UPDATE messages SET status = 'failed' WHERE id = ?1",
                                params![id],
                            )?;
                            status_changed = true;
                        }
                    } else if db_receipt_type == "read" {
                        // read can be set if not already read or failed
                        if current_status != "read" && current_status != "failed" {
                            tx.execute(
                                "UPDATE messages SET status = 'read' WHERE id = ?1",
                                params![id],
                            )?;
                            status_changed = true;
                        }
                    } else if db_receipt_type == "delivered" {
                        // delivered can only be set if not already delivered, read, or failed
                        if current_status == "sending"
                            || current_status == "queued"
                            || current_status == "pending"
                            || current_status == "sent"
                        {
                            tx.execute(
                                "UPDATE messages SET status = 'delivered' WHERE id = ?1",
                                params![id],
                            )?;
                            status_changed = true;
                        }
                    } else if db_receipt_type == "sent" {
                        // sent can only be set if in initial sending states
                        if current_status == "sending"
                            || current_status == "queued"
                            || current_status == "pending"
                        {
                            tx.execute(
                                "UPDATE messages SET status = 'sent' WHERE id = ?1",
                                params![id],
                            )?;
                            status_changed = true;
                        }
                    }
                }

                if status_changed {
                    updated.push((conv_id, id));
                }

                tx.execute(
                    "INSERT INTO message_receipts (message_id, recipient_external_id, receipt_type, occurred_at, raw_payload_json)
                     VALUES (?1, ?2, ?3, ?4, '{}')
                     ON CONFLICT(message_id, recipient_external_id, receipt_type) DO UPDATE SET
                         occurred_at = MAX(message_receipts.occurred_at, excluded.occurred_at)",
                    params![id, recipient_phone, db_receipt_type, occurred_at],
                )?;
            }
        }

        tx.commit()?;
        Ok(updated)
    }

    /// Ensures that a conversation record exists in `channel_conversations` (and if direct, `channel_identities`).
    pub fn ensure_conversation_exists(
        conn: &mut Connection,
        channel_account_id: i64,
        chat_jid: &str,
        title: Option<&str>,
    ) -> Result<i64> {
        let tx = conn.transaction()?;
        let now_str = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let is_group = phone::is_group_jid(chat_jid);

        let (external_thread_id, kind, default_title) = if is_group {
            (
                chat_jid.trim().to_string(),
                "group",
                title
                    .map(str::to_string)
                    .unwrap_or_else(|| "WhatsApp Group".to_string()),
            )
        } else {
            let peer_phone = phone::resolve_lid_or_jid_to_phone(&tx, chat_jid)
                .or_else(|| phone::jid_to_phone(chat_jid))
                .unwrap_or_else(|| chat_jid.trim().to_string());
            let normalized_phone = phone::normalize_e164(&peer_phone, Some("84"))
                .unwrap_or_else(|| peer_phone.clone());
            let direct_thread_id = if phone::is_direct_jid(chat_jid) {
                chat_jid.trim().to_string()
            } else if normalized_phone.chars().all(|c| c.is_ascii_digit())
                && normalized_phone.len() >= 7
            {
                phone::phone_to_direct_jid(&normalized_phone)
            } else {
                chat_jid.trim().to_string()
            };
            let formatted_title = title.map(str::to_string).unwrap_or_else(|| {
                if normalized_phone.chars().all(|c| c.is_ascii_digit())
                    && normalized_phone.len() >= 7
                {
                    format!("+{}", normalized_phone)
                } else {
                    normalized_phone.clone()
                }
            });
            (direct_thread_id, "direct", formatted_title)
        };

        tx.execute(
            "INSERT INTO channel_conversations (
                channel_account_id, external_thread_id, kind, title, avatar_url, status, last_message_at, last_synced_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, NULL, 'active', ?5, ?5, ?5, ?5)
            ON CONFLICT(channel_account_id, external_thread_id, kind) DO UPDATE SET
                title = CASE
                    WHEN EXCLUDED.title IS NOT NULL AND EXCLUDED.title != 'WhatsApp Group' THEN EXCLUDED.title
                    ELSE channel_conversations.title
                END,
                last_synced_at = EXCLUDED.last_synced_at,
                updated_at = EXCLUDED.updated_at",
            params![
                channel_account_id,
                external_thread_id,
                kind,
                default_title,
                now_str
            ],
        )?;

        let conversation_id: i64 = tx.query_row(
            "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = ?3",
            params![channel_account_id, external_thread_id, kind],
            |row| row.get(0),
        )?;

        if !is_group {
            let peer_phone = phone::resolve_lid_or_jid_to_phone(&tx, chat_jid)
                .unwrap_or_else(|| chat_jid.trim().to_string());
            let normalized_phone =
                phone::normalize_e164(&peer_phone, Some("84")).unwrap_or(peer_phone);

            let lead_id = Self::resolve_lead_by_phone(&tx, &normalized_phone)?;
            let link_source = if lead_id.is_some() {
                "phone_match"
            } else {
                "unlinked"
            };

            let display_name = title
                .map(str::to_string)
                .unwrap_or_else(|| format!("+{}", normalized_phone));

            tx.execute(
                "INSERT INTO channel_identities (
                    lead_id, channel, channel_account_id, identity_namespace, external_id,
                    normalized_address, display_name, link_source, created_at, updated_at
                ) VALUES (
                    ?1, 'whatsapp', ?2, 'phone', ?3,
                    ?4, ?5, ?6, ?7, ?7
                )
                ON CONFLICT(channel, channel_account_id, identity_namespace, external_id) WHERE channel_account_id IS NOT NULL DO UPDATE SET
                    lead_id = EXCLUDED.lead_id,
                    display_name = COALESCE(EXCLUDED.display_name, channel_identities.display_name),
                    link_source = EXCLUDED.link_source,
                    updated_at = EXCLUDED.updated_at",
                params![
                    lead_id,
                    channel_account_id,
                    normalized_phone,
                    normalized_phone,
                    display_name,
                    link_source,
                    now_str
                ],
            )?;

            let identity_id: i64 = tx.query_row(
                "SELECT id FROM channel_identities
                 WHERE channel = 'whatsapp' AND channel_account_id = ?1 AND identity_namespace = 'phone' AND external_id = ?2",
                params![channel_account_id, normalized_phone],
                |row| row.get(0),
            )?;

            tx.execute(
                "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
                 VALUES (?1, ?2, 'member', ?3)
                 ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                params![conversation_id, identity_id, now_str],
            )?;
        }

        tx.commit()?;
        Ok(conversation_id)
    }

    /// Unwraps nested wrapper messages such as ephemeral, view-once, document-with-caption,
    /// edited, device-sent, group-mentioned, and bot-invoke messages.
    pub fn unwrap_message(
        mut msg: &whatsapp_rust::prelude::wa::Message,
    ) -> &whatsapp_rust::prelude::wa::Message {
        for _ in 0..10 {
            if let Some(eph) = msg.ephemeral_message.as_option() {
                if let Some(inner) = eph.message.as_option() {
                    msg = inner;
                    continue;
                }
            }
            if let Some(vo) = msg.view_once_message.as_option() {
                if let Some(inner) = vo.message.as_option() {
                    msg = inner;
                    continue;
                }
            }
            if let Some(vo2) = msg.view_once_message_v2.as_option() {
                if let Some(inner) = vo2.message.as_option() {
                    msg = inner;
                    continue;
                }
            }
            if let Some(vo2_ext) = msg.view_once_message_v2_extension.as_option() {
                if let Some(inner) = vo2_ext.message.as_option() {
                    msg = inner;
                    continue;
                }
            }
            if let Some(doc_cap) = msg.document_with_caption_message.as_option() {
                if let Some(inner) = doc_cap.message.as_option() {
                    msg = inner;
                    continue;
                }
            }
            if let Some(ed) = msg.edited_message.as_option() {
                if let Some(inner) = ed.message.as_option() {
                    msg = inner;
                    continue;
                }
            }
            if let Some(ds) = msg.device_sent_message.as_option() {
                if let Some(inner) = ds.message.as_option() {
                    msg = inner;
                    continue;
                }
            }
            if let Some(gm) = msg.group_mentioned_message.as_option() {
                if let Some(inner) = gm.message.as_option() {
                    msg = inner;
                    continue;
                }
            }
            if let Some(bi) = msg.bot_invoke_message.as_option() {
                if let Some(inner) = bi.message.as_option() {
                    msg = inner;
                    continue;
                }
            }
            if let Some(bf) = msg.bot_forwarded_message.as_option() {
                if let Some(inner) = bf.message.as_option() {
                    msg = inner;
                    continue;
                }
            }
            break;
        }
        msg
    }

    /// Extracts a `RawWhatsAppMessage` from a protobuf `wa::Message` and `wa::MessageKey`.
    pub fn extract_raw_message(
        msg: &whatsapp_rust::prelude::wa::Message,
        key: &whatsapp_rust::prelude::wa::MessageKey,
        timestamp_secs: Option<u64>,
        push_name: Option<&str>,
        chat_jid: &str,
        conv_title: Option<&str>,
        participant_override: Option<&str>,
    ) -> Option<RawWhatsAppMessage> {
        let id = key.id.clone().unwrap_or_default();
        if id.is_empty() {
            return None;
        }

        let is_from_me = key.from_me.unwrap_or(false);
        let is_group = phone::is_group_jid(chat_jid);
        let sender_jid = if is_from_me {
            None
        } else if is_group {
            participant_override
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty() && !phone::is_group_jid(p))
                .or_else(|| {
                    key.participant
                        .clone()
                        .filter(|p| !p.trim().is_empty() && !phone::is_group_jid(p))
                })
        } else {
            participant_override
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty() && !phone::is_group_jid(p))
                .or_else(|| {
                    key.participant
                        .clone()
                        .filter(|p| !p.trim().is_empty() && !phone::is_group_jid(p))
                })
                .or_else(|| {
                    if phone::is_group_jid(chat_jid) {
                        None
                    } else {
                        Some(chat_jid.to_string())
                    }
                })
        };

        let unwrapped = Self::unwrap_message(msg);

        // Check for message revoke (deleted message)
        if let Some(pm) = unwrapped.protocol_message.as_option() {
            if pm.r#type
                == Some(whatsapp_rust::prelude::wa::message::protocol_message::Type::REVOKE)
            {
                if let Some(target_key) = pm.key.as_option() {
                    if let Some(target_id) = target_key.id.as_deref() {
                        let iso_timestamp = timestamp_secs.and_then(|secs| {
                            DateTime::from_timestamp(secs as i64, 0).map(|dt| dt.to_rfc3339())
                        });
                        return Some(RawWhatsAppMessage {
                            id: target_id.to_string(),
                            chat_jid: chat_jid.to_string(),
                            sender_jid,
                            sender_name: None,
                            text: Some("This message was deleted".to_string()),
                            timestamp: iso_timestamp,
                            is_from_me,
                            attachments: vec![],
                            quote: None,
                            raw_json: Some(serde_json::json!({ "is_revoke": true })),
                            mentions: vec![],
                        });
                    }
                }
            }
        }

        let text = unwrapped
            .conversation
            .clone()
            .or_else(|| {
                unwrapped
                    .extended_text_message
                    .as_option()
                    .and_then(|e| e.text.clone())
            })
            .or_else(|| {
                unwrapped
                    .image_message
                    .as_option()
                    .and_then(|i| i.caption.clone())
            })
            .or_else(|| {
                unwrapped
                    .video_message
                    .as_option()
                    .and_then(|v| v.caption.clone())
            })
            .or_else(|| {
                unwrapped
                    .document_message
                    .as_option()
                    .and_then(|d| d.caption.clone())
            })
            .or_else(|| {
                unwrapped.location_message.as_option().map(|l| {
                    let name = l.name.as_deref().unwrap_or("Location");
                    let address = l.address.as_deref().unwrap_or("");
                    if address.is_empty() {
                        format!("📍 {name}")
                    } else {
                        format!("📍 {name} ({address})")
                    }
                })
            })
            .or_else(|| {
                unwrapped.contact_message.as_option().map(|c| {
                    let name = c.display_name.as_deref().unwrap_or("Contact");
                    format!("👤 Contact: {name}")
                })
            })
            .or_else(|| {
                unwrapped
                    .buttons_response_message
                    .as_option()
                    .and_then(|b| b.selected_button_id.clone())
            })
            .or_else(|| {
                unwrapped
                    .template_button_reply_message
                    .as_option()
                    .and_then(|b| b.selected_display_text.clone())
            })
            .or_else(|| {
                unwrapped
                    .list_response_message
                    .as_option()
                    .and_then(|l| l.title.clone())
            });

        let mut attachments = Vec::new();
        if let Some(img) = unwrapped.image_message.as_option() {
            let thumb_data_url = img
                .jpeg_thumbnail
                .as_ref()
                .filter(|t| !t.is_empty())
                .map(|t| {
                    format!(
                        "data:image/jpeg;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(t)
                    )
                });
            let media_key_b64 = img
                .media_key
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));
            let file_sha256_b64 = img
                .file_sha256
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));
            let file_enc_sha256_b64 = img
                .file_enc_sha256
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));

            attachments.push(RawWhatsAppAttachment {
                media_type: "image".to_string(),
                file_name: None,
                mime_type: img
                    .mimetype
                    .clone()
                    .or_else(|| Some("image/jpeg".to_string())),
                size_bytes: img.file_length,
                url: img.url.clone(),
                direct_path: img.direct_path.clone(),
                thumbnail_url: thumb_data_url,
                media_key: media_key_b64,
                file_sha256: file_sha256_b64,
                file_enc_sha256: file_enc_sha256_b64,
                width: img.width,
                height: img.height,
            });
        } else if let Some(vid) = unwrapped.video_message.as_option() {
            let thumb_data_url = vid
                .jpeg_thumbnail
                .as_ref()
                .filter(|t| !t.is_empty())
                .map(|t| {
                    format!(
                        "data:image/jpeg;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(t)
                    )
                });
            let media_key_b64 = vid
                .media_key
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));
            let file_sha256_b64 = vid
                .file_sha256
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));
            let file_enc_sha256_b64 = vid
                .file_enc_sha256
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));

            attachments.push(RawWhatsAppAttachment {
                media_type: "video".to_string(),
                file_name: None,
                mime_type: vid
                    .mimetype
                    .clone()
                    .or_else(|| Some("video/mp4".to_string())),
                size_bytes: vid.file_length,
                url: vid.url.clone(),
                direct_path: vid.direct_path.clone(),
                thumbnail_url: thumb_data_url,
                media_key: media_key_b64,
                file_sha256: file_sha256_b64,
                file_enc_sha256: file_enc_sha256_b64,
                width: vid.width,
                height: vid.height,
            });
        } else if let Some(aud) = unwrapped.audio_message.as_option() {
            let media_key_b64 = aud
                .media_key
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));
            let file_sha256_b64 = aud
                .file_sha256
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));
            let file_enc_sha256_b64 = aud
                .file_enc_sha256
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));

            attachments.push(RawWhatsAppAttachment {
                media_type: "audio".to_string(),
                file_name: None,
                mime_type: aud
                    .mimetype
                    .clone()
                    .or_else(|| Some("audio/ogg".to_string())),
                size_bytes: aud.file_length,
                url: aud.url.clone(),
                direct_path: aud.direct_path.clone(),
                thumbnail_url: None,
                media_key: media_key_b64,
                file_sha256: file_sha256_b64,
                file_enc_sha256: file_enc_sha256_b64,
                width: None,
                height: None,
            });
        } else if let Some(doc) = unwrapped.document_message.as_option() {
            let thumb_data_url = doc
                .jpeg_thumbnail
                .as_ref()
                .filter(|t| !t.is_empty())
                .map(|t| {
                    format!(
                        "data:image/jpeg;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(t)
                    )
                });
            let media_key_b64 = doc
                .media_key
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));
            let file_sha256_b64 = doc
                .file_sha256
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));
            let file_enc_sha256_b64 = doc
                .file_enc_sha256
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));

            attachments.push(RawWhatsAppAttachment {
                media_type: "document".to_string(),
                file_name: doc.file_name.clone().or_else(|| doc.title.clone()),
                mime_type: doc
                    .mimetype
                    .clone()
                    .or_else(|| Some("application/octet-stream".to_string())),
                size_bytes: doc.file_length,
                url: doc.url.clone(),
                direct_path: doc.direct_path.clone(),
                thumbnail_url: thumb_data_url,
                media_key: media_key_b64,
                file_sha256: file_sha256_b64,
                file_enc_sha256: file_enc_sha256_b64,
                width: None,
                height: None,
            });
        } else if let Some(stk) = unwrapped.sticker_message.as_option() {
            let thumb_data_url = stk
                .png_thumbnail
                .as_ref()
                .filter(|t| !t.is_empty())
                .map(|t| {
                    format!(
                        "data:image/png;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(t)
                    )
                });
            let media_key_b64 = stk
                .media_key
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));
            let file_sha256_b64 = stk
                .file_sha256
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));
            let file_enc_sha256_b64 = stk
                .file_enc_sha256
                .as_ref()
                .filter(|k| !k.is_empty())
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k));

            attachments.push(RawWhatsAppAttachment {
                media_type: "sticker".to_string(),
                file_name: None,
                mime_type: stk
                    .mimetype
                    .clone()
                    .or_else(|| Some("image/webp".to_string())),
                size_bytes: stk.file_length,
                url: stk.url.clone(),
                direct_path: stk.direct_path.clone(),
                thumbnail_url: thumb_data_url,
                media_key: media_key_b64,
                file_sha256: file_sha256_b64,
                file_enc_sha256: file_enc_sha256_b64,
                width: stk.width,
                height: stk.height,
            });
        }

        let context_info = unwrapped
            .extended_text_message
            .as_option()
            .and_then(|ext| ext.context_info.as_option())
            .or_else(|| {
                unwrapped
                    .image_message
                    .as_option()
                    .and_then(|img| img.context_info.as_option())
            })
            .or_else(|| {
                unwrapped
                    .video_message
                    .as_option()
                    .and_then(|vid| vid.context_info.as_option())
            })
            .or_else(|| {
                unwrapped
                    .document_message
                    .as_option()
                    .and_then(|doc| doc.context_info.as_option())
            })
            .or_else(|| {
                unwrapped
                    .audio_message
                    .as_option()
                    .and_then(|aud| aud.context_info.as_option())
            })
            .or_else(|| {
                unwrapped
                    .sticker_message
                    .as_option()
                    .and_then(|stk| stk.context_info.as_option())
            });

        let mentions = context_info
            .map(|ctx_info| ctx_info.mentioned_jid.clone())
            .unwrap_or_default();

        let quote = context_info.and_then(|ctx_info| {
            ctx_info.stanza_id.as_deref().map(|stanza_id| {
                let (quoted_text, quoted_media_type) =
                    if let Some(qm) = ctx_info.quoted_message.as_option() {
                        let qm_unwrapped = Self::unwrap_message(qm);
                        let text = qm_unwrapped
                            .conversation
                            .clone()
                            .or_else(|| {
                                qm_unwrapped
                                    .extended_text_message
                                    .as_option()
                                    .and_then(|e| e.text.clone())
                            })
                            .or_else(|| {
                                qm_unwrapped
                                    .image_message
                                    .as_option()
                                    .and_then(|i| i.caption.clone())
                            })
                            .or_else(|| {
                                qm_unwrapped
                                    .video_message
                                    .as_option()
                                    .and_then(|v| v.caption.clone())
                            })
                            .or_else(|| {
                                qm_unwrapped
                                    .document_message
                                    .as_option()
                                    .and_then(|d| d.caption.clone().or(d.title.clone()))
                            });
                        let media_type = if qm_unwrapped.image_message.as_option().is_some() {
                            Some("image".to_string())
                        } else if qm_unwrapped.video_message.as_option().is_some() {
                            Some("video".to_string())
                        } else if qm_unwrapped.audio_message.as_option().is_some() {
                            Some("audio".to_string())
                        } else if qm_unwrapped.document_message.as_option().is_some() {
                            Some("file".to_string())
                        } else if qm_unwrapped.sticker_message.as_option().is_some() {
                            Some("sticker".to_string())
                        } else {
                            None
                        };
                        (text, media_type)
                    } else {
                        (None, None)
                    };

                RawWhatsAppQuote {
                    stanza_id: stanza_id.to_string(),
                    participant_jid: ctx_info.participant.clone().unwrap_or_default(),
                    text: quoted_text,
                    media_type: quoted_media_type,
                    timestamp: None,
                }
            })
        });

        // Skip completely empty messages
        if text.is_none() && attachments.is_empty() {
            return None;
        }

        let iso_timestamp = timestamp_secs
            .and_then(|secs| DateTime::from_timestamp(secs as i64, 0).map(|dt| dt.to_rfc3339()));

        let sender_name = if is_group {
            push_name.map(str::to_string)
        } else {
            push_name
                .map(str::to_string)
                .or_else(|| conv_title.map(str::to_string))
        };

        Some(RawWhatsAppMessage {
            id,
            chat_jid: chat_jid.to_string(),
            sender_jid,
            sender_name,
            text,
            timestamp: iso_timestamp,
            is_from_me,
            attachments,
            quote,
            raw_json: None,
            mentions,
        })
    }

    /// Extracts a `RawWhatsAppReaction` from a protobuf `wa::Message` and `wa::MessageKey`.
    pub fn extract_raw_reaction(
        msg: &whatsapp_rust::prelude::wa::Message,
        key: &whatsapp_rust::prelude::wa::MessageKey,
        timestamp_secs: Option<u64>,
        chat_jid: &str,
        participant_override: Option<&str>,
    ) -> Option<RawWhatsAppReaction> {
        let unwrapped = Self::unwrap_message(msg);
        if let Some(rm) = unwrapped.reaction_message.as_option() {
            let target_id = rm.key.as_option().and_then(|k| k.id.clone())?;
            if target_id.trim().is_empty() {
                return None;
            }
            let is_from_me = key.from_me.unwrap_or(false);
            let is_group = phone::is_group_jid(chat_jid);
            let sender_jid = if is_from_me {
                String::new()
            } else if is_group {
                participant_override
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty() && !phone::is_group_jid(p))
                    .or_else(|| {
                        key.participant
                            .clone()
                            .filter(|p| !p.trim().is_empty() && !phone::is_group_jid(p))
                    })
                    .unwrap_or_default()
            } else {
                participant_override
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty() && !phone::is_group_jid(p))
                    .or_else(|| {
                        key.participant
                            .clone()
                            .filter(|p| !p.trim().is_empty() && !phone::is_group_jid(p))
                    })
                    .unwrap_or_else(|| {
                        if phone::is_group_jid(chat_jid) {
                            String::new()
                        } else {
                            chat_jid.to_string()
                        }
                    })
            };
            let emoji = rm.text.clone().unwrap_or_default();
            let timestamp = rm
                .sender_timestamp_ms
                .and_then(|ms| {
                    DateTime::from_timestamp_millis(ms)
                        .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true))
                })
                .or_else(|| {
                    timestamp_secs.and_then(|s| {
                        DateTime::from_timestamp(s as i64, 0)
                            .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true))
                    })
                });

            return Some(RawWhatsAppReaction {
                message_id: target_id,
                chat_jid: chat_jid.to_string(),
                sender_jid,
                emoji,
                timestamp,
                is_from_me,
            });
        }
        None
    }

    /// Ingests or synchronizes group metadata and all participants from `whatsapp_rust::GroupMetadata`.
    pub fn sync_group_metadata(
        conn: &mut Connection,
        channel_account_id: i64,
        group: &whatsapp_rust::GroupMetadata,
    ) -> Result<i64> {
        let tx = conn.transaction()?;
        let now_str = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let external_thread_id = group.id.to_string();
        let title = if group.subject.trim().is_empty() {
            "WhatsApp Group".to_string()
        } else {
            group.subject.trim().to_string()
        };

        // Upsert channel_conversations
        tx.execute(
            "INSERT INTO channel_conversations (
                channel_account_id, external_thread_id, kind, title, avatar_url, status, last_synced_at, created_at, updated_at
            ) VALUES (?1, ?2, 'group', ?3, NULL, 'active', ?4, ?4, ?4)
            ON CONFLICT(channel_account_id, external_thread_id, kind) DO UPDATE SET
                title = CASE
                    WHEN EXCLUDED.title != 'WhatsApp Group' THEN EXCLUDED.title
                    ELSE channel_conversations.title
                END,
                last_synced_at = EXCLUDED.last_synced_at,
                updated_at = EXCLUDED.updated_at",
            params![channel_account_id, external_thread_id, title, now_str],
        )?;

        let conversation_id: i64 = tx.query_row(
            "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'group'",
            params![channel_account_id, external_thread_id],
            |row| row.get(0),
        )?;

        let mut active_identity_ids = Vec::new();

        for participant in &group.participants {
            let part_jid_str = participant.jid.to_string();

            // Save LID-PN mapping if phone_number is present
            if let Some(ref lid) = participant.lid {
                if let Some(ref pn) = participant.phone_number {
                    let _ = phone::save_lid_pn_mapping(&tx, &lid.to_string(), pn.user.as_str());
                }
            }
            if let Some(ref pn) = participant.phone_number {
                if participant.jid.is_lid() || phone::is_lid_jid(&part_jid_str) {
                    let _ = phone::save_lid_pn_mapping(&tx, &part_jid_str, pn.user.as_str());
                }
            }

            let is_lid = phone::is_lid_jid(&part_jid_str) || participant.jid.is_lid();
            let phone_opt = participant
                .phone_number
                .as_ref()
                .map(|pn| pn.user.to_string())
                .or_else(|| phone::resolve_lid_or_jid_to_phone(&tx, &part_jid_str))
                .or_else(|| {
                    if !is_lid {
                        phone::jid_to_phone(&part_jid_str)
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    if !is_lid {
                        phone::normalize_e164(&part_jid_str, Some("84"))
                    } else {
                        None
                    }
                });

            let ext_id = phone_opt.clone().unwrap_or_else(|| part_jid_str.clone());

            let lead_id = phone_opt
                .as_deref()
                .and_then(|p| Self::resolve_lead_by_phone(&tx, p).ok().flatten());
            let link_source = if lead_id.is_some() {
                "phone_match"
            } else {
                "unlinked"
            };

            let server_name = participant
                .details
                .as_ref()
                .and_then(|d| d.display_name.as_ref())
                .map(|d| d.to_string())
                .or_else(|| participant.username.as_ref().map(|u| u.to_string()));

            let is_valid_name = server_name.as_ref().is_some_and(|n| {
                let t = n.trim();
                !t.is_empty() && !t.starts_with("12036") && !t.starts_with("+12036")
            });

            let display_name = if let Some(name) = server_name.filter(|_| is_valid_name) {
                name
            } else if let Some(ref p) = phone_opt {
                format!("+{}", p)
            } else {
                "Member".to_string()
            };

            tx.execute(
                "INSERT INTO channel_identities (
                    lead_id, channel, channel_account_id, identity_namespace, external_id,
                    normalized_address, display_name, link_source, created_at, updated_at
                ) VALUES (
                    ?1, 'whatsapp', ?2, 'phone', ?3,
                    ?4, ?5, ?6, ?7, ?7
                )
                ON CONFLICT(channel, channel_account_id, identity_namespace, external_id) WHERE channel_account_id IS NOT NULL DO UPDATE SET
                    lead_id = COALESCE(EXCLUDED.lead_id, channel_identities.lead_id),
                    display_name = CASE
                        WHEN EXCLUDED.display_name != 'Member'
                         AND EXCLUDED.display_name NOT LIKE '+12036%'
                         AND EXCLUDED.display_name NOT LIKE '12036%'
                        THEN EXCLUDED.display_name
                        WHEN channel_identities.display_name IS NOT NULL
                         AND channel_identities.display_name != 'Member'
                         AND channel_identities.display_name NOT LIKE '+12036%'
                         AND channel_identities.display_name NOT LIKE '12036%'
                        THEN channel_identities.display_name
                        ELSE EXCLUDED.display_name
                    END,
                    link_source = CASE WHEN EXCLUDED.lead_id IS NOT NULL THEN 'phone_match' ELSE channel_identities.link_source END,
                    updated_at = EXCLUDED.updated_at",
                params![
                    lead_id,
                    channel_account_id,
                    ext_id,
                    ext_id,
                    display_name,
                    link_source,
                    now_str
                ],
            )?;

            let identity_id: i64 = tx.query_row(
                "SELECT id FROM channel_identities
                 WHERE channel = 'whatsapp' AND channel_account_id = ?1 AND identity_namespace = 'phone' AND external_id = ?2",
                params![channel_account_id, ext_id],
                |row| row.get(0),
            )?;

            active_identity_ids.push(identity_id);

            let role_str = if participant.is_super_admin() {
                "owner"
            } else if participant.is_admin() {
                "admin"
            } else {
                "member"
            };

            tx.execute(
                "INSERT INTO conversation_participants (
                    conversation_id, identity_id, participant_role, joined_at, left_at
                ) VALUES (?1, ?2, ?3, ?4, NULL)
                ON CONFLICT(conversation_id, identity_id) DO UPDATE SET
                    participant_role = EXCLUDED.participant_role,
                    left_at = NULL",
                params![conversation_id, identity_id, role_str, now_str],
            )?;

            // Re-link historical messages from this participant in the group to their newly synced identity
            if let Some(ref lid) = participant.lid {
                let lid_str = lid.to_string();
                let lid_user = phone::extract_lid_user(&lid_str).unwrap_or_else(|| lid_str.clone());
                let _ = tx.execute(
                    "UPDATE messages SET sender_identity_id = ?1
                     WHERE conversation_id = ?2
                       AND (
                           json_extract(raw_payload_json, '$.sender_jid') LIKE ?3
                           OR json_extract(raw_payload_json, '$.sender_jid') LIKE ?4
                       )",
                    params![
                        identity_id,
                        conversation_id,
                        format!("%{}%", lid_user),
                        format!("%{}%", lid_str)
                    ],
                );
            }
            if !part_jid_str.is_empty() {
                let _ = tx.execute(
                    "UPDATE messages SET sender_identity_id = ?1
                     WHERE conversation_id = ?2
                       AND json_extract(raw_payload_json, '$.sender_jid') LIKE ?3",
                    params![identity_id, conversation_id, format!("%{}%", part_jid_str)],
                );
            }
        }

        // Mark removed participants if active list is known
        if !active_identity_ids.is_empty() {
            let placeholders = (0..active_identity_ids.len())
                .map(|i| format!("?{}", i + 3))
                .collect::<Vec<_>>()
                .join(", ");
            let query = format!(
                "UPDATE conversation_participants
                 SET left_at = ?1
                 WHERE conversation_id = ?2 AND left_at IS NULL AND identity_id NOT IN ({placeholders})"
            );
            let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();
            params_vec.push(now_str.into());
            params_vec.push(conversation_id.into());
            for id in active_identity_ids {
                params_vec.push(id.into());
            }
            tx.execute(&query, rusqlite::params_from_iter(params_vec.iter()))?;
        }

        tx.commit()?;
        Ok(conversation_id)
    }

    /// Updates a participant's display name from USync verified name or contact lookup.
    pub fn update_participant_verified_name(
        conn: &mut Connection,
        channel_account_id: i64,
        jid_str: &str,
        verified_name: &str,
    ) -> Result<()> {
        let name = verified_name.trim();
        if name.is_empty()
            || name.starts_with("12036")
            || name.starts_with("+12036")
            || name == "Member"
            || name == "WhatsApp Group"
        {
            return Ok(());
        }
        let clean_phone = phone::resolve_lid_or_jid_to_phone(conn, jid_str)
            .or_else(|| phone::jid_to_phone(jid_str))
            .or_else(|| phone::normalize_e164(jid_str, Some("84")))
            .unwrap_or_else(|| jid_str.chars().filter(|c| c.is_ascii_digit()).collect());

        let now_str = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        conn.execute(
            "UPDATE channel_identities
             SET display_name = ?1, updated_at = ?2
             WHERE channel = 'whatsapp'
               AND channel_account_id = ?3
               AND (external_id = ?4 OR external_id = ?5 OR normalized_address = ?4 OR normalized_address = ?5)
               AND (display_name IS NULL OR display_name = 'Member' OR display_name LIKE '+%' OR display_name LIKE '12036%')",
            rusqlite::params![name, now_str, channel_account_id, jid_str, clean_phone],
        )?;

        Ok(())
    }

    /// Repairs historical messages in group chats where sender_identity_id was mistakenly linked to the group identity itself,
    /// or where raw_payload_json stored the group JID as sender_jid.
    /// Also re-links messages to their real participant identity if participant info is present in raw_payload_json.
    pub fn repair_misattributed_group_senders(
        conn: &mut Connection,
        channel_account_id: i64,
    ) -> Result<usize> {
        let tx = conn.transaction()?;

        // 1. Clear any raw_payload_json.sender_jid that was set to group JID (@g.us)
        let _ = tx.execute(
            "UPDATE messages
             SET raw_payload_json = json_set(raw_payload_json, '$.sender_jid', NULL)
             WHERE conversation_id IN (
                 SELECT id FROM channel_conversations
                 WHERE channel_account_id = ?1 AND kind = 'group'
             )
             AND json_extract(raw_payload_json, '$.sender_jid') LIKE '%@g.us'",
            rusqlite::params![channel_account_id],
        );

        // 2. Set sender_identity_id = NULL for all group messages where sender_identity_id was mistakenly pointing to the group identity
        let affected = tx.execute(
            "UPDATE messages
             SET sender_identity_id = NULL
             WHERE conversation_id IN (
                 SELECT id FROM channel_conversations
                 WHERE channel_account_id = ?1 AND kind = 'group'
             )
             AND sender_identity_id IN (
                 SELECT ci.id FROM channel_identities ci
                 JOIN channel_conversations cc ON cc.external_thread_id = ci.external_id AND cc.channel_account_id = ci.channel_account_id
                 WHERE ci.channel_account_id = ?1 AND cc.kind = 'group'
             )",
            rusqlite::params![channel_account_id],
        )?;

        // 3. Automatically re-link group messages that have a valid sender_jid (LID or Phone JID) in raw_payload_json
        // to the matching participant's channel_identities row
        let _ = tx.execute(
            "UPDATE messages
             SET sender_identity_id = (
                 SELECT ci.id FROM channel_identities ci
                 JOIN conversation_participants cp ON cp.identity_id = ci.id AND cp.conversation_id = messages.conversation_id
                 WHERE ci.channel = 'whatsapp'
                   AND ci.channel_account_id = ?1
                   AND (
                       ci.external_id = json_extract(messages.raw_payload_json, '$.sender_jid')
                       OR ci.normalized_address = json_extract(messages.raw_payload_json, '$.sender_jid')
                       OR json_extract(messages.raw_payload_json, '$.sender_jid') LIKE '%' || ci.external_id || '%'
                   )
                 LIMIT 1
             )
             WHERE conversation_id IN (
                 SELECT id FROM channel_conversations
                 WHERE channel_account_id = ?1 AND kind = 'group'
             )
             AND sender_identity_id IS NULL
             AND json_extract(raw_payload_json, '$.sender_jid') IS NOT NULL
             AND json_extract(raw_payload_json, '$.sender_jid') NOT LIKE '%@g.us'",
            rusqlite::params![channel_account_id],
        );

        tx.commit()?;
        Ok(affected)
    }
}
