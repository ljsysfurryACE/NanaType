//! blivechat's client protocol: `{"cmd": n, "data": ...}` frames.
//!
//! Text messages are positional arrays that grew over versions (16 fields in
//! v1.7, 19 in v1.10); gifts, memberships and Super Chats are objects. Every
//! field is read leniently, so a newer or older server never drops a message.

use serde_json::{Value, json};

const HEARTBEAT: i64 = 0;
const JOIN_ROOM: i64 = 1;
const ADD_TEXT: i64 = 2;
const ADD_GIFT: i64 = 3;
const ADD_MEMBER: i64 = 4;
const ADD_SUPER_CHAT: i64 = 5;
const DEL_SUPER_CHAT: i64 = 6;
const FATAL_ERROR: i64 = 8;

/// Which room to join.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RoomKey {
    /// A live room number.
    RoomId(u64),
    /// The streamer's 身份码 (open platform auth code).
    AuthCode(String),
}

impl RoomKey {
    /// Read what the user typed: a room number or an auth code.
    pub fn parse(auth_code: bool, text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        if auth_code {
            Some(Self::AuthCode(text.to_owned()))
        } else {
            text.parse().ok().filter(|id| *id > 0).map(Self::RoomId)
        }
    }
}

/// Guard (大航海) level. blivechat's `privilegeType`: 1 is the highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Guard {
    #[default]
    None,
    /// 总督
    Governor,
    /// 提督
    Admiral,
    /// 舰长
    Captain,
}

impl Guard {
    pub fn from_privilege(value: i64) -> Self {
        match value {
            1 => Self::Governor,
            2 => Self::Admiral,
            3 => Self::Captain,
            _ => Self::None,
        }
    }
}

