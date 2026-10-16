//! Authenticated Bilibili live chat. IO runs off the UI thread.
//!
//! Protocol documentation and WBI material come from
//! SocialSisterYi/bilibili-API-collect
//! (CC-BY-NC-4.0). See ../NOTICE.md and the accompanying licence files.
mod api;
mod cookie;
mod credentials;
mod error;
mod handle;
mod http;
mod live_ws;
mod login;
pub mod protocol;
pub mod sending;
mod wbi;
pub use api::{Profile, Room, SendReceipt};
pub use error::ApiError;
pub use handle::{
    AccountState, BilibiliConfig, BilibiliHandle, ConnectionState, Event, SessionEvent,
};
pub use http::Client;
pub use login::{QrLogin, QrPoll, QrStatus};
pub use protocol::{Author, ChatEvent, Guard};
pub use sending::{Delivery, DeliveryState};
