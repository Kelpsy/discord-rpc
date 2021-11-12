use serde::{Deserialize, Serialize, Serializer};
use std::time::SystemTime;

fn serialize_timestamp<S: Serializer>(
    value: &Option<SystemTime>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if let Some(timestamp) = value.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()) {
        serializer.serialize_u64(timestamp.as_secs())
    } else {
        serializer.serialize_none()
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Timestamps {
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_timestamp"
    )]
    pub start: Option<SystemTime>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_timestamp"
    )]
    pub end: Option<SystemTime>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Images {
    #[serde(rename = "large_image", skip_serializing_if = "Option::is_none")]
    pub large_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_text: Option<String>,
    #[serde(rename = "small_image", skip_serializing_if = "Option::is_none")]
    pub small_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_text: Option<String>,
}

fn private(value: &bool) -> bool {
    !*value
}

fn serialize_public<S: Serializer>(value: &bool, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u8(*value as u8)
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Party {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,
    #[serde(
        skip_serializing_if = "private",
        rename = "privacy",
        serialize_with = "serialize_public"
    )]
    pub public: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Secrets {
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub match_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectate: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Presence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Timestamps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Images>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub party: Option<Party>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Secrets>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>,
}
