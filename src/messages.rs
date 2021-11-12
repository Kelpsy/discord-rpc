use super::{Presence, User};
use serde::{ser::SerializeMap, Deserialize, Serialize, Serializer};

#[derive(Clone, Copy, Debug)]
pub struct SetActivity<'a> {
    pub pid: u32,
    pub nonce: i32,
    pub presence: Option<&'a Presence>,
}

impl<'a> Serialize for SetActivity<'a> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        struct Args<'a>(&'a SetActivity<'a>);

        impl<'a> Serialize for Args<'a> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut obj = serializer.serialize_map(None)?;
                obj.serialize_entry("pid", &self.0.pid)?;
                if let Some(activity) = &self.0.presence {
                    obj.serialize_entry("activity", activity)?;
                }
                obj.end()
            }
        }

        let mut obj = serializer.serialize_map(Some(3))?;
        obj.serialize_entry("cmd", "SET_ACTIVITY")?;
        obj.serialize_entry("nonce", &self.nonce)?;
        obj.serialize_entry("args", &Args(self))?;
        obj.end()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Handshake<'a> {
    pub version: i32,
    pub app_id: &'a str,
}

impl<'a> Serialize for Handshake<'a> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut obj = serializer.serialize_map(Some(2))?;
        obj.serialize_entry("v", &self.version)?;
        obj.serialize_entry("client_id", self.app_id)?;
        obj.end()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ToggleSubscription<'a, const ENABLED: bool> {
    pub nonce: i32,
    pub event: &'a str,
}

impl<'a, const ENABLED: bool> Serialize for ToggleSubscription<'a, ENABLED> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut obj = serializer.serialize_map(Some(3))?;
        obj.serialize_entry("cmd", if ENABLED { "SUBSCRIBE" } else { "UNSUBSCRIBE" })?;
        obj.serialize_entry("nonce", &self.nonce)?;
        obj.serialize_entry("event", self.event)?;
        obj.end()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct JoinReply<'a> {
    pub nonce: i32,
    pub accepted: bool,
    pub user_id: &'a str,
}

impl<'a> Serialize for JoinReply<'a> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        struct Args<'a>(&'a JoinReply<'a>);

        impl<'a> Serialize for Args<'a> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut obj = serializer.serialize_map(None)?;
                obj.serialize_entry("user_id", self.0.user_id)?;
                obj.end()
            }
        }

        let mut obj = serializer.serialize_map(Some(2))?;
        obj.serialize_entry(
            "cmd",
            if self.accepted {
                "SEND_ACTIVITY_JOIN_INVITE"
            } else {
                "CLOSE_ACTIVITY_JOIN_REQUEST"
            },
        )?;
        obj.serialize_entry("nonce", &self.nonce)?;
        obj.serialize_entry("args", &Args(self))?;
        obj.end()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct HandshakeReply {
    #[serde(rename = "cmd")]
    pub command: String,
    #[serde(rename = "evt")]
    pub event: String,
    pub data: HandshakeReplyData,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HandshakeReplyData {
    pub user: Option<User>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Event {
    #[serde(rename = "evt")]
    pub event: String,
    pub data: serde_json::Map<String, serde_json::Value>,
}
