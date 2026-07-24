use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSetting {
    pub block_name: i32,
    pub sign_admin_msg: i32,
    pub add_member_only: i32,
    pub set_topic_only: i32,
    pub enable_msg_history: i32,
    pub join_appr: i32,
    pub lock_create_post: i32,
    pub lock_create_poll: i32,
    pub lock_send_msg: i32,
    pub lock_view_member: i32,
    pub bann_feature: i32,
    pub dirty_media: i32,
    pub ban_duration: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupTopicType {
    Note = 0,
    Message = 2,
    Poll = 3,
}

impl Serialize for GroupTopicType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for GroupTopicType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            0 => Ok(GroupTopicType::Note),
            2 => Ok(GroupTopicType::Message),
            3 => Ok(GroupTopicType::Poll),
            _ => Ok(GroupTopicType::Note),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupTopic {
    pub r#type: GroupTopicType,
    pub color: i32,
    pub emoji: String,
    pub start_time: u64,
    pub duration: u64,
    pub params: serde_json::Value,
    pub id: String,
    pub creator_id: String,
    pub create_time: u64,
    pub editor_id: String,
    pub edit_time: u64,
    pub repeat: i32,
    pub action: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupType {
    Group = 1,
    Community = 2,
}

impl Serialize for GroupType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for GroupType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            1 => Ok(GroupType::Group),
            2 => Ok(GroupType::Community),
            _ => Ok(GroupType::Group),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupCurrentMem {
    pub id: String,
    pub d_name: String,
    pub zalo_name: String,
    pub avatar: String,
    #[serde(rename = "avatar_25")]
    pub avatar_25: String,
    pub account_status: i32,
    pub r#type: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfo {
    pub group_id: String,
    pub name: String,
    pub desc: String,
    pub r#type: GroupType,
    pub creator_id: String,
    pub version: String,
    pub avt: String,
    pub full_avt: String,
    pub member_ids: Vec<String>,
    pub admin_ids: Vec<String>,
    pub current_mems: Vec<GroupCurrentMem>,
    pub update_mems: Vec<serde_json::Value>,
    pub admins: Vec<serde_json::Value>,
    pub has_more_member: i32,
    pub sub_type: i32,
    pub total_member: u32,
    pub max_member: u32,
    pub setting: GroupSetting,
    pub created_time: u64,
    pub visibility: i32,
    pub global_id: String,
    pub e2ee: i32,
    pub extra_info: GroupExtraInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupExtraInfo {
    pub enable_media_store: i32,
}
