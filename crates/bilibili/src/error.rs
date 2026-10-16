// Copyright (c) 2026 Sena. MIT; see ../../../LICENSE-MIT.
use std::fmt;

/// 协议层统一错误:区分本地 IO、网络传输、HTTP 状态、业务码、解析失败与非法入参。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// Local authentication requirement; no request was sent.
    LoginRequired,
    /// 本地会话文件读写失败。
    Io(String),
    /// 网络传输失败(连接、TLS、超时等)。
    Network(String),
    /// HTTP 状态码非 2xx。
    Http {
        /// 响应状态码。
        status: u16,
    },
    /// B 站业务码非 0,`message` 为服务端返回的真实错误信息。
    Business {
        /// 业务错误码。
        code: i64,
        /// 服务端错误信息。
        message: String,
    },
    /// 响应体解析失败(JSON/protobuf)。
    Parse(String),
    /// 调用方入参不合法(如投币数量越界),未发起网络请求。
    InvalidInput(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoginRequired => write!(f, "login required"),
            Self::Io(message) => write!(f, "local session io error: {message}"),
            Self::Network(message) => write!(f, "network error: {message}"),
            Self::Http { status } => write!(f, "http status {status}"),
            Self::Business { code, message } => write!(f, "bilibili code {code}: {message}"),
            Self::Parse(message) => write!(f, "parse error: {message}"),
            Self::InvalidInput(message) => write!(f, "invalid input: {message}"),
        }
    }
}

impl std::error::Error for ApiError {}
