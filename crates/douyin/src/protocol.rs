//! Independently written, deliberately sparse protobuf messages for webcast wire fields.
//! Unknown fields are ignored by prost. No generated upstream decoder or SDK is included.
use crate::{Author, ChatEvent, Content};
use prost::Message;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read;

pub const MAX_FRAME: usize = 4 * 1024 * 1024;
const MAX_INFLATED: usize = 16 * 1024 * 1024;
const RETAINED: usize = 8192;

#[derive(Clone, PartialEq, Message)]
pub struct PushFrame {
    #[prost(uint64, tag = "2")]
    pub log_id: u64,
    #[prost(message, repeated, tag = "5")]
    pub headers: Vec<Header>,
    #[prost(string, tag = "6")]
    pub encoding: String,
    #[prost(string, tag = "7")]
    pub kind: String,
    #[prost(bytes = "vec", tag = "8")]
    pub payload: Vec<u8>,
}
#[derive(Clone, PartialEq, Message)]
pub struct Header {
    #[prost(string, tag = "1")]
    pub key: String,
    #[prost(string, tag = "2")]
    pub value: String,
}
#[derive(Clone, PartialEq, Message)]
pub struct Response {
    #[prost(message, repeated, tag = "1")]
    pub messages: Vec<Envelope>,
    #[prost(string, tag = "2")]
    pub cursor: String,
    #[prost(string, tag = "5")]
    pub internal_ext: String,
    #[prost(uint64, tag = "8")]
    pub heartbeat_seconds: u64,
    #[prost(bool, tag = "9")]
    pub need_ack: bool,
    #[prost(string, tag = "10")]
    pub push_server: String,
    #[prost(string, tag = "13")]
    pub proxy_server: String,
    #[prost(string, tag = "14")]
    pub push_server_v2: String,
}
#[derive(Clone, PartialEq, Message)]
pub struct Envelope {
    #[prost(string, tag = "1")]
    pub method: String,
    #[prost(bytes = "vec", tag = "2")]
    pub payload: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub id: u64,
}
#[derive(Clone, PartialEq, Message)]
struct Common {
    #[prost(uint64, tag = "2")]
    id: u64,
    #[prost(uint64, tag = "4")]
    created: u64,
}
#[derive(Clone, PartialEq, Message)]
struct User {
    #[prost(uint64, tag = "1")]
    id: u64,
    #[prost(string, tag = "3")]
    name: String,
    #[prost(message, optional, tag = "9")]
    avatar: Option<Image>,
}
#[derive(Clone, PartialEq, Message)]
struct Image {
    #[prost(string, repeated, tag = "1")]
    urls: Vec<String>,
}
#[derive(Clone, PartialEq, Message)]
struct Text {
    #[prost(string, tag = "2")]
    pattern: String,
    #[prost(message, repeated, tag = "4")]
    pieces: Vec<TextPiece>,
}
#[derive(Clone, PartialEq, Message)]
struct TextPiece {
    #[prost(string, tag = "11")]
    text: String,
    #[prost(message, optional, tag = "25")]
    image: Option<TextImage>,
}
#[derive(Clone, PartialEq, Message)]
struct TextImage {
    #[prost(message, optional, tag = "1")]
    image: Option<Image>,
}
#[derive(Clone, PartialEq, Message)]
struct Chat {
    #[prost(message, optional, tag = "1")]
    common: Option<Common>,
    #[prost(message, optional, tag = "2")]
    user: Option<User>,
    #[prost(string, tag = "3")]
    text: String,
    #[prost(message, optional, tag = "40")]
    rich: Option<Text>,
}
#[derive(Clone, PartialEq, Message)]
struct Emoji {
    #[prost(message, optional, tag = "1")]
    common: Option<Common>,
    #[prost(message, optional, tag = "2")]
    user: Option<User>,
    #[prost(message, optional, tag = "4")]
    rich: Option<Text>,
    #[prost(string, tag = "5")]
    text: String,
}
#[derive(Clone, PartialEq, Message)]
struct Gift {
    #[prost(message, optional, tag = "1")]
    common: Option<Common>,
    #[prost(uint64, tag = "2")]
    gift_id: u64,
    #[prost(uint64, tag = "5")]
    repeat: u64,
    #[prost(message, optional, tag = "7")]
    user: Option<User>,
    #[prost(message, optional, tag = "8")]
    recipient: Option<User>,
    #[prost(uint64, tag = "11")]
    group: u64,
    #[prost(message, optional, tag = "15")]
    gift: Option<GiftInfo>,
}
#[derive(Clone, PartialEq, Message)]
struct GiftInfo {
    #[prost(int32, optional, tag = "12")]
    diamonds: Option<i32>,
    #[prost(string, tag = "16")]
    name: String,
}

