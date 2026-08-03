use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeReaction {
    pub emoji: String,
    pub code: String,
    pub removed: bool,
    pub actor_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeForwardMessage {
    pub provider_msg_id: String,
    pub client_msg_id: String,
    pub content_type: String,
    pub original_msg_id: Option<String>,
    pub is_forwarded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeMention {
    pub target_uid: String,
    pub utf16_offset: usize,
    pub utf16_len: usize,
    pub mention_text: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Phase2AProbeSummary {
    pub total_fixtures: usize,
    pub reaction_fixtures: usize,
    pub forward_fixtures: usize,
    pub mention_fixtures: usize,
    pub sanitized: bool,
}

/// Slices a string using UTF-16 code unit offsets and lengths.
/// Returns `None` if the requested range is out of bounds or falls on an invalid boundary.
pub fn utf16_slice(value: &str, start: usize, len: usize) -> Option<String> {
    let mut utf16_units = 0usize;
    let mut start_byte = None;
    let mut end_byte = None;

    let end_target = start.checked_add(len)?;

    if start == 0 {
        start_byte = Some(0);
    }
    if end_target == 0 {
        end_byte = Some(0);
    }

    for (byte_idx, ch) in value.char_indices() {
        if utf16_units == start && start_byte.is_none() {
            start_byte = Some(byte_idx);
        }
        if utf16_units == end_target && end_byte.is_none() {
            end_byte = Some(byte_idx);
        }
        utf16_units += ch.len_utf16();
    }

    if utf16_units == start && start_byte.is_none() {
        start_byte = Some(value.len());
    }
    if utf16_units == end_target && end_byte.is_none() {
        end_byte = Some(value.len());
    }

    match (start_byte, end_byte) {
        (Some(s), Some(e)) if s <= e && e <= value.len() => Some(value[s..e].to_string()),
        _ => None,
    }
}

pub fn classify_reaction_fixture(value: &Value) -> anyhow::Result<ProbeReaction> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Expected JSON object for reaction fixture"))?;

    let code = object
        .get("code")
        .or_else(|| object.get("rIcon"))
        .or_else(|| object.get("icon"))
        .or_else(|| object.get("reaction"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let emoji = object
        .get("emoji")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| map_code_to_emoji(&code));

    let removed = object
        .get("removed")
        .and_then(Value::as_bool)
        .or_else(|| object.get("is_remove").and_then(Value::as_bool))
        .or_else(|| object.get("isRemove").and_then(Value::as_bool))
        .unwrap_or_else(|| {
            let action = object.get("action").and_then(Value::as_str).unwrap_or("");
            let r_type = object.get("rType").and_then(Value::as_i64).unwrap_or(1);
            action == "remove" || r_type == 0 || code == "-1" || code.is_empty()
        });

    let actor_id = object
        .get("actor_id")
        .or_else(|| object.get("actorId"))
        .or_else(|| object.get("uId"))
        .or_else(|| object.get("uid"))
        .or_else(|| object.get("userId"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    Ok(ProbeReaction {
        emoji,
        code,
        removed,
        actor_id,
    })
}

pub fn map_code_to_emoji(code: &str) -> String {
    match code {
        "/:heart" | "heart" | "/-heart" => "❤️".to_string(),
        "/-haha" | "haha" | "/:haha" => "😆".to_string(),
        "/-like" | "like" | "/:like" => "👍".to_string(),
        "/-sad" | "sad" | "/:sad" => "😢".to_string(),
        "/-wow" | "wow" | "/:wow" => "😮".to_string(),
        "/-angry" | "angry" | "/:angry" => "😡".to_string(),
        other => other.to_string(),
    }
}

pub fn classify_forward_fixture(value: &Value) -> anyhow::Result<ProbeForwardMessage> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Expected JSON object for forward fixture"))?;

    let provider_msg_id = object
        .get("msg_id")
        .or_else(|| object.get("msgId"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let client_msg_id = object
        .get("cli_msg_id")
        .or_else(|| object.get("cliMsgId"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let content_type = object
        .get("content_type")
        .or_else(|| object.get("contentType"))
        .or_else(|| object.get("msgType"))
        .and_then(Value::as_str)
        .unwrap_or("text")
        .to_string();

    let original_msg_id = object
        .get("original_msg_id")
        .or_else(|| object.get("originalMsgId"))
        .and_then(Value::as_str)
        .map(String::from);

    let is_forwarded = object
        .get("is_forwarded")
        .or_else(|| object.get("isForwarded"))
        .and_then(Value::as_bool)
        .unwrap_or(original_msg_id.is_some());

    Ok(ProbeForwardMessage {
        provider_msg_id,
        client_msg_id,
        content_type,
        original_msg_id,
        is_forwarded,
    })
}

pub fn extract_mentions_fixture(value: &Value) -> anyhow::Result<Vec<ProbeMention>> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Expected JSON object for mention fixture"))?;

    let text = object.get("text").and_then(Value::as_str).unwrap_or("");

    let mentions_array = object
        .get("mentions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut result = Vec::new();
    for mention_val in mentions_array {
        if let Some(m_obj) = mention_val.as_object() {
            let target_uid = m_obj
                .get("target_uid")
                .or_else(|| m_obj.get("uid"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();

            let utf16_offset = m_obj
                .get("utf16_offset")
                .or_else(|| m_obj.get("offset"))
                .or_else(|| m_obj.get("pos"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;

            let utf16_len = m_obj
                .get("utf16_len")
                .or_else(|| m_obj.get("len"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;

            let mention_text = utf16_slice(text, utf16_offset, utf16_len);

            result.push(ProbeMention {
                target_uid,
                utf16_offset,
                utf16_len,
                mention_text,
            });
        }
    }

    Ok(result)
}

pub fn is_sanitized_fixture(value: &Value) -> bool {
    let text = value.to_string();
    let forbidden_keys = ["zpw_sek", "_zlang", "cookie", "password", "auth_token"];
    for key in forbidden_keys {
        if text.contains(key) {
            return false;
        }
    }
    let phone_pattern = regex::Regex::new(r#"\b(84|0)[35789]\d{8}\b"#).unwrap();
    if phone_pattern.is_match(&text) {
        return false;
    }
    true
}

pub fn generate_phase2a_probe_summary(fixtures: &[Value]) -> Phase2AProbeSummary {
    let mut summary = Phase2AProbeSummary {
        total_fixtures: fixtures.len(),
        sanitized: true,
        ..Default::default()
    };

    for fixture in fixtures {
        if !is_sanitized_fixture(fixture) {
            summary.sanitized = false;
        }
        if fixture.get("code").is_some()
            || fixture.get("active_reactions").is_some()
            || fixture.get("reactions_by_msg").is_some()
        {
            summary.reaction_fixtures += 1;
        } else if fixture.get("is_forwarded").is_some()
            || (fixture.is_array()
                && fixture
                    .as_array()
                    .is_some_and(|a| a.iter().any(|item| item.get("is_forwarded").is_some())))
        {
            summary.forward_fixtures += 1;
        } else if fixture.get("mentions").is_some()
            || (fixture.is_array()
                && fixture
                    .as_array()
                    .is_some_and(|a| a.iter().any(|item| item.get("case_name").is_some())))
        {
            summary.mention_fixtures += 1;
        }
    }

    summary
}
