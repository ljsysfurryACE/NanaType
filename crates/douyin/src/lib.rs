//! Rust-owned Douyin decoding and session management. No third-party web SDK is bundled.
mod credentials;
mod handle;
mod native;
pub mod protocol;
pub mod signing;

pub use handle::DouyinHandle;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub enabled: bool,
    pub room: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AccountState {
    #[default]
    SignedOut,
    Unverified,
    Ready {
        uid: String,
        name: String,
    },
    Expired,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ConnectionState {
    #[default]
    Off,
    Connecting,
    Connected {
        room: String,
    },
    Disconnected,
    Error(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    pub uid: String,
    pub name: String,
    pub avatar: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    Text(String),
    Emote {
        text: String,
        url: String,
    },
    Gift {
        name: String,
        count: u64,
        diamonds: Option<u64>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatEvent {
    pub id: String,
    pub author: Author,
    pub content: Content,
    pub at: u64,
}
#[derive(Debug, Clone)]
pub enum Event {
    Account(AccountState),
    Connection(ConnectionState),
    Chat {
        room: String,
        event: ChatEvent,
    },
    /// A stable diagnostic code; adapters localize it, never log raw transport URLs.
    Warning(&'static str),
}
#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub epoch: u64,
    pub event: Event,
}

/// Accept canonical live-room addresses or room numbers.
pub fn room_id(input: &str) -> Result<String, String> {
    let text = input.trim();
    let id = if text.starts_with("https://") || text.starts_with("http://") {
        let url = url::Url::parse(text).map_err(|_| "invalid-room")?;
        if url.host_str() != Some("live.douyin.com")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("invalid-room".into());
        }
        url.path().trim_matches('/').to_owned()
    } else {
        text.to_owned()
    };
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("invalid-room".into());
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn room_input_cannot_navigate_outside_douyin() {
        assert_eq!(
            room_id(" https://live.douyin.com/123?from=x ").unwrap(),
            "123"
        );
        assert!(room_id("https://live.douyin.com.evil.test/123").is_err());
        assert!(room_id("https://user@live.douyin.com/123").is_err());
        assert!(room_id("../profile").is_err());
        assert!(room_id("").is_err());
    }
}
