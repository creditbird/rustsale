use crate::conversation::error::{ConversationError, ConversationErrorCode, ConversationResult};
use crate::conversation::types::ConversationMentionInput;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionParticipant {
    pub identity_id: i64,
    pub external_id: Option<String>,
    pub display_name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedConversationMention {
    pub pos: usize,
    pub len: usize,
    pub target_identity_id: Option<i64>,
    pub target_external_id: Option<String>,
    pub display_name: Option<String>,
    pub is_all: bool,
}

/// Returns the length of a string in UTF-16 code units.
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Extracts a substring defined by a UTF-16 code unit offset `start` and length `len`.
/// Returns `None` if the range is out of bounds or splits a multi-unit character (surrogate pair).
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
        (Some(b_start), Some(b_end)) if b_start <= b_end && b_end <= value.len() => {
            Some(value[b_start..b_end].to_string())
        }
        _ => None,
    }
}

/// Validates outbound mentions against text and active group participants.
pub fn validate_mentions(
    text: &str,
    inputs: &[ConversationMentionInput],
    participants: &[MentionParticipant],
) -> ConversationResult<Vec<ResolvedConversationMention>> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }

    if participants.is_empty() {
        return Err(ConversationError::new(
            ConversationErrorCode::MentionDirectConversationUnsupported,
            "Mentions are not supported in direct conversations",
        ));
    }

    if inputs.len() > 20 {
        return Err(ConversationError::new(
            ConversationErrorCode::InvalidMentionRange,
            "Exceeded maximum limit of 20 mentions per message",
        ));
    }

    let text_len_u16 = utf16_len(text);
    let mut resolved = Vec::with_capacity(inputs.len());
    let mut last_end = 0usize;

    for (idx, input) in inputs.iter().enumerate() {
        if input.len == 0 {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidMentionRange,
                "Mention length must be greater than zero",
            ));
        }

        let end = input.pos.checked_add(input.len).ok_or_else(|| {
            ConversationError::new(
                ConversationErrorCode::InvalidMentionRange,
                "Mention position overflow",
            )
        })?;

        if end > text_len_u16 {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidMentionRange,
                "Mention range out of bounds",
            ));
        }

        if idx > 0 && input.pos < last_end {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidMentionRange,
                "Mention ranges must be sorted and non-overlapping",
            ));
        }
        last_end = end;

        let display_text = utf16_slice(text, input.pos, input.len).ok_or_else(|| {
            ConversationError::new(
                ConversationErrorCode::InvalidMentionRange,
                "Mention range does not align with character boundaries",
            )
        })?;

        if !display_text.starts_with('@') {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidMentionRange,
                "Mention text must start with @",
            ));
        }

        if display_text.chars().any(|c| c.is_control()) {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidMentionRange,
                "Mention text cannot contain control characters",
            ));
        }

        if display_text.chars().count() > 100 {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidMentionRange,
                "Mention display text exceeds maximum length of 100 characters",
            ));
        }

        if input.is_all {
            resolved.push(ResolvedConversationMention {
                pos: input.pos,
                len: input.len,
                target_identity_id: None,
                target_external_id: None,
                display_name: Some("all".to_string()),
                is_all: true,
            });
        } else {
            let target_id = input.target_identity_id.ok_or_else(|| {
                ConversationError::new(
                    ConversationErrorCode::MentionParticipantNotFound,
                    "Mention input requires target identity ID",
                )
            })?;

            let p = participants
                .iter()
                .find(|p| p.identity_id == target_id)
                .ok_or_else(|| {
                    ConversationError::new(
                        ConversationErrorCode::MentionParticipantNotFound,
                        format!("Target participant identity {} not found", target_id),
                    )
                })?;

            if !p.is_active {
                return Err(ConversationError::new(
                    ConversationErrorCode::MentionParticipantNotFound,
                    format!("Target participant identity {} is inactive", target_id),
                ));
            }

            resolved.push(ResolvedConversationMention {
                pos: input.pos,
                len: input.len,
                target_identity_id: Some(p.identity_id),
                target_external_id: p.external_id.clone(),
                display_name: Some(p.display_name.clone()),
                is_all: false,
            });
        }
    }

    Ok(resolved)
}