pub struct Packet {
    pub response: Response,
    pub ack: Option<Vec<u8>>,
}
pub fn decode(bytes: &[u8]) -> Result<Packet, &'static str> {
    if bytes.len() > MAX_FRAME {
        return Err("frame-too-large");
    }
    let push = PushFrame::decode(bytes).map_err(|_| "invalid-frame")?;
    if !matches!(push.kind.as_str(), "msg" | "") {
        return Ok(Packet {
            response: Response::default(),
            ack: None,
        });
    }
    let gzip = push.encoding == "gzip"
        || push
            .headers
            .iter()
            .any(|h| h.key == "compress_type" && h.value == "gzip")
        || push.payload.starts_with(&[31, 139]);
    let payload = if gzip {
        let mut output = Vec::new();
        flate2::read::GzDecoder::new(push.payload.as_slice())
            .take((MAX_INFLATED + 1) as u64)
            .read_to_end(&mut output)
            .map_err(|_| "invalid-gzip")?;
        if output.len() > MAX_INFLATED {
            return Err("payload-too-large");
        }
        output
    } else {
        push.payload
    };
    let response = Response::decode(payload.as_slice()).map_err(|_| "invalid-response")?;
    let ack = response.need_ack.then(|| {
        let value = push
            .headers
            .iter()
            .find(|h| h.key == "im-internal_ext")
            .map(|h| h.value.as_str())
            .unwrap_or(&response.internal_ext);
        PushFrame {
            log_id: push.log_id,
            kind: "ack".into(),
            payload: value.as_bytes().to_vec(),
            ..Default::default()
        }
        .encode_to_vec()
    });
    Ok(Packet { response, ack })
}

