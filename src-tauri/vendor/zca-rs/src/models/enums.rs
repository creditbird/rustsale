#![allow(non_camel_case_types)]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreadType {
    User = 0,
    Group = 1,
}

impl Serialize for ThreadType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for ThreadType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            0 => Ok(ThreadType::User),
            1 => Ok(ThreadType::Group),
            _ => Ok(ThreadType::User),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DestType {
    Group = 1,
    User = 3,
    Page = 5,
}

impl Serialize for DestType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for DestType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            1 => Ok(DestType::Group),
            3 => Ok(DestType::User),
            5 => Ok(DestType::Page),
            _ => Ok(DestType::User),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gender {
    Male = 0,
    Female = 1,
}

impl Serialize for Gender {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for Gender {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            0 => Ok(Gender::Male),
            1 => Ok(Gender::Female),
            _ => Ok(Gender::Male),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AvatarSize {
    Small = 120,
    Large = 240,
}

impl Serialize for AvatarSize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(*self as u16)
    }
}

impl<'de> Deserialize<'de> for AvatarSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u16::deserialize(deserializer)?;
        match val {
            120 => Ok(AvatarSize::Small),
            240 => Ok(AvatarSize::Large),
            _ => Ok(AvatarSize::Small),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinBankCard {
    ABBank = 970425,
    ACB = 970416,
    Agribank = 970405,
    BIDV = 970418,
    BVBank = 970454,
    BacA_Bank = 970409,
    BaoViet_Bank = 970438,
    CAKE = 546034,
    CB_Bank = 970444,
    CIMB_Bank = 422589,
    Coop_Bank = 970446,
    DBS_Bank = 796500,
    DongA_Bank = 970406,
    Eximbank = 970431,
    GPBank = 970408,
    HDBank = 970437,
    HSBC = 458761,
    HongLeong_Bank = 970442,
    IBK_HCM = 970456,
    IBK_HN = 970455,
    Indovina_Bank = 970434,
    KBank = 668888,
    KienlongBank = 970452,
    Kookmin_Bank_HCM = 970463,
    Kookmin_Bank_HN = 970462,
    LPBank = 970449,
    MB_Bank = 970422,
    MSB = 970426,
    NCB = 970419,
    Nam_A_Bank = 970428,
    NongHyup_Bank = 801011,
    OCB = 970448,
    Ocean_Bank = 970414,
    PGBank = 970430,
    PVcomBank = 970412,
    Public_Bank_Vietnam = 970439,
    SCB = 970429,
    SHB = 970443,
    Sacombank = 970403,
    Saigon_Bank = 970400,
    SeABank = 970440,
    Shinhan_Bank = 970424,
    Standard_Chartered_Vietnam = 970410,
    TNEX = 9704261,
    TPBank = 970423,
    Techcombank = 970407,
    Timo = 963388,
    UBank = 546035,
    United_Overseas_Bank_Vietnam = 970458,
    VIB = 970441,
    VPBank = 970432,
    VRB = 970421,
    VietABank = 970427,
    VietBank = 970433,
    Vietcombank = 970436,
    VietinBank = 970415,
    Woori_Bank = 970457,
}

impl Serialize for BinBankCard {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(*self as u32)
    }
}

