//! Caption fallback for media messages.
//!
//! Zalo keeps the text typed under a photo or video in `content_json["title"]`
//! and leaves `text_content` empty. Consumers that only look at `text_content`
//! (the new-message webhook, the `read_timeline` view) would miss it.

use serde_json::Value;

/// Media kinds whose `title` is a user-written caption. Other kinds
/// (`unsupported`, `file`, ...) put a link title or a file name there, so they
/// are deliberately left out.
const CAPTION_KINDS: [&str; 2] = ["image", "video"];

/// Caption of an image or video message: `content["title"]`, trimmed, `None`
/// when blank or missing, cut to at most `max_chars` characters.
#[must_use]
pub fn media_caption(content_type: &str, content: &Value, max_chars: usize) -> Option<String> {
    if !CAPTION_KINDS.contains(&content_type) {
        return None;
    }
    let title = content.get("title")?.as_str()?.trim();
    if title.is_empty() {
        return None;
    }
    Some(title.chars().take(max_chars).collect())
}

/// Text of a message: `text_content` when it has any non-blank text,
/// otherwise the media caption.
#[must_use]
pub fn text_or_caption(
    content_type: &str,
    text_content: Option<&str>,
    content: &Value,
    max_chars: usize,
) -> Option<String> {
    match text_content {
        Some(text) if !text.trim().is_empty() => Some(text.to_string()),
        // Blank text: caption if any, else keep whatever was stored.
        other => media_caption(content_type, content, max_chars)
            .or_else(|| other.filter(|text| !text.is_empty()).map(str::to_string)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn image_and_video_use_trimmed_title() {
        let content = json!({ "title": "  TGBU4673319 cont này  " });
        assert_eq!(
            media_caption("image", &content, 100).as_deref(),
            Some("TGBU4673319 cont này")
        );
        assert_eq!(
            media_caption("video", &content, 100).as_deref(),
            Some("TGBU4673319 cont này")
        );
    }

    #[test]
    fn blank_or_missing_title_is_none() {
        assert_eq!(
            media_caption("image", &json!({ "title": "  \n " }), 100),
            None
        );
        assert_eq!(media_caption("image", &json!({ "title": 5 }), 100), None);
        assert_eq!(media_caption("image", &json!({}), 100), None);
        assert_eq!(media_caption("image", &Value::Null, 100), None);
    }

    #[test]
    fn other_kinds_never_use_title() {
        let content = json!({ "title": "https://example.test/x" });
        for kind in ["unsupported", "file", "sticker", "text", "audio"] {
            assert_eq!(media_caption(kind, &content, 100), None, "{kind}");
        }
    }

    #[test]
    fn caption_is_cut_by_chars_not_bytes() {
        let content = json!({ "title": "ảảảảả" });
        assert_eq!(media_caption("image", &content, 3).as_deref(), Some("ảảả"));
    }

    #[test]
    fn text_wins_over_caption() {
        let content = json!({ "title": "caption" });
        assert_eq!(
            text_or_caption("image", Some("text"), &content, 100).as_deref(),
            Some("text")
        );
        assert_eq!(
            text_or_caption("image", Some("  "), &content, 100).as_deref(),
            Some("caption")
        );
        assert_eq!(
            text_or_caption("image", None, &content, 100).as_deref(),
            Some("caption")
        );
        assert_eq!(text_or_caption("unsupported", None, &content, 100), None);
    }
}
