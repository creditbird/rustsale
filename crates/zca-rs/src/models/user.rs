use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusinessCategory {
    Other = 0,
    RealEstate = 1,
    TechnologyAndDevices = 2,
    TravelAndHospitality = 3,
    EducationAndTraining = 4,
    ShoppingAndRetail = 5,
    CosmeticsAndBeauty = 6,
    RestaurantAndCafe = 7,
    AutoAndMotorbike = 8,
    FashionAndApparel = 9,
    FoodAndBeverage = 10,
    MediaAndEntertainment = 11,
    InternalCommunications = 12,
    Transportation = 13,
    Telecommunications = 14,
}

impl BusinessCategory {
    pub fn name(&self) -> &'static str {
        match self {
            BusinessCategory::Other => "Dịch vụ khác (Không hiển thị)",
            BusinessCategory::RealEstate => "Bất động sản",
            BusinessCategory::TechnologyAndDevices => "Công nghệ & Thiết bị",
            BusinessCategory::TravelAndHospitality => "Du lịch & Lưu trú",
            BusinessCategory::EducationAndTraining => "Giáo dục & Đào tạo",
            BusinessCategory::ShoppingAndRetail => "Mua sắm & Bán lẻ",
            BusinessCategory::CosmeticsAndBeauty => "Mỹ phẩm & Làm đẹp",
            BusinessCategory::RestaurantAndCafe => "Nhà hàng & Quán",
            BusinessCategory::AutoAndMotorbike => "Ô tô & Xe máy",
            BusinessCategory::FashionAndApparel => "Thời trang & May mặc",
            BusinessCategory::FoodAndBeverage => "Thực phẩm & Đồ uống",
            BusinessCategory::MediaAndEntertainment => "Truyền thông & Giải trí",
            BusinessCategory::InternalCommunications => "Truyền thông nội bộ",
            BusinessCategory::Transportation => "Vận tải",
            BusinessCategory::Telecommunications => "Viễn thông",
        }
    }
}

impl Serialize for BusinessCategory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for BusinessCategory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            0 => Ok(BusinessCategory::Other),
            1 => Ok(BusinessCategory::RealEstate),
            2 => Ok(BusinessCategory::TechnologyAndDevices),
            3 => Ok(BusinessCategory::TravelAndHospitality),
            4 => Ok(BusinessCategory::EducationAndTraining),
            5 => Ok(BusinessCategory::ShoppingAndRetail),
            6 => Ok(BusinessCategory::CosmeticsAndBeauty),
            7 => Ok(BusinessCategory::RestaurantAndCafe),
            8 => Ok(BusinessCategory::AutoAndMotorbike),
            9 => Ok(BusinessCategory::FashionAndApparel),
            10 => Ok(BusinessCategory::FoodAndBeverage),
            11 => Ok(BusinessCategory::MediaAndEntertainment),
            12 => Ok(BusinessCategory::InternalCommunications),
            13 => Ok(BusinessCategory::Transportation),
            14 => Ok(BusinessCategory::Telecommunications),
            _ => Ok(BusinessCategory::Other),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZBusinessPackage {
    pub label: Option<HashMap<String, String>>,
    pub pkg_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub zalo_name: String,
    pub avatar: String,
    pub bgavatar: String,
    pub cover: String,
    pub gender: crate::models::Gender,
    pub dob: i64,
    pub sdob: String,
    pub status: String,
    pub phone_number: String,
    pub is_fr: i32,
    pub is_blocked: i32,
    pub last_action_time: u64,
    pub last_update_time: u64,
    pub is_active: i32,
    pub key: i32,
    pub r#type: i32,
    #[serde(rename = "isActivePC")]
    pub is_active_pc: i32,
    pub is_active_web: i32,
    pub is_valid: i32,
    pub user_key: String,
    pub account_status: i32,
    pub oa_info: serde_json::Value,
    #[serde(rename = "user_mode")]
    pub user_mode: i32,
    pub global_id: String,
    pub biz_pkg: ZBusinessPackage,
    pub created_ts: u64,
    #[serde(rename = "oa_status")]
    pub oa_status: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserBasic {
    pub avatar: String,
    pub cover: String,
    pub status: String,
    pub gender: crate::models::Gender,
    pub dob: i64,
    pub sdob: String,
    pub global_id: String,
    pub biz_pkg: ZBusinessPackage,
    pub uid: String,
    #[serde(rename = "zalo_name")]
    pub zalo_name: String,
    #[serde(rename = "display_name")]
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSetting {
    pub add_friend_via_contact: i32,
    pub display_on_recommend_friend: i32,
    pub add_friend_via_group: i32,
    pub add_friend_via_qr: i32,
    pub quick_message_status: i32,
    pub show_online_status: bool,
    pub accept_stranger_call: i32,
    pub archived_chat_status: i32,
    pub receive_message: i32,
    pub add_friend_via_phone: i32,
    pub display_seen_status: i32,
    pub view_birthday: i32,
    #[serde(rename = "setting_2FA_status")]
    pub setting_2fa_status: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutoReplyScope {
    Everyone = 0,
    Stranger = 1,
    SpecificFriends = 2,
    FriendsExcept = 3,
}

impl Serialize for AutoReplyScope {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for AutoReplyScope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            0 => Ok(AutoReplyScope::Everyone),
            1 => Ok(AutoReplyScope::Stranger),
            2 => Ok(AutoReplyScope::SpecificFriends),
            3 => Ok(AutoReplyScope::FriendsExcept),
            _ => Ok(AutoReplyScope::Everyone),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoReplyItem {
    pub id: u64,
    pub weight: i32,
    pub enable: bool,
    pub modified_time: u64,
    pub start_time: u64,
    pub end_time: u64,
    pub content: String,
    pub scope: AutoReplyScope,
    pub uids: Option<Vec<String>>,
    pub owner_id: u64,
    pub recurrence: Vec<String>,
    pub created_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub ownerId: String,
    pub isDefault: bool,
    pub path: String,
    pub catalogPhoto: Option<String>,
    pub totalProduct: u32,
    pub created_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductCatalogItem {
    pub price: String,
    pub description: String,
    pub path: String,
    pub product_id: String,
    pub product_name: String,
    pub currency_unit: String,
    pub product_photos: Vec<String>,
    pub create_time: u64,
    pub catalog_id: String,
    pub owner_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelData {
    pub id: u32,
    pub text: String,
    pub text_key: String,
    pub conversations: Vec<String>,
    pub color: String,
    pub offset: i32,
    pub emoji: String,
    pub create_time: u64,
}