impl<'de> Deserialize<'de> for BinBankCard {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u32::deserialize(deserializer)?;
        match val {
            970425 => Ok(BinBankCard::ABBank),
            970416 => Ok(BinBankCard::ACB),
            970405 => Ok(BinBankCard::Agribank),
            970418 => Ok(BinBankCard::BIDV),
            970454 => Ok(BinBankCard::BVBank),
            970409 => Ok(BinBankCard::BacA_Bank),
            970438 => Ok(BinBankCard::BaoViet_Bank),
            546034 => Ok(BinBankCard::CAKE),
            970444 => Ok(BinBankCard::CB_Bank),
            422589 => Ok(BinBankCard::CIMB_Bank),
            970446 => Ok(BinBankCard::Coop_Bank),
            796500 => Ok(BinBankCard::DBS_Bank),
            970406 => Ok(BinBankCard::DongA_Bank),
            970431 => Ok(BinBankCard::Eximbank),
            970408 => Ok(BinBankCard::GPBank),
            970437 => Ok(BinBankCard::HDBank),
            458761 => Ok(BinBankCard::HSBC),
            970442 => Ok(BinBankCard::HongLeong_Bank),
            970456 => Ok(BinBankCard::IBK_HCM),
            970455 => Ok(BinBankCard::IBK_HN),
            970434 => Ok(BinBankCard::Indovina_Bank),
            668888 => Ok(BinBankCard::KBank),
            970452 => Ok(BinBankCard::KienlongBank),
            970463 => Ok(BinBankCard::Kookmin_Bank_HCM),
            970462 => Ok(BinBankCard::Kookmin_Bank_HN),
            970449 => Ok(BinBankCard::LPBank),
            970422 => Ok(BinBankCard::MB_Bank),
            970426 => Ok(BinBankCard::MSB),
            970419 => Ok(BinBankCard::NCB),
            970428 => Ok(BinBankCard::Nam_A_Bank),
            801011 => Ok(BinBankCard::NongHyup_Bank),
            970448 => Ok(BinBankCard::OCB),
            970414 => Ok(BinBankCard::Ocean_Bank),
            970430 => Ok(BinBankCard::PGBank),
            970412 => Ok(BinBankCard::PVcomBank),
            970439 => Ok(BinBankCard::Public_Bank_Vietnam),
            970429 => Ok(BinBankCard::SCB),
            970443 => Ok(BinBankCard::SHB),
            970403 => Ok(BinBankCard::Sacombank),
            970400 => Ok(BinBankCard::Saigon_Bank),
            970440 => Ok(BinBankCard::SeABank),
            970424 => Ok(BinBankCard::Shinhan_Bank),
            970410 => Ok(BinBankCard::Standard_Chartered_Vietnam),
            9704261 => Ok(BinBankCard::TNEX),
            970423 => Ok(BinBankCard::TPBank),
            970407 => Ok(BinBankCard::Techcombank),
            963388 => Ok(BinBankCard::Timo),
            546035 => Ok(BinBankCard::UBank),
            970458 => Ok(BinBankCard::United_Overseas_Bank_Vietnam),
            970441 => Ok(BinBankCard::VIB),
            970432 => Ok(BinBankCard::VPBank),
            970421 => Ok(BinBankCard::VRB),
            970427 => Ok(BinBankCard::VietABank),
            970433 => Ok(BinBankCard::VietBank),
            970436 => Ok(BinBankCard::Vietcombank),
            970415 => Ok(BinBankCard::VietinBank),
            970457 => Ok(BinBankCard::Woori_Bank),
            _ => Ok(BinBankCard::Vietcombank),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Reactions {
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
    #[serde(rename = "")]
    None,
}

impl Reactions {
    pub fn to_type_and_source(&self) -> (i32, i32) {
        match self {
            Reactions::Haha => (0, 6),
            Reactions::Like => (3, 6),
            Reactions::Heart => (5, 6),
            Reactions::Wow => (32, 6),
            Reactions::Cry => (2, 6),
            Reactions::Angry => (20, 6),
            Reactions::Kiss => (8, 6),
            Reactions::TearsOfJoy => (7, 6),
            Reactions::Shit => (66, 6),
            Reactions::Rose => (120, 6),
            Reactions::BrokenHeart => (65, 6),
            Reactions::Dislike => (4, 6),
            Reactions::Love => (29, 6),
            Reactions::Confused => (51, 6),
            Reactions::Wink => (45, 6),
            Reactions::Fade => (121, 6),
            Reactions::Sun => (67, 6),
            Reactions::Birthday => (126, 6),
            Reactions::Bomb => (127, 6),
            Reactions::Ok => (68, 6),
            Reactions::Peace => (69, 6),
            Reactions::Thanks => (70, 6),
            Reactions::Punch => (71, 6),
            Reactions::Share => (72, 6),
            Reactions::Pray => (73, 6),
            Reactions::No => (131, 6),
            Reactions::Bad => (132, 6),
            Reactions::LoveYou => (133, 6),
            Reactions::Sad => (1, 6),
            Reactions::VerySad => (16, 6),
            Reactions::Cool => (21, 6),
            Reactions::Nerd => (22, 6),
            Reactions::BigSmile => (23, 6),
            Reactions::Sunglasses => (26, 6),
            Reactions::Neutral => (30, 6),
            Reactions::SadFace => (35, 6),
            Reactions::Bye => (36, 6),
            Reactions::Sleepy => (38, 6),
            Reactions::Wipe => (39, 6),
            Reactions::Dig => (42, 6),
            Reactions::Anguish => (44, 6),
            Reactions::Handclap => (46, 6),
            Reactions::AngryFace => (47, 6),
            Reactions::FChair => (48, 6),
            Reactions::LChair => (49, 6),
            Reactions::RChair => (50, 6),
            Reactions::Silent => (52, 6),
            Reactions::Surprise => (53, 6),
            Reactions::Embarrassed => (54, 6),
            Reactions::Afraid => (60, 6),
            Reactions::Sad2 => (61, 6),
            Reactions::BigLaugh => (62, 6),
            Reactions::Rich => (63, 6),
            Reactions::Beer => (99, 6),
            Reactions::None => (-1, 6),
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Reactions::Heart => "/-heart",
            Reactions::Like => "/-strong",
            Reactions::Haha => ":>",
            Reactions::Wow => ":o",
            Reactions::Cry => ":-((",
            Reactions::Angry => ":-h",
            Reactions::Kiss => ":-*",
            Reactions::TearsOfJoy => ":')",
            Reactions::Shit => "/-shit",
            Reactions::Rose => "/-rose",
            Reactions::BrokenHeart => "/-break",
            Reactions::Dislike => "/-weak",
            Reactions::Love => ";xx",
            Reactions::Confused => ";-/",
            Reactions::Wink => ";-)",
            Reactions::Fade => "/-fade",
            Reactions::Sun => "/-li",
            Reactions::Birthday => "/-bd",
            Reactions::Bomb => "/-bome",
            Reactions::Ok => "/-ok",
            Reactions::Peace => "/-v",
            Reactions::Thanks => "/-thanks",
            Reactions::Punch => "/-punch",
            Reactions::Share => "/-share",
            Reactions::Pray => "_()_",
            Reactions::No => "/-no",
            Reactions::Bad => "/-bad",
            Reactions::LoveYou => "/-loveu",
            Reactions::Sad => "--b",
            Reactions::VerySad => ":((",
            Reactions::Cool => "x-)",
            Reactions::Nerd => "8-)",
            Reactions::BigSmile => ";-d",
            Reactions::Sunglasses => "b-)",
            Reactions::Neutral => ":--|",
            Reactions::SadFace => "p-(",
            Reactions::Bye => ":-bye",
            Reactions::Sleepy => "|-)",
            Reactions::Wipe => ":wipe",
            Reactions::Dig => ":-dig",
            Reactions::Anguish => "&-(",
            Reactions::Handclap => ":handclap",
            Reactions::AngryFace => ">-|",
            Reactions::FChair => ":-f",
            Reactions::LChair => ":-l",
            Reactions::RChair => ":-r",
            Reactions::Silent => ";-x",
            Reactions::Surprise => ":-o",
            Reactions::Embarrassed => ";-s",
            Reactions::Afraid => ";-a",
            Reactions::Sad2 => ":-<",
            Reactions::BigLaugh => ":))",
            Reactions::Rich => "$-)",
            Reactions::Beer => "/-beer",
            Reactions::None => "",
        }
    }
}