type ComboKey = (u64, u64, u64, u64);
#[derive(Default)]
pub struct Decoder {
    seen: HashSet<u64>,
    order: VecDeque<u64>,
    combos: HashMap<ComboKey, u64>,
    combo_order: VecDeque<ComboKey>,
}
impl Decoder {
    pub fn messages(&mut self, response: &Response) -> Vec<ChatEvent> {
        response
            .messages
            .iter()
            .filter_map(|m| self.message(m))
            .collect()
    }
    fn message(&mut self, envelope: &Envelope) -> Option<ChatEvent> {
        // Failed payloads do not poison deduplication of a later valid retransmission.
        if envelope.method != "WebcastGiftMessage"
            && envelope.id != 0
            && self.seen.contains(&envelope.id)
        {
            return None;
        }
        let mut combo = false;
        let (common, user, content) = match envelope.method.as_str() {
            "WebcastChatMessage" => {
                let m = Chat::decode(envelope.payload.as_slice()).ok()?;
                (m.common, m.user, text(m.text, m.rich))
            }
            "WebcastEmojiChatMessage" => {
                let m = Emoji::decode(envelope.payload.as_slice()).ok()?;
                (m.common, m.user, text(m.text, m.rich))
            }
            "WebcastGiftMessage" => {
                let m = Gift::decode(envelope.payload.as_slice()).ok()?;
                let user = m.user.as_ref()?;
                let gift = m.gift?;
                let total = m.repeat.max(1);
                let count = if m.group != 0 {
                    combo = true;
                    let key = (
                        m.group,
                        user.id,
                        m.gift_id,
                        m.recipient.as_ref().map_or(0, |u| u.id),
                    );
                    let previous = self.combos.get(&key).copied().unwrap_or(0);
                    if total <= previous {
                        return None;
                    }
                    if self.combos.insert(key, total).is_none() {
                        self.combo_order.push_back(key);
                    }
                    while self.combo_order.len() > RETAINED {
                        if let Some(old) = self.combo_order.pop_front() {
                            self.combos.remove(&old);
                        }
                    }
                    total - previous
                } else {
                    total
                };
                (
                    m.common,
                    m.user,
                    Content::Gift {
                        name: gift.name,
                        count,
                        diamonds: gift.diamonds.filter(|n| *n >= 0).map(|n| n as u64),
                    },
                )
            }
            _ => return None,
        };
        let user = user?;
        let common = common.unwrap_or_default();
        let id = if envelope.id != 0 {
            envelope.id
        } else {
            common.id
        };
        if id != 0 && !combo {
            if !self.seen.insert(id) {
                return None;
            }
            self.order.push_back(id);
            while self.order.len() > RETAINED {
                if let Some(old) = self.order.pop_front() {
                    self.seen.remove(&old);
                }
            }
        }
        Some(ChatEvent {
            id: if id == 0 {
                String::new()
            } else {
                id.to_string()
            },
            author: Author {
                uid: if user.id == 0 {
                    String::new()
                } else {
                    user.id.to_string()
                },
                name: user.name,
                avatar: user
                    .avatar
                    .and_then(|i| i.urls.into_iter().next())
                    .unwrap_or_default(),
            },
            content,
            at: common.created.saturating_mul(1000),
        })
    }
}
fn text(value: String, rich: Option<Text>) -> Content {
    if let Some(rich) = rich {
        let label = if value.is_empty() {
            rich.pattern
        } else {
            value
        };
        if let Some(url) = rich.pieces.into_iter().find_map(|p| {
            p.image
                .and_then(|i| i.image)
                .and_then(|i| i.urls.into_iter().next())
        }) {
            return Content::Emote { text: label, url };
        }
        Content::Text(label)
    } else {
        Content::Text(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn compressed_batch_ack_and_unknown_fields() {
        let response = Response {
            need_ack: true,
            internal_ext: "wrong-for-ack".into(),
            messages: vec![Envelope {
                method: "future".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gzip.write_all(&response.encode_to_vec()).unwrap();
        let mut frame = PushFrame {
            log_id: 9,
            encoding: "gzip".into(),
            kind: "msg".into(),
            headers: vec![Header {
                key: "im-internal_ext".into(),
                value: "exact-header".into(),
            }],
            payload: gzip.finish().unwrap(),
        }
        .encode_to_vec();
        frame.extend_from_slice(&[0xa0, 0x06, 0x01]); // unknown varint field 100
        let packet = decode(&frame).unwrap();
        let ack = PushFrame::decode(packet.ack.unwrap().as_slice()).unwrap();
        assert_eq!(ack.log_id, 9);
        assert_eq!(ack.payload, b"exact-header");
        assert!(Decoder::default().messages(&packet.response).is_empty());
        assert!(decode(&[0xff]).is_err());
        assert!(decode(&vec![0; MAX_FRAME + 1]).is_err());
    }
    #[test]
    fn combo_updates_are_deltas_and_replays_do_not_duplicate() {
        let mut decoder = Decoder::default();
        let gift = |id, repeat| Envelope {
            id,
            method: "WebcastGiftMessage".into(),
            payload: Gift {
                user: Some(User {
                    id: 42,
                    ..Default::default()
                }),
                group: 7,
                gift_id: 8,
                repeat,
                gift: Some(GiftInfo {
                    name: "gift".into(),
                    diamonds: Some(2),
                }),
                ..Default::default()
            }
            .encode_to_vec(),
        };
        let result = decoder.messages(&Response {
            messages: vec![gift(1, 1), gift(1, 3), gift(3, 3), gift(1, 3)],
            ..Default::default()
        });
        assert_eq!(result.len(), 2);
        assert!(matches!(
            result[1].content,
            Content::Gift {
                count: 2,
                diamonds: Some(2),
                ..
            }
        ));
    }
    #[test]
    fn decompression_is_bounded() {
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gzip.write_all(&vec![0; MAX_INFLATED + 1]).unwrap();
        let frame = PushFrame {
            encoding: "gzip".into(),
            payload: gzip.finish().unwrap(),
            ..Default::default()
        }
        .encode_to_vec();
        assert!(matches!(decode(&frame), Err("payload-too-large")));
    }
}
