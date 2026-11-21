//! Readable text for Zalo messages stored as `content_type = "unsupported"`.
//!
//! Those rows keep only the provider `content` JSON (no message type), so the
//! kind is inferred from its shape. Formatted text (`action: "rtf"`) is real
//! text the customer typed. Everything else gets a bracketed Vietnamese label
//! so nobody mistakes it for something the customer wrote; a link or card
//! title only ever appears inside such a label. Unknown shapes are never
//! silent: they get a `[không đọc được: ...]` label and one log line (tag only,
//! never message content or URLs).

use serde_json::Value;

const TITLE_MAX_CHARS: usize = 300;
const TAG_MAX_CHARS: usize = 40;

/// What an unsupported message turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Described {
    /// Text the customer wrote (formatted message): shown as is.
    Text(String),
    /// A known non-text kind, already bracketed.
    Label(String),
    /// A shape this code does not know; `tag` is a short, sanitised hint.
    Unknown { tag: String },
}

fn non_blank(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
}

fn labelled(label: &str, title: Option<&str>) -> Described {
    let title = title.map(str::trim).filter(|t| !t.is_empty());
    Described::Label(match title {
        Some(title) => format!(
            "{label} {}",
            title.chars().take(TITLE_MAX_CHARS).collect::<String>()
        ),
        None => label.to_string(),
    })
}

fn params_contain(content: &Value, needle: &str) -> bool {
    content
        .get("params")
        .and_then(Value::as_str)
        .is_some_and(|params| params.contains(needle))
}

fn sanitised_tag(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
        .take(TAG_MAX_CHARS)
        .collect()
}

