use serde::{Deserialize, Deserializer, Serialize, Serializer};

fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(value) => Ok(value),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        _ => Ok(String::new()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FriendEventType {
    Add = 0,
    Remove = 1,
    Request = 2,
    UndoRequest = 3,
    RejectRequest = 4,
    SeenFriendRequest = 5,
    Block = 6,
    Unblock = 7,
    BlockCall = 8,
    UnblockCall = 9,
    PinUnpin = 10,
    PinCreate = 11,
    Unknown = 12,
}

impl Serialize for FriendEventType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for FriendEventType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            0 => Ok(FriendEventType::Add),
            1 => Ok(FriendEventType::Remove),
            2 => Ok(FriendEventType::Request),
            3 => Ok(FriendEventType::UndoRequest),
            4 => Ok(FriendEventType::RejectRequest),
            5 => Ok(FriendEventType::SeenFriendRequest),
            6 => Ok(FriendEventType::Block),
            7 => Ok(FriendEventType::Unblock),
            8 => Ok(FriendEventType::BlockCall),
            9 => Ok(FriendEventType::UnblockCall),
            10 => Ok(FriendEventType::PinUnpin),
            11 => Ok(FriendEventType::PinCreate),
            _ => Ok(FriendEventType::Unknown),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TFriendEventRejectUndo {
    pub to_uid: String,
    pub from_uid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TFriendEventRequest {
    pub to_uid: String,
    pub from_uid: String,
    pub src: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TFriendEventPinTopic {
    pub topic_id: String,
    pub topic_type: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TFriendEventPinCreateTopicParams {
    pub sender_uid: String,
    pub sender_name: String,
    #[serde(rename = "client_msg_id")]
    pub client_msg_id: String,
    #[serde(rename = "global_msg_id")]
    pub global_msg_id: String,
    #[serde(rename = "msg_type")]
    pub msg_type: i32,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TFriendEventPinCreateTopic {
    pub r#type: i32,
    pub color: i32,
    pub emoji: String,
    pub start_time: u64,
    pub duration: u64,
    pub params: TFriendEventPinCreateTopicParams,
    pub id: String,
    pub creator_id: String,
    pub create_time: u64,
    pub editor_id: String,
    pub edit_time: u64,
    pub repeat: i32,
    pub action: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TFriendEventPinCreate {
    #[serde(default)]
    pub old_topic: Option<TFriendEventPinTopic>,
    pub topic: TFriendEventPinCreateTopic,
    pub actor_id: String,
    pub old_version: i64,
    pub version: i64,
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TFriendEventPinUnpin {
    pub topic: TFriendEventPinTopic,
    pub actor_id: String,
    pub old_version: i64,
    pub version: i64,
    pub conversation_id: String,
}

/// Typed friend-event payload. Known shapes are parsed into a concrete
/// variant; anything unexpected falls back to [`FriendEventData::Raw`] so no
/// data is ever lost on schema drift.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FriendEventData {
    Request(TFriendEventRequest),
    RejectUndo(TFriendEventRejectUndo),
    PinCreate(Box<TFriendEventPinCreate>),
    PinUnpin(Box<TFriendEventPinUnpin>),
    SeenRequest(Vec<String>),
    /// Plain thread id (add/remove/block/unblock events carry only a uid).
    Base(String),
    Raw(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendEvent {
    pub r#type: FriendEventType,
    pub data: FriendEventData,
    pub thread_id: String,
    pub is_self: bool,
}

pub fn initialize_friend_event(
    uid: &str,
    data: serde_json::Value,
    r#type: FriendEventType,
) -> FriendEvent {
    match r#type {
        FriendEventType::Add
        | FriendEventType::Remove
        | FriendEventType::Block
        | FriendEventType::Unblock
        | FriendEventType::BlockCall
        | FriendEventType::UnblockCall => {
            let thread_id = data.as_str().unwrap_or("").to_owned();
            let is_self = !(r#type == FriendEventType::Add || r#type == FriendEventType::Remove);
            FriendEvent {
                r#type,
                data: FriendEventData::Base(thread_id.clone()),
                thread_id,
                is_self,
            }
        }
        FriendEventType::RejectRequest | FriendEventType::UndoRequest => {
            let thread_id = data
                .get("toUid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let from_uid = data.get("fromUid").and_then(|v| v.as_str()).unwrap_or("");
            let is_self = from_uid == uid;
            let typed = match serde_json::from_value::<TFriendEventRejectUndo>(data.clone()) {
                Ok(v) => FriendEventData::RejectUndo(v),
                Err(_) => FriendEventData::Raw(data),
            };
            FriendEvent {
                r#type,
                data: typed,
                thread_id,
                is_self,
            }
        }
        FriendEventType::Request => {
            let thread_id = data
                .get("toUid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let from_uid = data.get("fromUid").and_then(|v| v.as_str()).unwrap_or("");
            let is_self = from_uid == uid;
            let typed = match serde_json::from_value::<TFriendEventRequest>(data.clone()) {
                Ok(v) => FriendEventData::Request(v),
                Err(_) => FriendEventData::Raw(data),
            };
            FriendEvent {
                r#type,
                data: typed,
                thread_id,
                is_self,
            }
        }
        FriendEventType::SeenFriendRequest => {
            let typed = match serde_json::from_value::<Vec<String>>(data.clone()) {
                Ok(v) => FriendEventData::SeenRequest(v),
                Err(_) => FriendEventData::Raw(data),
            };
            FriendEvent {
                r#type,
                data: typed,
                thread_id: uid.to_owned(),
                is_self: true,
            }
        }
        FriendEventType::PinCreate => {
            let thread_id = data
                .get("conversationId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let actor_id = data.get("actorId").and_then(|v| v.as_str()).unwrap_or("");
            let is_self = actor_id == uid;
            let typed = match serde_json::from_value::<TFriendEventPinCreate>(data.clone()) {
                Ok(v) => FriendEventData::PinCreate(Box::new(v)),
                Err(_) => FriendEventData::Raw(data),
            };
            FriendEvent {
                r#type,
                data: typed,
                thread_id,
                is_self,
            }
        }
        FriendEventType::PinUnpin => {
            let thread_id = data
                .get("conversationId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            let actor_id = data.get("actorId").and_then(|v| v.as_str()).unwrap_or("");
            let is_self = actor_id == uid;
            let typed = match serde_json::from_value::<TFriendEventPinUnpin>(data.clone()) {
                Ok(v) => FriendEventData::PinUnpin(Box::new(v)),
                Err(_) => FriendEventData::Raw(data),
            };
            FriendEvent {
                r#type,
                data: typed,
                thread_id,
                is_self,
            }
        }
        _ => FriendEvent {
            r#type: FriendEventType::Unknown,
            data: FriendEventData::Raw(data),
            thread_id: String::new(),
            is_self: false,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupEventType {
    JoinRequest,
    Join,
    Leave,
    RemoveMember,
    BlockMember,
    UpdateSetting,
    Update,
    NewLink,
    AddAdmin,
    RemoveAdmin,
    NewPinTopic,
    UpdatePinTopic,
    ReorderPinTopic,
    UpdateBoard,
    RemoveBoard,
    UpdateTopic,
    UnpinTopic,
    RemoveTopic,
    AcceptRemind,
    RejectRemind,
    RemindTopic,
    UpdateAvatar,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupEventUpdateMember {
    pub id: String,
    #[serde(rename = "dName")]
    pub d_name: String,
    pub avatar: String,
    pub r#type: i32,
    #[serde(rename = "avatar_25")]
    pub avatar_25: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TGroupEventJoinRequest {
    pub uids: Vec<String>,
    pub total_pending: i64,
    pub group_id: String,
    pub time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TGroupEventPinTopic {
    pub old_board_version: i64,
    pub board_version: i64,
    pub topic: crate::models::GroupTopic,
    pub actor_id: String,
    pub group_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TGroupEventReorderTopicItem {
    pub topic_id: String,
    pub topic_type: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TGroupEventReorderPinTopic {
    pub old_board_version: i64,
    pub actor_id: String,
    pub topics: Vec<TGroupEventReorderTopicItem>,
    pub group_id: String,
    pub board_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TGroupEventRemindRespond {
    pub topic_id: String,
    pub update_members: Vec<String>,
    pub group_id: String,
    pub time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TGroupEventRemindTopic {
    pub msg: String,
    pub editor_id: String,
    pub color: String,
    pub emoji: String,
    pub creator_id: String,
    pub edit_time: u64,
    pub r#type: i32,
    pub duration: u64,
    #[serde(rename = "group_id")]
    pub group_id: String,
    pub create_time: u64,
    pub repeat: i32,
    pub start_time: u64,
    pub time: u64,
    pub remind_type: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TGroupEventBase {
    pub sub_type: i32,
    pub group_id: String,
    pub creator_id: String,
    pub group_name: String,
    pub source_id: String,
    #[serde(default)]
    pub update_members: Vec<GroupEventUpdateMember>,
    #[serde(default)]
    pub group_setting: Option<crate::models::GroupSetting>,
    #[serde(default)]
    pub group_topic: Option<crate::models::GroupTopic>,
    #[serde(default)]
    pub info: Option<serde_json::Value>,
    #[serde(default)]
    pub extra_data: Option<serde_json::Value>,
    pub time: String,
    #[serde(default)]
    pub avt: Option<String>,
    #[serde(default)]
    pub full_avt: Option<String>,
    pub is_add: i32,
    pub hide_group_info: i32,
    pub version: String,
    pub group_type: i32,
}

/// Typed group-event payload. Known shapes are parsed into a concrete
/// variant; anything unexpected falls back to [`GroupEventData::Raw`] so no
/// data is ever lost on schema drift.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GroupEventData {
    JoinRequest(TGroupEventJoinRequest),
    PinTopic(Box<TGroupEventPinTopic>),
    ReorderPinTopic(TGroupEventReorderPinTopic),
    RemindRespond(TGroupEventRemindRespond),
    RemindTopic(Box<TGroupEventRemindTopic>),
    Base(Box<TGroupEventBase>),
    Raw(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupEvent {
    pub r#type: GroupEventType,
    pub act: String,
    pub data: GroupEventData,
    pub thread_id: String,
    pub is_self: bool,
}

pub fn initialize_group_event(
    uid: &str,
    data: serde_json::Value,
    r#type: GroupEventType,
    act: String,
) -> GroupEvent {
    let thread_id = if let Some(gid) = data.get("group_id").and_then(|v| v.as_str()) {
        gid.to_owned()
    } else {
        data.get("groupId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned()
    };

    let is_self = match r#type {
        GroupEventType::JoinRequest => false,
        GroupEventType::NewPinTopic
        | GroupEventType::UnpinTopic
        | GroupEventType::UpdatePinTopic
        | GroupEventType::ReorderPinTopic => {
            let actor_id = data.get("actorId").and_then(|v| v.as_str()).unwrap_or("");
            actor_id == uid
        }
        GroupEventType::UpdateBoard | GroupEventType::RemoveBoard => {
            let source_id = data.get("sourceId").and_then(|v| v.as_str()).unwrap_or("");
            source_id == uid
        }
        GroupEventType::AcceptRemind | GroupEventType::RejectRemind => {
            let mut matched = false;
            if let Some(arr) = data.get("updateMembers").and_then(|v| v.as_array()) {
                for v in arr {
                    if v.as_str() == Some(uid) {
                        matched = true;
                        break;
                    }
                }
            }
            matched
        }
        GroupEventType::RemindTopic => {
            let creator_id = data.get("creatorId").and_then(|v| v.as_str()).unwrap_or("");
            creator_id == uid
        }
        _ => {
            let mut matched = false;
            if let Some(arr) = data.get("updateMembers").and_then(|v| v.as_array()) {
                for member in arr {
                    if member.get("id").and_then(|v| v.as_str()) == Some(uid) {
                        matched = true;
                        break;
                    }
                }
            }
            let source_id = data.get("sourceId").and_then(|v| v.as_str()).unwrap_or("");
            matched || source_id == uid
        }
    };

    let typed = parse_group_event_data(&data, r#type);

    GroupEvent {
        r#type,
        act,
        data: typed,
        thread_id,
        is_self,
    }
}

fn parse_group_event_data(data: &serde_json::Value, r#type: GroupEventType) -> GroupEventData {
    match r#type {
        GroupEventType::JoinRequest => {
            match serde_json::from_value::<TGroupEventJoinRequest>(data.clone()) {
                Ok(v) => GroupEventData::JoinRequest(v),
                Err(_) => GroupEventData::Raw(data.clone()),
            }
        }
        GroupEventType::NewPinTopic
        | GroupEventType::UnpinTopic
        | GroupEventType::UpdatePinTopic => {
            match serde_json::from_value::<TGroupEventPinTopic>(data.clone()) {
                Ok(v) => GroupEventData::PinTopic(Box::new(v)),
                Err(_) => GroupEventData::Raw(data.clone()),
            }
        }
        GroupEventType::ReorderPinTopic => {
            match serde_json::from_value::<TGroupEventReorderPinTopic>(data.clone()) {
                Ok(v) => GroupEventData::ReorderPinTopic(v),
                Err(_) => GroupEventData::Raw(data.clone()),
            }
        }
        GroupEventType::AcceptRemind | GroupEventType::RejectRemind => {
            match serde_json::from_value::<TGroupEventRemindRespond>(data.clone()) {
                Ok(v) => GroupEventData::RemindRespond(v),
                Err(_) => GroupEventData::Raw(data.clone()),
            }
        }
        GroupEventType::RemindTopic => {
            match serde_json::from_value::<TGroupEventRemindTopic>(data.clone()) {
                Ok(v) => GroupEventData::RemindTopic(Box::new(v)),
                Err(_) => GroupEventData::Raw(data.clone()),
            }
        }
        // UpdateBoard / RemoveBoard and everything else fall back to the base
        // shape; if that does not fit, keep the raw payload.
        _ => match serde_json::from_value::<TGroupEventBase>(data.clone()) {
            Ok(v) => GroupEventData::Base(Box::new(v)),
            Err(_) => GroupEventData::Raw(data.clone()),
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TDeliveredMessage {
    pub msg_id: String,
    pub seen: i32,
    pub delivered_uids: Vec<String>,
    pub seen_uids: Vec<String>,
    pub real_msg_id: String,
    pub m_s_ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TGroupDeliveredMessage {
    #[serde(flatten)]
    pub base: TDeliveredMessage,
    pub group_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDeliveredMessage {
    pub r#type: crate::models::ThreadType,
    pub data: TDeliveredMessage,
    pub thread_id: String,
    pub is_self: bool,
}

impl UserDeliveredMessage {
    pub fn new(data: TDeliveredMessage) -> Self {
        let thread_id = data.delivered_uids.first().cloned().unwrap_or_default();
        Self {
            r#type: crate::models::ThreadType::User,
            data,
            thread_id,
            is_self: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupDeliveredMessage {
    pub r#type: crate::models::ThreadType,
    pub data: TGroupDeliveredMessage,
    pub thread_id: String,
    pub is_self: bool,
}

impl GroupDeliveredMessage {
    pub fn new(uid: &str, data: TGroupDeliveredMessage) -> Self {
        let thread_id = data.group_id.clone();
        let is_self = data.base.delivered_uids.iter().any(|u| u == uid);
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
pub enum DeliveredMessage {
    #[serde(rename = "0")]
    User(UserDeliveredMessage),
    #[serde(rename = "1")]
    Group(GroupDeliveredMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TUserSeenMessage {
    pub id_to: String,
    pub msg_id: String,
    pub real_msg_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TGroupSeenMessage {
    pub msg_id: String,
    pub group_id: String,
    pub seen_uids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSeenMessage {
    pub r#type: crate::models::ThreadType,
    pub data: TUserSeenMessage,
    pub thread_id: String,
    pub is_self: bool,
}

impl UserSeenMessage {
    pub fn new(data: TUserSeenMessage) -> Self {
        let thread_id = data.id_to.clone();
        Self {
            r#type: crate::models::ThreadType::User,
            data,
            thread_id,
            is_self: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSeenMessage {
    pub r#type: crate::models::ThreadType,
    pub data: TGroupSeenMessage,
    pub thread_id: String,
    pub is_self: bool,
}

impl GroupSeenMessage {
    pub fn new(uid: &str, data: TGroupSeenMessage) -> Self {
        let thread_id = data.group_id.clone();
        let is_self = data.seen_uids.iter().any(|u| u == uid);
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
pub enum SeenMessage {
    #[serde(rename = "0")]
    User(UserSeenMessage),
    #[serde(rename = "1")]
    Group(GroupSeenMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TTyping {
    pub uid: String,
    pub ts: String,
    #[serde(rename = "isPC")]
    pub is_pc: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TGroupTyping {
    #[serde(flatten)]
    pub base: TTyping,
    pub gid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTyping {
    pub r#type: crate::models::ThreadType,
    pub data: TTyping,
    pub thread_id: String,
    pub is_self: bool,
}

impl UserTyping {
    pub fn new(data: TTyping) -> Self {
        let thread_id = data.uid.clone();
        Self {
            r#type: crate::models::ThreadType::User,
            data,
            thread_id,
            is_self: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupTyping {
    pub r#type: crate::models::ThreadType,
    pub data: TGroupTyping,
    pub thread_id: String,
    pub is_self: bool,
}

impl GroupTyping {
    pub fn new(data: TGroupTyping) -> Self {
        let thread_id = data.gid.clone();
        Self {
            r#type: crate::models::ThreadType::Group,
            data,
            thread_id,
            is_self: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Typing {
    #[serde(rename = "0")]
    User(UserTyping),
    #[serde(rename = "1")]
    Group(GroupTyping),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TUndoContent {
    pub global_msg_id: u64,
    pub cli_msg_id: u64,
    pub delete_msg: i32,
    pub src_id: u64,
    pub dest_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TUndo {
    pub action_id: String,
    pub msg_id: String,
    pub cli_msg_id: String,
    pub msg_type: String,
    pub uid_from: String,
    pub id_to: String,
    pub d_name: String,
    pub ts: String,
    pub status: i32,
    pub content: TUndoContent,
    pub notify: String,
    pub ttl: i32,
    pub user_id: String,
    pub uin: String,
    pub cmd: i32,
    pub st: i32,
    pub at: i32,
    pub real_msg_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Undo {
    pub data: TUndo,
    pub thread_id: String,
    pub is_self: bool,
    pub is_group: bool,
}

impl Undo {
    pub fn new(uid: &str, mut data: TUndo, is_group: bool) -> Self {
        let is_self = data.uid_from == "0" || (!uid.is_empty() && data.uid_from == uid);
        let thread_id = if is_group || is_self {
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
            data,
            thread_id,
            is_self,
            is_group,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum EventReactions {
    #[serde(rename = "/-heart")]
    Heart,
    #[serde(rename = "/-strong")]
    Like,
    #[serde(rename = ":>")]
    Haha,
    #[serde(rename = ":o")]
    Wow,
    #[serde(rename = ":-((")]
    Cry,
    #[serde(rename = ":-h")]
    Angry,
    #[serde(rename = ":-*")]
    Kiss,
    #[serde(rename = ":')")]
    TearsOfJoy,
    #[serde(rename = "/-shit")]
    Shit,
    #[serde(rename = "/-rose")]
    Rose,
    #[serde(rename = "/-break")]
    BrokenHeart,
    #[serde(rename = "/-weak")]
    Dislike,
    #[serde(rename = ";xx")]
    Love,
    #[serde(rename = ";-/")]
    Confused,
    #[serde(rename = ";-)")]
    Wink,
    #[serde(rename = "/-fade")]
    Fade,
    #[serde(rename = "/-li")]
    Sun,
    #[serde(rename = "/-bd")]
    Birthday,
    #[serde(rename = "/-bome")]
    Bomb,
    #[serde(rename = "/-ok")]
    Ok,
    #[serde(rename = "/-v")]
    Peace,
    #[serde(rename = "/-thanks")]
    Thanks,
    #[serde(rename = "/-punch")]
    Punch,
    #[serde(rename = "/-share")]
    Share,
    #[serde(rename = "_()_")]
    Pray,
    #[serde(rename = "/-no")]
    No,
    #[serde(rename = "/-bad")]
    Bad,
    #[serde(rename = "/-loveu")]
    LoveYou,
    #[serde(rename = "--b")]
    Sad,
    #[serde(rename = ":((")]
    VerySad,
    #[serde(rename = "x-)")]
    Cool,
    #[serde(rename = "8-)")]
    Nerd,
    #[serde(rename = ";-d")]
    BigSmile,
    #[serde(rename = "b-)")]
    Sunglasses,
    #[serde(rename = ":--|")]
    Neutral,
    #[serde(rename = "p-(")]
    SadFace,
    #[serde(rename = ":-bye")]
    Bye,
    #[serde(rename = "|-)")]
    Sleepy,
    #[serde(rename = ":wipe")]
    Wipe,
    #[serde(rename = ":-dig")]
    Dig,
    #[serde(rename = "&-(")]
    Anguish,
    #[serde(rename = ":handclap")]
    Handclap,
    #[serde(rename = ">-|")]
    AngryFace,
    #[serde(rename = ":-f")]
    FChair,
    #[serde(rename = ":-l")]
    LChair,
    #[serde(rename = ":-r")]
    RChair,
    #[serde(rename = ";-x")]
    Silent,
    #[serde(rename = ":-o")]
    Surprise,
    #[serde(rename = ";-s")]
    Embarrassed,
    #[serde(rename = ";-a")]
    Afraid,
    #[serde(rename = ":-<")]
    Sad2,
    #[serde(rename = ":))")]
    BigLaugh,
    #[serde(rename = "$-)")]
    Rich,
    #[serde(rename = "/-beer")]
    Beer,
    Unknown(String),
}

impl EventReactions {
    pub fn as_provider_str(&self) -> &str {
        match self {
            Self::Heart => "/-heart",
            Self::Like => "/-strong",
            Self::Haha => ":>",
            Self::Wow => ":o",
            Self::Cry => ":-((",
            Self::Angry => ":-h",
            Self::Kiss => ":-*",
            Self::TearsOfJoy => ":')",
            Self::Shit => "/-shit",
            Self::Rose => "/-rose",
            Self::BrokenHeart => "/-break",
            Self::Dislike => "/-weak",
            Self::Love => ";xx",
            Self::Confused => ";-/",
            Self::Wink => ";-)",
            Self::Fade => "/-fade",
            Self::Sun => "/-li",
            Self::Birthday => "/-bd",
            Self::Bomb => "/-bome",
            Self::Ok => "/-ok",
            Self::Peace => "/-v",
            Self::Thanks => "/-thanks",
            Self::Punch => "/-punch",
            Self::Share => "/-share",
            Self::Pray => "_()_",
            Self::No => "/-no",
            Self::Bad => "/-bad",
            Self::LoveYou => "/-loveu",
            Self::Sad => "--b",
            Self::VerySad => ":((",
            Self::Cool => "x-)",
            Self::Nerd => "8-)",
            Self::BigSmile => ";-d",
            Self::Sunglasses => "b-)",
            Self::Neutral => ":--|",
            Self::SadFace => "p-(",
            Self::Bye => ":-bye",
            Self::Sleepy => "|-)",
            Self::Wipe => ":wipe",
            Self::Dig => ":-dig",
            Self::Anguish => "&-(",
            Self::Handclap => ":handclap",
            Self::AngryFace => ">-|",
            Self::FChair => ":-f",
            Self::LChair => ":-l",
            Self::RChair => ":-r",
            Self::Silent => ";-x",
            Self::Surprise => ":-o",
            Self::Embarrassed => ";-s",
            Self::Afraid => ";-a",
            Self::Sad2 => ":-<",
            Self::BigLaugh => ":))",
            Self::Rich => "$-)",
            Self::Beer => "/-beer",
            Self::Unknown(raw_value) => raw_value,
        }
    }
}

impl From<String> for EventReactions {
    fn from(value: String) -> Self {
        match value.as_str() {
            "/-heart" => Self::Heart,
            "/-strong" => Self::Like,
            ":>" => Self::Haha,
            ":o" => Self::Wow,
            ":-((" => Self::Cry,
            ":-h" => Self::Angry,
            ":-*" => Self::Kiss,
            ":')" => Self::TearsOfJoy,
            "/-shit" => Self::Shit,
            "/-rose" => Self::Rose,
            "/-break" => Self::BrokenHeart,
            "/-weak" => Self::Dislike,
            ";xx" => Self::Love,
            ";-/" => Self::Confused,
            ";-)" => Self::Wink,
            "/-fade" => Self::Fade,
            "/-li" => Self::Sun,
            "/-bd" => Self::Birthday,
            "/-bome" => Self::Bomb,
            "/-ok" => Self::Ok,
            "/-v" => Self::Peace,
            "/-thanks" => Self::Thanks,
            "/-punch" => Self::Punch,
            "/-share" => Self::Share,
            "_()_" => Self::Pray,
            "/-no" => Self::No,
            "/-bad" => Self::Bad,
            "/-loveu" => Self::LoveYou,
            "--b" => Self::Sad,
            ":((" => Self::VerySad,
            "x-)" => Self::Cool,
            "8-)" => Self::Nerd,
            ";-d" => Self::BigSmile,
            "b-)" => Self::Sunglasses,
            ":--|" => Self::Neutral,
            "p-(" => Self::SadFace,
            ":-bye" => Self::Bye,
            "|-)" => Self::Sleepy,
            ":wipe" => Self::Wipe,
            ":-dig" => Self::Dig,
            "&-(" => Self::Anguish,
            ":handclap" => Self::Handclap,
            ">-|" => Self::AngryFace,
            ":-f" => Self::FChair,
            ":-l" => Self::LChair,
            ":-r" => Self::RChair,
            ";-x" => Self::Silent,
            ":-o" => Self::Surprise,
            ";-s" => Self::Embarrassed,
            ";-a" => Self::Afraid,
            ":-<" => Self::Sad2,
            ":))" => Self::BigLaugh,
            "$-)" => Self::Rich,
            "/-beer" => Self::Beer,
            _ => Self::Unknown(value),
        }
    }
}

impl From<EventReactions> for String {
    fn from(value: EventReactions) -> Self {
        value.as_provider_str().to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TReactionMsg {
    #[serde(rename = "gMsgID")]
    #[serde(deserialize_with = "deserialize_string_or_number")]
    pub g_msg_id: String,
    #[serde(rename = "cMsgID")]
    #[serde(deserialize_with = "deserialize_string_or_number")]
    pub c_msg_id: String,
    pub msg_type: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TReactionContent {
    pub r_msg: Vec<TReactionMsg>,
    pub r_icon: EventReactions,
    pub r_type: i32,
    pub source: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TReaction {
    pub action_id: String,
    pub msg_id: String,
    pub cli_msg_id: String,
    pub msg_type: String,
    pub uid_from: String,
    pub id_to: String,
    pub d_name: Option<String>,
    pub content: TReactionContent,
    #[serde(deserialize_with = "deserialize_string_or_number")]
    pub ts: String,
    pub ttl: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub data: TReaction,
    pub thread_id: String,
    pub is_self: bool,
    pub is_group: bool,
}

impl Reaction {
    pub fn new(uid: &str, mut data: TReaction, is_group: bool) -> Self {
        let is_self = data.uid_from == "0" || data.uid_from == uid;
        let thread_id = if is_group || is_self {
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
            data,
            thread_id,
            is_self,
            is_group,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReminderRepeatMode {
    None = 0,
    Daily = 1,
    Weekly = 2,
    Monthly = 3,
}

impl Serialize for ReminderRepeatMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for ReminderRepeatMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            0 => Ok(ReminderRepeatMode::None),
            1 => Ok(ReminderRepeatMode::Daily),
            2 => Ok(ReminderRepeatMode::Weekly),
            3 => Ok(ReminderRepeatMode::Monthly),
            _ => Ok(ReminderRepeatMode::None),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderUser {
    pub creator_uid: String,
    pub to_uid: String,
    pub emoji: String,
    pub color: i32,
    pub reminder_id: String,
    pub create_time: u64,
    pub repeat: ReminderRepeatMode,
    pub start_time: u64,
    pub edit_time: u64,
    pub end_time: u64,
    pub params: ReminderUserParams,
    pub r#type: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderUserParams {
    pub title: String,
    pub set_title: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderGroupResponseMem {
    pub reject_member: i32,
    pub my_resp: i32,
    pub accept_member: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderGroup {
    pub editor_id: String,
    pub emoji: String,
    pub color: i32,
    pub group_id: String,
    pub creator_id: String,
    pub edit_time: u64,
    pub event_type: i32,
    pub response_mem: Option<ReminderGroupResponseMem>,
    pub params: ReminderGroupParams,
    pub r#type: i32,
    pub duration: u64,
    pub repeat_info: Option<serde_json::Value>,
    pub repeat_data: Vec<serde_json::Value>,
    pub create_time: u64,
    pub repeat: ReminderRepeatMode,
    pub start_time: u64,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderGroupParams {
    pub title: String,
    pub set_title: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Reminder {
    User(ReminderUser),
    Group(ReminderGroup),
}

pub fn get_friend_event_type(act: &str) -> FriendEventType {
    match act {
        "add" => FriendEventType::Add,
        "remove" => FriendEventType::Remove,
        "block" => FriendEventType::Block,
        "unblock" => FriendEventType::Unblock,
        "block_call" => FriendEventType::BlockCall,
        "unblock_call" => FriendEventType::UnblockCall,
        "req_v2" => FriendEventType::Request,
        "reject" => FriendEventType::RejectRequest,
        "undo_req" => FriendEventType::UndoRequest,
        "seen_fr_req" => FriendEventType::SeenFriendRequest,
        "pin_unpin" => FriendEventType::PinUnpin,
        "pin_create" => FriendEventType::PinCreate,
        _ => FriendEventType::Unknown,
    }
}

pub fn get_group_event_type(act: &str) -> GroupEventType {
    match act {
        "join_request" => GroupEventType::JoinRequest,
        "join" => GroupEventType::Join,
        "leave" => GroupEventType::Leave,
        "remove_member" => GroupEventType::RemoveMember,
        "block_member" => GroupEventType::BlockMember,
        "update_setting" => GroupEventType::UpdateSetting,
        "update_avatar" => GroupEventType::UpdateAvatar,
        "update" => GroupEventType::Update,
        "new_link" => GroupEventType::NewLink,
        "add_admin" => GroupEventType::AddAdmin,
        "remove_admin" => GroupEventType::RemoveAdmin,
        "new_pin_topic" => GroupEventType::NewPinTopic,
        "update_pin_topic" => GroupEventType::UpdatePinTopic,
        "update_topic" => GroupEventType::UpdateTopic,
        "update_board" => GroupEventType::UpdateBoard,
        "remove_board" => GroupEventType::RemoveBoard,
        "reorder_pin_topic" => GroupEventType::ReorderPinTopic,
        "unpin_topic" => GroupEventType::UnpinTopic,
        "remove_topic" => GroupEventType::RemoveTopic,
        "accept_remind" => GroupEventType::AcceptRemind,
        "reject_remind" => GroupEventType::RejectRemind,
        "remind_topic" => GroupEventType::RemindTopic,
        _ => GroupEventType::Unknown,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn reaction_history_accepts_numeric_timestamp() {
        let reaction: TReaction = serde_json::from_value(serde_json::json!({
            "actionId": "reaction-action",
            "msgId": "reaction-event",
            "cliMsgId": "reaction-client",
            "msgType": "1",
            "uidFrom": "peer-1",
            "idTo": "0",
            "dName": "Peer",
            "content": {
                "rMsg": [{"gMsgID": 1785483160459_i64, "cMsgID": 1785483160460_i64, "msgType": 1}],
                "rIcon": ":-h",
                "rType": 1,
                "source": 1
            },
            "ts": 1785483160459_i64,
            "ttl": 0
        }))
        .unwrap();

        assert_eq!(reaction.ts, "1785483160459");
        assert_eq!(reaction.content.r_msg[0].g_msg_id, "1785483160459");
        assert_eq!(reaction.content.r_msg[0].c_msg_id, "1785483160460");
    }

    #[test]
    fn unknown_reaction_icon_round_trips_raw_provider_value() {
        let raw_icon = "/:new-haha";
        let reaction: EventReactions = serde_json::from_value(serde_json::json!(raw_icon)).unwrap();

        assert_eq!(
            serde_json::to_value(&reaction).unwrap(),
            serde_json::json!(raw_icon)
        );
    }

    #[test]
    fn friend_request_event_parses_typed_payload() {
        let data = serde_json::json!({
            "toUid": "111",
            "fromUid": "222",
            "src": 1,
            "message": "hi",
        });
        let ev = initialize_friend_event("111", data, FriendEventType::Request);

        assert_eq!(ev.thread_id, "111");
        assert!(!ev.is_self); // fromUid (222) != uid (111)
        match ev.data {
            FriendEventData::Request(req) => {
                assert_eq!(req.from_uid, "222");
                assert_eq!(req.message, "hi");
            }
            other => panic!("expected typed Request, got {other:?}"),
        }
    }

    #[test]
    fn friend_base_event_keeps_thread_id_string() {
        let data = serde_json::json!("333");
        let ev = initialize_friend_event("me", data, FriendEventType::Add);

        assert_eq!(ev.thread_id, "333");
        assert!(!ev.is_self); // ADD is never self
        assert!(matches!(ev.data, FriendEventData::Base(ref s) if s == "333"));
    }

    #[test]
    fn group_join_request_event_parses_typed_payload() {
        let data = serde_json::json!({
            "uids": ["a", "b"],
            "totalPending": 2,
            "groupId": "g1",
            "time": "123",
        });
        let ev = initialize_group_event(
            "me",
            data,
            GroupEventType::JoinRequest,
            "join_request".into(),
        );

        assert_eq!(ev.thread_id, "g1");
        assert!(!ev.is_self);
        match ev.data {
            GroupEventData::JoinRequest(jr) => {
                assert_eq!(jr.uids, vec!["a", "b"]);
                assert_eq!(jr.total_pending, 2);
            }
            other => panic!("expected typed JoinRequest, got {other:?}"),
        }
    }

    #[test]
    fn group_unexpected_payload_falls_back_to_raw() {
        // Missing required base fields -> must not panic, must keep raw data.
        let data = serde_json::json!({ "weird": true, "groupId": "g9" });
        let ev = initialize_group_event("me", data, GroupEventType::Update, "update".into());

        assert_eq!(ev.thread_id, "g9");
        assert!(matches!(ev.data, GroupEventData::Raw(_)));
    }

    #[test]
    fn undo_detects_is_self_with_mobile_uid() {
        let own_uid = "351966042521";
        let peer_uid = "84903333333";
        let group_id = "g_123456789";

        let mut t_undo_self = create_dummy_undo();
        t_undo_self.id_to = peer_uid.to_string();
        t_undo_self.uid_from = own_uid.to_string();
        let undo_self = Undo::new(own_uid, t_undo_self, false);
        assert!(undo_self.is_self);
        assert_eq!(undo_self.thread_id, peer_uid);

        let mut t_undo_group = create_dummy_undo();
        t_undo_group.id_to = group_id.to_string();
        t_undo_group.uid_from = own_uid.to_string();
        let undo_grp = Undo::new(own_uid, t_undo_group, true);
        assert!(undo_grp.is_self);
        assert_eq!(undo_grp.thread_id, group_id);
    }

    fn create_dummy_undo() -> TUndo {
        TUndo {
            action_id: "act-1".to_string(),
            msg_id: "msg-1".to_string(),
            cli_msg_id: "cli-1".to_string(),
            msg_type: "undo".to_string(),
            uid_from: "".to_string(),
            id_to: "".to_string(),
            d_name: "User".to_string(),
            ts: "123".to_string(),
            status: 1,
            content: TUndoContent {
                global_msg_id: 1,
                cli_msg_id: 1,
                delete_msg: 1,
                src_id: 0,
                dest_id: 0,
            },
            notify: "".to_string(),
            ttl: 0,
            user_id: "".to_string(),
            uin: "".to_string(),
            cmd: 501,
            st: 0,
            at: 0,
            real_msg_id: "real-1".to_string(),
        }
    }
}
