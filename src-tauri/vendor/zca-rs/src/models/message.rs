use serde::{Deserialize, Deserializer, Serialize, Serializer};

fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        _ => Ok(String::new()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttachmentSource {
    Url(String),
    Buffer {
        data: Vec<u8>,
        filename: String,
        metadata: AttachmentMetadata,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMetadata {
    pub total_size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoardType {
    Note = 1,
    PinnedMessage = 2,
    Poll = 3,
}

impl Serialize for BoardType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for BoardType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            1 => Ok(BoardType::Note),
            2 => Ok(BoardType::PinnedMessage),
            3 => Ok(BoardType::Poll),
            _ => Ok(BoardType::Note),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollDetail {
    pub creator: String,
    pub question: String,
    pub options: Vec<PollOptions>,
    pub joined: bool,
    pub closed: bool,
    pub poll_id: u64,
    pub allow_multi_choices: bool,
    pub allow_add_new_option: bool,
    pub is_anonymous: bool,
    pub poll_type: u32,
    pub created_time: u64,
    pub updated_time: u64,
    pub expired_time: u64,
    pub is_hide_vote_preview: bool,
    pub num_vote: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollOptions {
    pub content: String,
    pub votes: u32,
    pub voted: bool,
    pub voters: Vec<String>,
    pub option_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDetail {
    pub id: String,
    pub r#type: u32,
    pub color: u32,
    pub emoji: String,
    pub start_time: u64,
    pub duration: u64,
    pub params: NoteParams,
    pub creator_id: String,
    pub editor_id: String,
    pub create_time: u64,
    pub edit_time: u64,
    pub repeat: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteParams {
    pub title: String,
    pub extra: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedMessageDetail {
    pub id: String,
    pub r#type: u32,
    pub color: u32,
    pub emoji: String,
    pub start_time: u64,
    pub duration: u64,
    pub params: serde_json::Value,
    pub creator_id: String,
    pub editor_id: String,
    pub create_time: u64,
    pub edit_time: u64,
    pub repeat: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TAttachmentContent {
    pub title: String,
    pub description: String,
    pub href: String,
    pub thumb: String,
    pub childnumber: u32,
    pub action: String,
    pub params: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TMessage {
    pub action_id: String,
    pub msg_id: String,
    pub cli_msg_id: String,
    pub msg_type: String,
    pub uid_from: String,
    pub id_to: String,
    pub d_name: String,
    pub ts: String,
    pub status: i32,
    pub content: serde_json::Value,
    pub notify: String,
    pub ttl: i32,
    pub user_id: String,
    pub uin: String,
    #[serde(default)]
    pub top_out: Option<String>,
    #[serde(default)]
    pub top_out_time_out: Option<String>,
    #[serde(default)]
    pub top_out_impr_time_out: Option<String>,
    pub property_ext: Option<TPropertyExt>,
    #[serde(default)]
    pub params_ext: TParamsExt,
    pub cmd: i32,
    pub st: i32,
    pub at: i32,
    pub real_msg_id: String,
    pub quote: Option<TQuote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TPropertyExt {
    pub color: i32,
    pub size: i32,
    pub r#type: i32,
    pub sub_type: i32,
    pub ext: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TParamsExt {
    pub count_unread: i32,
    pub contain_type: i32,
    pub platform_type: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TGroupMessage {
    #[serde(flatten)]
    pub base: TMessage,
    pub mentions: Option<Vec<TMention>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TMention {
    pub uid: String,
    pub pos: i32,
    pub len: i32,
    pub r#type: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TQuote {
    #[serde(deserialize_with = "deserialize_string_or_number")]
    pub owner_id: String,
    pub cli_msg_id: u64,
    pub global_msg_id: u64,
    pub cli_msg_type: u32,
    pub ts: u64,
    pub msg: String,
    pub attach: String,
    pub from_d: String,
    pub ttl: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub r#type: crate::models::ThreadType,
    pub data: TMessage,
    pub thread_id: String,
    pub is_self: bool,
}

impl UserMessage {
    pub fn new(uid: &str, mut data: TMessage) -> Self {
        let is_self = data.uid_from == "0" || (!uid.is_empty() && data.uid_from == uid);
        let thread_id = if is_self {
            data.id_to.clone()
        } else {
            data.uid_from.clone()
        };

        if data.id_to == "0" {
            data.id_to = uid.to_owned();
        }
        if data.uid_from == "0" {
            data.uid_from = uid.to_owned();
        }

        Self {
            r#type: crate::models::ThreadType::User,
            data,
            thread_id,
            is_self,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMessage {
    pub r#type: crate::models::ThreadType,
    pub data: TGroupMessage,
    pub thread_id: String,
    pub is_self: bool,
}

impl GroupMessage {
    pub fn new(uid: &str, mut data: TGroupMessage) -> Self {
        let is_self = data.base.uid_from == "0" || (!uid.is_empty() && data.base.uid_from == uid);
        let thread_id = data.base.id_to.clone();

        if data.base.uid_from == "0" {
            data.base.uid_from = uid.to_owned();
        }

        Self {
            r#type: crate::models::ThreadType::Group,
            data,
            thread_id,
            is_self,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "0")]
    User(UserMessage),
    #[serde(rename = "1")]
    Group(GroupMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickMessage {
    pub id: u64,
    pub keyword: String,
    pub r#type: u32,
    pub created_time: u64,
    pub last_modified: u64,
    pub message: QuickMessageContent,
    pub media: Option<QuickMessageMedia>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickMessageContent {
    pub title: String,
    pub params: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickMessageMedia {
    pub items: Vec<QuickMessageMediaItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickMessageMediaItem {
    pub r#type: u32,
    pub photo_id: u64,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub preview_thumb: String,
    pub raw_url: String,
    pub thumb_url: String,
    pub normal_url: String,
    pub hd_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StickerDetail {
    pub id: u32,
    pub cate_id: u32,
    pub r#type: u32,
    pub text: String,
    pub uri: String,
    pub fkey: u32,
    pub status: u32,
    pub sticker_url: String,
    pub sticker_sprite_url: String,
    pub sticker_webp_url: Option<String>,
    pub total_frames: u32,
    pub duration: u32,
    pub effect_id: u32,
    pub checksum: String,
    pub ext: u32,
    pub source: u32,
    pub fss: serde_json::Value,
    pub fss_info: serde_json::Value,
    pub version: u32,
    pub ext_info: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerBasic {
    pub r#type: u32,
    pub cate_id: u32,
    pub sticker_id: u32,
}

fn deserialize_string_or_number_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::String(s) => s.parse::<u64>().map_err(serde::de::Error::custom),
        serde_json::Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| serde::de::Error::custom("not a u64")),
        _ => Ok(0),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStyle {
    Bold,
    Italic,
    Underline,
    StrikeThrough,
    Red,
    Orange,
    Yellow,
    Green,
    Small,
    Big,
    UnorderedList,
    OrderedList,
    Indent(u32),
}

impl std::fmt::Display for TextStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TextStyle::Bold => "b",
            TextStyle::Italic => "i",
            TextStyle::Underline => "u",
            TextStyle::StrikeThrough => "s",
            TextStyle::Red => "c_db342e",
            TextStyle::Orange => "c_f27806",
            TextStyle::Yellow => "c_f7b503",
            TextStyle::Green => "c_15a85f",
            TextStyle::Small => "f_13",
            TextStyle::Big => "f_18",
            TextStyle::UnorderedList => "lst_1",
            TextStyle::OrderedList => "lst_2",
            TextStyle::Indent(size) => return write!(f, "ind_{}0", size),
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageStyle {
    pub start: u32,
    pub len: u32,
    pub st: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Urgency {
    Default = 0,
    Important = 1,
    Urgent = 2,
}

impl Serialize for Urgency {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for Urgency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            0 => Ok(Urgency::Default),
            1 => Ok(Urgency::Important),
            2 => Ok(Urgency::Urgent),
            _ => Ok(Urgency::Default),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMention {
    pub pos: i32,
    pub uid: String,
    pub len: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageQuote {
    pub content: serde_json::Value,
    pub msg_type: String,
    pub property_ext: Option<serde_json::Value>,
    pub uid_from: String,
    pub msg_id: String,
    pub cli_msg_id: String,
    pub ts: String,
    pub ttl: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContent {
    pub msg: String,
    pub styles: Option<Vec<MessageStyle>>,
    pub urgency: Option<Urgency>,
    pub quote: Option<SendMessageQuote>,
    pub mentions: Option<Vec<MessageMention>>,
    pub attachments: Option<Vec<AttachmentSource>>,
    pub ttl: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResult {
    #[serde(deserialize_with = "deserialize_string_or_number_u64")]
    pub msg_id: u64,
    #[serde(default, deserialize_with = "deserialize_string_or_number_u64")]
    pub cli_msg_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message: Option<SendMessageResult>,
    pub attachment: Vec<SendMessageResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReactionType {
    Standard(crate::models::Reactions),
    Custom {
        r_type: i32,
        source: i32,
        icon: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardReference {
    pub id: String,
    pub ts: u64,
    pub log_src_type: i32,
    pub fw_lvl: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionRequest {
    pub client_id: Option<u64>,
    pub reaction: Option<ReactionType>,
    pub msg_id: String,
    pub cli_msg_id: String,
    pub thread_id: String,
    pub thread_type: crate::models::ThreadType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardMessageRequest {
    pub client_id: Option<u64>,
    pub message: String,
    pub thread_id: String,
    pub thread_type: crate::models::ThreadType,
    pub reference: Option<ForwardReference>,
    pub ttl: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardMessageReceipt {
    pub msg_id: String,
    pub cli_msg_id: String,
}

impl ForwardMessageReceipt {
    pub fn normalize(val: &serde_json::Value) -> Result<Self, crate::error::ZaloError> {
        let root = if let Some(data) = val.get("data") {
            if data.is_object() {
                data
            } else if let Some(first) = data.as_array().and_then(|a| a.first()) {
                first
            } else {
                val
            }
        } else if let Some(msg) = val.get("message") {
            if msg.is_object() {
                msg
            } else {
                val
            }
        } else {
            val
        };

        let extract_str = |keys: &[&str]| -> Option<String> {
            for key in keys {
                if let Some(v) = root.get(key) {
                    if let Some(s) = v.as_str() {
                        if !s.is_empty() {
                            return Some(s.to_string());
                        }
                    } else if let Some(n) = v.as_u64() {
                        return Some(n.to_string());
                    } else if let Some(n) = v.as_i64() {
                        return Some(n.to_string());
                    }
                }
            }
            None
        };

        let msg_id = extract_str(&["msgId", "msg_id", "gMsgID", "globalMsgId"]).unwrap_or_default();
        let cli_msg_id =
            extract_str(&["cliMsgId", "cli_msg_id", "cMsgID", "clientId"]).unwrap_or_default();

        Ok(Self { msg_id, cli_msg_id })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn send_message_result_defaults_missing_client_id() {
        let result: SendMessageResult = serde_json::from_value(serde_json::json!({
            "msgId": "101"
        }))
        .unwrap();
        assert_eq!(result.msg_id, 101);
        assert_eq!(result.cli_msg_id, 0);
    }

    #[test]
    fn group_history_message_defaults_missing_params_ext() {
        let message: TMessage = serde_json::from_value(serde_json::json!({
            "actionId": "action-1",
            "msgId": "message-1",
            "cliMsgId": "client-1",
            "msgType": "webchat",
            "uidFrom": "peer-1",
            "idTo": "group-1",
            "dName": "Peer",
            "ts": "1785456000000",
            "status": 1,
            "content": "hello",
            "notify": "",
            "ttl": 0,
            "userId": "",
            "uin": "",
            "propertyExt": null,
            "cmd": 501,
            "st": 0,
            "at": 0,
            "realMsgId": "real-1",
            "quote": null
        }))
        .unwrap();

        assert_eq!(message.params_ext.count_unread, 0);
        assert_eq!(message.params_ext.contain_type, 0);
        assert_eq!(message.params_ext.platform_type, 0);
    }

    #[test]
    fn realtime_message_defaults_partial_params_ext() {
        let message: TMessage = serde_json::from_value(serde_json::json!({
            "actionId": "action-2",
            "msgId": "message-2",
            "cliMsgId": "client-2",
            "msgType": "webchat",
            "uidFrom": "peer-2",
            "idTo": "self-1",
            "dName": "Peer",
            "ts": "1785548753795",
            "status": 1,
            "content": "hello",
            "notify": "",
            "ttl": 0,
            "userId": "",
            "uin": "",
            "propertyExt": null,
            "paramsExt": {
                "countUnread": 0,
                "containType": 0
            },
            "cmd": 501,
            "st": 0,
            "at": 0,
            "realMsgId": "real-2",
            "quote": null
        }))
        .unwrap();

        assert_eq!(message.params_ext.count_unread, 0);
        assert_eq!(message.params_ext.contain_type, 0);
        assert_eq!(message.params_ext.platform_type, 0);
    }

    #[test]
    fn user_and_group_message_detect_is_self_with_mobile_uid() {
        let own_uid = "351966042521";
        let peer_uid = "84903333333";
        let group_id = "g_123456789";

        // 1. UserMessage sent from web (uidFrom == "0")
        let mut msg_web = create_dummy_message();
        msg_web.uid_from = "0".to_string();
        msg_web.id_to = peer_uid.to_string();
        let user_msg_web = UserMessage::new(own_uid, msg_web);
        assert!(user_msg_web.is_self);
        assert_eq!(user_msg_web.thread_id, peer_uid);
        assert_eq!(user_msg_web.data.uid_from, own_uid);

        // 2. UserMessage sent from mobile (uidFrom == own_uid)
        let mut msg_mobile = create_dummy_message();
        msg_mobile.uid_from = own_uid.to_string();
        msg_mobile.id_to = peer_uid.to_string();
        let user_msg_mobile = UserMessage::new(own_uid, msg_mobile);
        assert!(user_msg_mobile.is_self);
        assert_eq!(user_msg_mobile.thread_id, peer_uid);

        // 3. UserMessage received from peer (uidFrom == peer_uid)
        let mut msg_peer = create_dummy_message();
        msg_peer.uid_from = peer_uid.to_string();
        msg_peer.id_to = own_uid.to_string();
        let user_msg_peer = UserMessage::new(own_uid, msg_peer);
        assert!(!user_msg_peer.is_self);
        assert_eq!(user_msg_peer.thread_id, peer_uid);

        // 4. GroupMessage sent from mobile (uidFrom == own_uid)
        let mut grp_mobile = TGroupMessage {
            base: create_dummy_message(),
            mentions: None,
        };
        grp_mobile.base.uid_from = own_uid.to_string();
        grp_mobile.base.id_to = group_id.to_string();
        let group_msg_mobile = GroupMessage::new(own_uid, grp_mobile);
        assert!(group_msg_mobile.is_self);
        assert_eq!(group_msg_mobile.thread_id, group_id);

        // 5. GroupMessage sent from peer (uidFrom == peer_uid)
        let mut grp_peer = TGroupMessage {
            base: create_dummy_message(),
            mentions: None,
        };
        grp_peer.base.uid_from = peer_uid.to_string();
        grp_peer.base.id_to = group_id.to_string();
        let group_msg_peer = GroupMessage::new(own_uid, grp_peer);
        assert!(!group_msg_peer.is_self);
        assert_eq!(group_msg_peer.thread_id, group_id);
    }

    fn create_dummy_message() -> TMessage {
        TMessage {
            action_id: "act-1".to_string(),
            msg_id: "msg-1".to_string(),
            cli_msg_id: "cli-1".to_string(),
            msg_type: "chat".to_string(),
            uid_from: "".to_string(),
            id_to: "".to_string(),
            d_name: "User".to_string(),
            ts: "1785548753795".to_string(),
            status: 1,
            content: "hello".to_string(),
            notify: "".to_string(),
            ttl: 0,
            user_id: "".to_string(),
            uin: "".to_string(),
            property_ext: None,
            params_ext: TParamsExt {
                count_unread: 0,
                contain_type: 0,
                platform_type: 0,
            },
            cmd: 501,
            st: 0,
            at: 0,
            real_msg_id: "real-1".to_string(),
            quote: None,
        }
    }
}