/// Who sent a message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Author {
    /// Bilibili uid; empty for very old servers or anonymous senders.
    pub uid: String,
    pub name: String,
    /// `https://` avatar URL; empty when the server gave none.
    pub avatar_url: String,
    pub guard: Guard,
    /// The room's owner.
    pub owner: bool,
    /// A room moderator.
    pub admin: bool,
    pub medal_level: u32,
    pub medal_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatEvent {
    /// A danmaku: text, or a single emoticon picture.
    Text {
        id: String,
        author: Author,
        content: String,
        /// `https://` URL of the emoticon when the danmaku is one.
        emoticon: Option<String>,
        /// Unix milliseconds.
        at_ms: u64,
    },
    Gift {
        id: String,
        author: Author,
        gift_name: String,
        num: u32,
        /// Paid value in gold coins (1000 = ¥1); 0 for free gifts.
        total_coin: u64,
        at_ms: u64,
    },
    /// Someone bought a guard membership (上舰).
    Member {
        id: String,
        author: Author,
        num: u32,
        /// "月" and the like.
        unit: String,
        total_coin: u64,
        at_ms: u64,
    },
    SuperChat {
        id: String,
        author: Author,
        /// Price in yuan.
        price: u64,
        content: String,
        at_ms: u64,
    },
    /// Super Chats taken down by the room.
    DeleteSuperChats { ids: Vec<String> },
    /// The server gave up on the room.
    Fatal { kind: FatalKind, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatalKind {
    /// The auth code is wrong; retrying will not help.
    AuthCode,
    TooManyRetries,
    TooManyConnections,
    Other,
}

/// One frame from the server.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Heartbeat,
    Event(ChatEvent),
    /// Something this client does not use (translations, unknown commands).
    Other,
}

/// The join request. It carries `roomId` too, which v1.7 servers read.
pub fn join_message(room: &RoomKey) -> String {
    let (key, legacy) = match room {
        RoomKey::RoomId(id) => (json!({ "type": 1, "value": id }), json!(id)),
        RoomKey::AuthCode(code) => (json!({ "type": 2, "value": code }), Value::Null),
    };
    let mut data = json!({
        "roomKey": key,
        "config": { "autoTranslate": false },
    });
    if !legacy.is_null() {
        data["roomId"] = legacy;
    }
    json!({ "cmd": JOIN_ROOM, "data": data }).to_string()
}

pub fn heartbeat_message() -> String {
    json!({ "cmd": HEARTBEAT, "data": {} }).to_string()
}

/// Read one text frame. `None` for frames that are not JSON at all.
pub fn parse(text: &str) -> Option<Incoming> {
    let frame: Value = serde_json::from_str(text).ok()?;
    let data = &frame["data"];
    Some(match frame["cmd"].as_i64()? {
        HEARTBEAT => Incoming::Heartbeat,
        ADD_TEXT => text_message(data).map_or(Incoming::Other, Incoming::Event),
        ADD_GIFT => Incoming::Event(ChatEvent::Gift {
            id: string(&data["id"]),
            author: object_author(data),
            gift_name: string(&data["giftName"]),
            num: int(&data["num"]).clamp(1, u32::MAX as i64) as u32,
            total_coin: int(&data["totalCoin"]).max(0) as u64,
            at_ms: millis(&data["timestamp"]),
        }),
        ADD_MEMBER => Incoming::Event(ChatEvent::Member {
            id: string(&data["id"]),
            author: object_author(data),
            num: int(&data["num"]).clamp(1, u32::MAX as i64) as u32,
            unit: string(&data["unit"]),
            total_coin: int(&data["total_coin"]).max(0) as u64,
            at_ms: millis(&data["timestamp"]),
        }),
        ADD_SUPER_CHAT => Incoming::Event(ChatEvent::SuperChat {
            id: string(&data["id"]),
            author: object_author(data),
            price: int(&data["price"]).max(0) as u64,
            content: string(&data["content"]),
            at_ms: millis(&data["timestamp"]),
        }),
        DEL_SUPER_CHAT => Incoming::Event(ChatEvent::DeleteSuperChats {
            ids: data["ids"]
                .as_array()
                .map(|ids| ids.iter().map(string).collect())
                .unwrap_or_default(),
        }),
        FATAL_ERROR => Incoming::Event(ChatEvent::Fatal {
            kind: match int(&data["type"]) {
                1 => FatalKind::AuthCode,
                2 => FatalKind::TooManyRetries,
                3 => FatalKind::TooManyConnections,
                _ => FatalKind::Other,
            },
            message: string(&data["msg"]),
        }),
        _ => Incoming::Other,
    })
}

/// `ADD_TEXT`: `[avatarUrl, timestamp, authorName, authorType, content,
/// privilegeType, isGiftDanmaku, authorLevel, isNewbie, isMobileVerified,
/// medalLevel, id, translation, contentType, contentTypeParams,
/// textEmoticons, uid, medalName, isMirror]`.
fn text_message(data: &Value) -> Option<ChatEvent> {
    let a = data.as_array()?;
    let at = |i: usize| a.get(i).unwrap_or(&Value::Null);
    let author_type = int(at(3));
    let author = Author {
        uid: string(at(16)),
        name: string(at(2)),
        avatar_url: url(at(0)),
        guard: Guard::from_privilege(int(at(5))),
        owner: author_type == 3,
        admin: author_type == 2,
        medal_level: int(at(10)).max(0) as u32,
        medal_name: string(at(17)),
    };
    let emoticon = (int(at(13)) == 1)
        .then(|| at(14).get(0).map(url))
        .flatten()
        .filter(|u| !u.is_empty());
    Some(ChatEvent::Text {
        id: string(at(11)),
        author,
        content: string(at(4)),
        emoticon,
        at_ms: millis(at(1)),
    })
}

fn object_author(data: &Value) -> Author {
    Author {
        uid: string(&data["uid"]),
        name: string(&data["authorName"]),
        avatar_url: url(&data["avatarUrl"]),
        guard: Guard::from_privilege(int(&data["privilegeType"])),
        owner: false,
        admin: false,
        medal_level: int(&data["medalLevel"]).max(0) as u32,
        medal_name: string(&data["medalName"]),
    }
}

/// A string or a number as text (uids and ids come as either).
fn string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn int(v: &Value) -> i64 {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .unwrap_or(0),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        Value::Bool(b) => i64::from(*b),
        _ => 0,
    }
}

/// Seconds or milliseconds (sources differ) as milliseconds.
fn millis(v: &Value) -> u64 {
    let t = match v {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    };
    if !t.is_finite() || t <= 0.0 {
        0
    } else if t > 1e12 {
        t as u64
    } else {
        (t * 1000.0) as u64
    }
}

/// Avatars and emoticons come scheme-relative (`//i0.hdslb.com/...`).
fn url(v: &Value) -> String {
    let s = string(v);
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("//") {
        format!("https://{rest}")
    } else if let Some(rest) = s.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        s.to_owned()
    }
}