fn shape_name(content: &Value) -> &'static str {
    match content {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Classifies the stored provider content of an unsupported message.
#[must_use]
pub fn describe(content: &Value) -> Described {
    let parsed = content
        .as_str()
        .filter(|s| s.trim_start().starts_with(['{', '[']))
        .and_then(|s| serde_json::from_str::<Value>(s).ok());
    let content = parsed.as_ref().unwrap_or(content);

    if let Some(items) = content.as_array() {
        let is_delete = items.iter().any(|item| {
            item.get("clientDelMsgId").is_some() || item.get("globalDelMsgId").is_some()
        });
        return if is_delete {
            Described::Label("[đã xóa một tin]".to_string())
        } else {
            Described::Unknown {
                tag: "array".to_string(),
            }
        };
    }
    if !content.is_object() {
        return Described::Unknown {
            tag: shape_name(content).to_string(),
        };
    }

    let action = content
        .get("action")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let title = non_blank(content.get("title"));
    let description = non_blank(content.get("description"));

    match action {
        "rtf" => match title.or(description) {
            Some(text) => Described::Text(text.to_string()),
            None => Described::Label("[tin định dạng không có chữ]".to_string()),
        },
        "msginfo.actionlist" => labelled("[thông báo]", title),
        "recommened.link" => labelled("[link]", title.or(description)),
        "recommened.misscall" => Described::Label("[cuộc gọi nhỡ]".to_string()),
        "recommened.calltime" => Described::Label("[cuộc gọi]".to_string()),
        "recommened.user" | "show.profile" => labelled("[danh thiếp]", title),
        "zinstant.bankcard" => Described::Label("[thẻ ngân hàng]".to_string()),
        "recommend.gif" => Described::Label("[ảnh GIF]".to_string()),
        "create" | "vote" | "close" | "update" | "share" if params_contain(content, "pollId") => {
            Described::Label("[bình chọn]".to_string())
        }
        "" => describe_without_action(content, title, description),
        other => Described::Unknown {
            tag: sanitised_tag(other),
        },
    }
}

fn describe_without_action(
    content: &Value,
    title: Option<&str>,
    description: Option<&str>,
) -> Described {
    let has = |key: &str| content.get(key).is_some();
    if has("globalMsgId") && (has("srcId") || has("deleteMsg")) {
        return Described::Label("[đã thu hồi một tin]".to_string());
    }
    if params_contain(content, "latitude") {
        return labelled("[vị trí]", title.or(description));
    }
    let href_is_gif = non_blank(content.get("href")).is_some_and(|href| href.contains(".gif"));
    if href_is_gif || params_contain(content, "thumb_height") {
        return Described::Label("[ảnh GIF]".to_string());
    }
    if params_contain(content, "zinstantdata_id") {
        return Described::Label("[thẻ nội dung]".to_string());
    }
    Described::Unknown {
        tag: "object".to_string(),
    }
}

/// Text to show for an unsupported message: never empty, never silent.
#[must_use]
pub fn unsupported_text(content: &Value, max_chars: usize) -> String {
    let text = match describe(content) {
        Described::Text(text) | Described::Label(text) => text,
        Described::Unknown { tag } => {
            tracing::warn!(tag = %tag, "zalo.unsupported_message.unknown_shape");
            format!("[không đọc được: {tag}]")
        }
    };
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn label(content: Value) -> String {
        match describe(&content) {
            Described::Label(text) | Described::Text(text) => text,
            Described::Unknown { tag } => format!("UNKNOWN:{tag}"),
        }
    }

    #[test]
    fn rtf_is_the_typed_text_with_newlines() {
        let content = json!({ "action": "rtf", "title": "dòng 1\ndòng 2", "description": "" });
        assert_eq!(
            describe(&content),
            Described::Text("dòng 1\ndòng 2".to_string())
        );
    }

    #[test]
    fn rtf_with_blank_title_falls_back_to_description_then_label() {
        let content = json!({ "action": "rtf", "title": " ", "description": "chữ" });
        assert_eq!(describe(&content), Described::Text("chữ".to_string()));
        let content = json!({ "action": "rtf", "title": "", "description": "" });
        assert_eq!(label(content), "[tin định dạng không có chữ]");
    }

    #[test]
    fn pin_notice_is_a_labelled_notice() {
        let content = json!({
            "action": "msginfo.actionlist",
            "title": "Vận Tải Quốc Anh đã ghim tin nhắn https://x.test/...",
            "params": "{}"
        });
        assert_eq!(
            label(content),
            "[thông báo] Vận Tải Quốc Anh đã ghim tin nhắn https://x.test/..."
        );
    }

    #[test]
    fn known_kinds_have_vietnamese_labels() {
        let cases = [
            (
                json!({"action":"recommened.link","title":"Bài viết"}),
                "[link] Bài viết",
            ),
            (
                json!({"action":"recommened.misscall","title":"sendBubbleMessage"}),
                "[cuộc gọi nhỡ]",
            ),
            (
                json!({"action":"recommened.calltime","title":"sendBubbleMessage"}),
                "[cuộc gọi]",
            ),
            (
                json!({"action":"recommened.user","title":"Hiếu"}),
                "[danh thiếp] Hiếu",
            ),
            (
                json!({"action":"show.profile","title":"Sinh nhật"}),
                "[danh thiếp] Sinh nhật",
            ),
            (json!({"action":"zinstant.bankcard"}), "[thẻ ngân hàng]"),
            (json!({"action":"recommend.gif","title":"NPH"}), "[ảnh GIF]"),
            (
                json!({"action":"","href":"https://z.test/a.gif","params":"{}"}),
                "[ảnh GIF]",
            ),
            (
                json!({"action":"","params":"{\"thumb_height\":220}"}),
                "[ảnh GIF]",
            ),
            (
                json!({"action":"","params":"{\"zinstantdata_id\":\"t=1\"}"}),
                "[thẻ nội dung]",
            ),
            (
                json!({"action":"","title":"","description":"93 Trương Đình Hội","params":"{\"latitude\":10.7}"}),
                "[vị trí] 93 Trương Đình Hội",
            ),
        ];
        for (content, expected) in cases {
            assert_eq!(label(content.clone()), expected, "{content}");
        }
    }

    #[test]
    fn link_label_uses_title_then_description() {
        let content =
            json!({"action":"recommened.link","title":"","description":"★★★★☆ · Quán cà phê"});
        assert_eq!(label(content), "[link] ★★★★☆ · Quán cà phê");
        let content = json!({"action":"recommened.link","title":"Tiêu đề","description":"mô tả"});
        assert_eq!(label(content), "[link] Tiêu đề");
        let content = json!({"action":"recommened.link","title":" ","description":" "});
        assert_eq!(label(content), "[link]");
    }

    #[test]
    fn polls_need_a_poll_id() {
        for action in ["create", "vote", "close", "update", "share"] {
            let content = json!({"action": action, "params": "{\"pollId\":1}"});
            assert_eq!(label(content), "[bình chọn]", "{action}");
        }
        let content = json!({"action": "vote", "params": "{}"});
        assert_eq!(label(content), "UNKNOWN:vote");
    }

    #[test]
    fn undo_and_delete_shapes() {
        let undo = json!({"cliMsgId":1,"deleteMsg":0,"destId":2,"globalMsgId":3,"srcId":4});
        assert_eq!(label(undo), "[đã thu hồi một tin]");
        let delete = json!([{"actionType":0,"clientDelMsgId":1,"globalDelMsgId":0}]);
        assert_eq!(label(delete), "[đã xóa một tin]");
    }

    #[test]
    fn content_stored_as_json_string_is_parsed() {
        let content = Value::String(r#"{"action":"rtf","title":"abc"}"#.to_string());
        assert_eq!(describe(&content), Described::Text("abc".to_string()));
    }

    #[test]
    fn unknown_shapes_are_never_silent_and_never_show_title_or_url() {
        let content = json!({"action":"brand.new","title":"https://secret.test/t?k=1"});
        let text = unsupported_text(&content, 100);
        assert_eq!(text, "[không đọc được: brand.new]");
        assert_eq!(
            unsupported_text(&json!({}), 100),
            "[không đọc được: object]"
        );
        assert_eq!(
            unsupported_text(&json!([1]), 100),
            "[không đọc được: array]"
        );
        assert_eq!(
            unsupported_text(&Value::Null, 100),
            "[không đọc được: null]"
        );
    }

    #[test]
    fn unknown_action_tag_is_sanitised_and_bounded() {
        let content = json!({"action": format!("a b\n<script>{}", "x".repeat(100))});
        let text = unsupported_text(&content, 500);
        assert!(text.starts_with("[không đọc được: abscriptxxx"));
        assert!(!text.contains(['<', '>', '\n']));
        assert!(text.chars().count() <= "[không đọc được: ]".chars().count() + TAG_MAX_CHARS);
    }

    #[test]
    fn long_text_is_cut_by_chars() {
        let content = json!({"action":"rtf","title":"ảảảảả"});
        assert_eq!(unsupported_text(&content, 3), "ảảả");
    }
}

/// Read-only census on a COPY of `rustsale.db` (never the live file):
/// `RUSTSALE_UNSUPPORTED_CHECK_DB=/tmp/copy/rustsale.db cargo test --lib unsupported_census_on_db_copy -- --ignored --nocapture`
#[cfg(test)]
mod db_copy_census {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    #[ignore = "needs RUSTSALE_UNSUPPORTED_CHECK_DB pointing at a copy of rustsale.db"]
    fn unsupported_census_on_db_copy() {
        let path = std::env::var("RUSTSALE_UNSUPPORTED_CHECK_DB").expect("set the DB copy path");
        let live = std::env::var("HOME")
            .map(|home| format!("{home}/.local/share/com.rustsale.crm"))
            .unwrap_or_default();
        assert!(
            !path.starts_with(&live),
            "refusing to open the live database"
        );
        let db = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open copy read-only");
        let mut stmt = db
            .prepare(
                "SELECT json_extract(raw_payload_json,'$.msgType'), content_json, text_content \
                 FROM messages WHERE content_type = 'unsupported'",
            )
            .expect("prepare");
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ))
            })
            .expect("query");
        // (msgType, action) -> (before blank, before raw text, after Text, after Label, after Unknown)
        let mut table: BTreeMap<(String, String), [u32; 5]> = BTreeMap::new();
        for row in rows {
            let (msg_type, content_json, text) = row.expect("row");
            let content: Value = serde_json::from_str(&content_json).unwrap_or(Value::Null);
            let action = content
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("-")
                .to_string();
            let entry = table.entry((msg_type, action)).or_default();
            entry[if text.trim().is_empty() { 0 } else { 1 }] += 1;
            entry[match describe(&content) {
                Described::Text(_) => 2,
                Described::Label(_) => 3,
                Described::Unknown { .. } => 4,
            }] += 1;
        }
        println!("msgType | action | before: blank | before: raw text | after: typed text | after: label | after: không đọc được");
        let mut totals = [0u32; 5];
        for ((msg_type, action), counts) in &table {
            println!(
                "{msg_type} | {action} | {} | {} | {} | {} | {}",
                counts[0], counts[1], counts[2], counts[3], counts[4]
            );
            for (total, count) in totals.iter_mut().zip(counts) {
                *total += count;
            }
        }
        println!(
            "TOTAL | | {} | {} | {} | {} | {}",
            totals[0], totals[1], totals[2], totals[3], totals[4]
        );
    }
}
