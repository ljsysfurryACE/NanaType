// Copyright (c) 2026 Sena. MIT; see ../../../LICENSE-MIT.
// Adapted protocol documentation: SocialSisterYi/bilibili-API-collect and
// contributors, CC-BY-NC-4.0; see ../NOTICE.md and ../LICENSE-API-COLLECT.
//! 直播间弹幕 WebSocket 协议:认证、心跳、数据包编解码与会话。
//!
//! 文档依据(bilibili-API-collect 镜像 docs/live/message_stream.md):
//! - 包头 16 字节大端:u32 封包总长 / u16 头长(恒 16)/ u16 协议版本 /
//!   u32 操作码 / u32 sequence;
//! - 操作码 2=心跳 3=心跳回复 5=普通命令包 7=认证 8=认证回复;
//! - 认证包正文 JSON `{uid, roomid, protover, platform, type, key}`,连接后
//!   尽快发送(文档:5 秒内);protover 请求 3(brotli);另按 web 端惯例附
//!   buvid3(缺 buvid 的连接可能被服务端很快断开);
//! - 命令包正文:protover 0/1 为不压缩 JSON;2 为 zlib;3 为 brotli 压缩的
//!   「多个带头普通包」,解压后按包头逐包递归;
//! - 心跳回复正文前 4 字节为人气值(大端 u32,文档示例 `00 00 09 a2`=2466);
//! - 心跳约 30 秒一次,超时不发服务端约 60 秒后断连。
//!
//! 事件索引依据:`DANMU_MSG` 的 `info[1]` 为弹幕文本、`info[2][1]` 为用户名
//! (镜像文档「DANMU_MSG 结构」),`info[3]` 为佩戴的粉丝牌(`[等级, 牌子名, …]`,
//! 未佩戴为空数组)、`info[4][0]` 为用户等级;`SEND_GIFT` 的 `data.uname`/
//! `data.giftName`/`data.num` 为投喂者、礼物名与数量(镜像文档「SEND_GIFT」:
//! `data` 是命名字段对象,非位置数组);`SUPER_CHAT_MESSAGE` 取 `data.id`、
//! `data.user_info.uname`、`data.message`、`data.price`(元)与 `data.time`
//! (置顶秒数)。cmd 亦常带版本后缀(如
//! `DANMU_MSG:4:0:2:2:2:0`),位置解析失败时退化为 unknown events
//! 透传,事件流不断。脆性说明:`SEND_GIFT` 的 combo(`combo_send` 等待调查)
//! 与盲盒(`blind_gift` 等待调查)形态字段文档尚不稳定,只取上述稳定
//! 子集,连击合并等形态不单独建模。
//!
//! 消费约定:会话为同步阻塞实现,调用方(UI 层)在专用线程以
//! [`HEARTBEAT_INTERVAL`] 周期发心跳并循环 [`LiveSocket::next_event`]。
//! **心跳不得排在 `next_event` 返回之后,必须独立周期调度**(计时触发,
//! 而非「取到事件后再发」):`next_event` 的 timeout 是单次底层读的上界
//! 而非调用整体时界,服务端慢速滴流喂字节时该调用可长期不返回,把心跳
//! 挂在其返回路径上会停止发心跳,约 60 秒后被服务端断连。
//! 本模块不做自动重连:收到 [`LiveEvent::Disconnected`] 后由调用方重新
//! `danmu_info` + `connect` 重建会话,重连退避策略归 UI 层。

use std::collections::VecDeque;
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket, client_tls_with_config, http};

use crate::ApiError;
use crate::http::USER_AGENT;

/// 弹幕服务器 wss 入口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveHost {
    /// 域名(getDanmuInfo `host_list[].host`)。
    pub host: String,
    /// wss 端口(同上 `wss_port`)。
    pub wss_port: u16,
}

/// `getDanmuInfo` 结果:认证 token 与候选入口列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DanmakuInfo {
    /// 真实房间号(认证包 body 的 `roomid`)。
    pub room_id: i64,
    /// 认证 uid:登录用户 mid,游客为 0。
    pub uid: i64,
    /// 服务端下发的认证 token(body 的 `key`)。
    pub token: String,
    /// 认证附带的 buvid3(客户端未持有时 None;缺失可能被服务端尽快断开)。
    pub buvid: Option<String>,
    /// 候选 wss 入口(按服务端给出的顺序逐个尝试)。
    pub hosts: Vec<LiveHost>,
    /// 认证请求的正文协议版本(恒 [`PROTOVER_BROTLI`];getDanmuInfo 响应
    /// 本身不含该字段,此值决定命令包正文以 brotli 下发)。
    pub protover: u16,
}

/// 操作码:客户端心跳。
pub const OP_HEARTBEAT: u32 = 2;
/// 操作码:心跳回复(正文为人气值)。
pub const OP_HEARTBEAT_REPLY: u32 = 3;
/// 操作码:普通命令包。
pub const OP_MESSAGE: u32 = 5;
/// 操作码:客户端认证。
pub const OP_AUTH: u32 = 7;
/// 操作码:认证回复。
pub const OP_AUTH_REPLY: u32 = 8;

/// 包头大小(字节,文档恒 16)。
pub const HEADER_LEN: usize = 16;
/// 正文协议版本:普通包正文为不压缩 JSON。
pub const PROTOVER_JSON: u16 = 0;
/// 正文协议版本:心跳/认证包正文不压缩(包头用 1)。
pub const PROTOVER_NO_COMPRESS: u16 = 1;
/// 正文协议版本:zlib 压缩。
pub const PROTOVER_ZLIB: u16 = 2;
/// 正文协议版本:brotli 压缩的多个带头普通包(web 端现行,认证请求它)。
pub const PROTOVER_BROTLI: u16 = 3;
/// 心跳发送间隔(文档:约 30 秒一次,否则约 60 秒后断连)。
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// 单包正文长度上限(实测包远小于此;防御畸形包头)。
const MAX_PACKET_LEN: usize = 1024 * 1024;
/// WS 单条消息上限(单包上限 1MiB,放宽到 4MiB 兼顾一帧多包的服务端
/// 批量;相较 tungstenite 默认 64MiB 收紧逾一个量级,防畸形大帧占内存)。
const MAX_WS_MESSAGE: usize = 4 * 1024 * 1024;
/// 压缩正文解压上限(解压炸弹防御)。
const MAX_INFLATED: u64 = 16 * 1024 * 1024;
/// TCP/TLS 连接与写超时。
const DIAL_TIMEOUT: Duration = Duration::from_secs(10);

/// 弹幕会话事件([`LiveSocket::next_event`] 逐条交付)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveEvent {
    Popularity { count: u32 },
    Message(crate::ChatEvent),
    Authenticated,
    Disconnected,
}

/// 包头字段(大端,共 [`HEADER_LEN`] 字节)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    /// 封包总大小(包头 + 正文)。
    pub packet_len: u32,
    /// 包头大小(恒 16)。
    pub header_len: u16,
    /// 正文协议版本。
    pub protover: u16,
    /// 操作码。
    pub operation: u32,
    /// 发包序号(单包语义恒 1,服务端不校验)。
    pub sequence: u32,
}

/// 编码一个完整包:16 字节大端包头 + 正文。
pub fn encode_packet(operation: u32, protover: u16, body: &[u8]) -> Vec<u8> {
    let packet_len = (HEADER_LEN + body.len()) as u32;
    let mut out = Vec::with_capacity(packet_len as usize);
    out.extend_from_slice(&packet_len.to_be_bytes());
    out.extend_from_slice(&(HEADER_LEN as u16).to_be_bytes());
    out.extend_from_slice(&protover.to_be_bytes());
    out.extend_from_slice(&operation.to_be_bytes());
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(body);
    out
}

/// 读取包头;不足 [`HEADER_LEN`] 字节返回 None。
pub fn read_header(buf: &[u8]) -> Option<PacketHeader> {
    let head = buf.first_chunk::<HEADER_LEN>()?;
    Some(PacketHeader {
        packet_len: u32::from_be_bytes(head[0..4].try_into().expect("4 bytes")),
        header_len: u16::from_be_bytes(head[4..6].try_into().expect("2 bytes")),
        protover: u16::from_be_bytes(head[6..8].try_into().expect("2 bytes")),
        operation: u32::from_be_bytes(head[8..12].try_into().expect("4 bytes")),
        sequence: u32::from_be_bytes(head[12..16].try_into().expect("4 bytes")),
    })
}

/// 认证包正文 JSON(文档「认证包正文」字段集;buvid 为 web 端补充字段)。
pub fn auth_body(
    room_id: i64,
    uid: i64,
    protover: u16,
    token: &str,
    buvid: Option<&str>,
) -> String {
    let mut body = serde_json::json!({
        "uid": uid,
        "roomid": room_id,
        "protover": protover,
        "platform": "web",
        "type": 2,
        "key": token,
    });
    if let Some(buvid) = buvid {
        body["buvid"] = buvid.into();
    }
    body.to_string()
}

/// 认证包(OP_AUTH,正文不压缩)。
pub fn auth_packet(info: &DanmakuInfo) -> Vec<u8> {
    let body = auth_body(
        info.room_id,
        info.uid,
        info.protover,
        &info.token,
        info.buvid.as_deref(),
    );
    encode_packet(OP_AUTH, PROTOVER_NO_COMPRESS, body.as_bytes())
}

/// 心跳包(OP_HEARTBEAT,空正文)。
pub fn heartbeat_packet() -> Vec<u8> {
    encode_packet(OP_HEARTBEAT, PROTOVER_NO_COMPRESS, &[])
}

/// 心跳回复正文人气值:前 4 字节大端 u32(文档示例 00 00 09 a2 = 2466)。
pub fn parse_popularity(body: &[u8]) -> Option<u32> {
    body.first_chunk::<4>()
        .map(|head| u32::from_be_bytes(*head))
}

/// 一个从字节流解出的完整包。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Packet {
    protover: u16,
    operation: u32,
    body: Vec<u8>,
}

/// 从缓冲解出所有完整包,返回 (包列表, 已消费字节数);
/// 尾部不足一个完整包时保留在缓冲(消费数不含)。
fn take_packets(buf: &[u8]) -> Result<(Vec<Packet>, usize), ApiError> {
    let mut packets = Vec::new();
    let mut offset = 0;
    while let Some(header) = read_header(&buf[offset..]) {
        let header_len = header.header_len as usize;
        let packet_len = header.packet_len as usize;
        if header_len < HEADER_LEN || packet_len < header_len || packet_len > MAX_PACKET_LEN {
            return Err(ApiError::Parse(format!(
                "弹幕包头非法: packet_len={}, header_len={}",
                header.packet_len, header.header_len
            )));
        }
        let end = offset + packet_len;
        if end > buf.len() {
            break; // 尾包不完整,留给下一帧。
        }
        packets.push(Packet {
            protover: header.protover,
            operation: header.operation,
            body: buf[offset + header_len..end].to_vec(),
        });
        offset = end;
    }
    Ok((packets, offset))
}

/// 按正文协议版本解压:2=zlib、3=brotli;0/1 原样(生产上限 [`MAX_INFLATED`])。
fn inflate(protover: u16, body: &[u8]) -> Result<Vec<u8>, ApiError> {
    inflate_with_limit(protover, body, MAX_INFLATED)
}

/// 解压并探测超限:`take(limit + 1)` 比上限多读 1 字节,读满即说明正文
/// 超过上限。静默截断会让截剩的残包被当作完整包丢弃且无任何信号,故
/// 如实报 Parse 错误——会话作废由调用方重建,与畸形包头口径一致。
fn read_capped(reader: impl Read, limit: u64, what: &str) -> Result<Vec<u8>, ApiError> {
    let mut out = Vec::new();
    let taken = reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut out)
        .map_err(|error| ApiError::Parse(format!("{what} 解包失败: {error}")))?;
    if taken as u64 > limit {
        return Err(ApiError::Parse(format!(
            "{what} 正文解压超过 {limit} 字节上限,残包不可信"
        )));
    }
    Ok(out)
}

/// 带「超限探测」的解压(测试注入小上限用)。
fn inflate_with_limit(protover: u16, body: &[u8], limit: u64) -> Result<Vec<u8>, ApiError> {
    match protover {
        // 不压缩正文的长度已在上游 take_packets 里受 MAX_PACKET_LEN 约束。
        PROTOVER_JSON | PROTOVER_NO_COMPRESS => Ok(body.to_vec()),
        PROTOVER_ZLIB => read_capped(flate2::read::ZlibDecoder::new(body), limit, "zlib"),
        PROTOVER_BROTLI => read_capped(brotli::Decompressor::new(body, 4096), limit, "brotli"),
        other => Err(ApiError::Parse(format!("未知弹幕正文协议版本: {other}"))),
    }
}

/// 单包 → 事件:命令包按 cmd 分发,其余操作码就地消化。
fn packet_events(packet: &Packet) -> Result<Vec<LiveEvent>, ApiError> {
    match packet.operation {
        OP_MESSAGE => message_events(packet.protover, &packet.body),
        OP_HEARTBEAT_REPLY => Ok(parse_popularity(&packet.body)
            .map(|count| vec![LiveEvent::Popularity { count }])
            .unwrap_or_default()),
        // 认证回复:正文 {"code":0} 为成功;非 0 是服务端拒绝(token 失效等),
        // 如实报错。正文异常或缺少 code 时不视为认证成功。
        OP_AUTH_REPLY => {
            let code = serde_json::from_slice::<serde_json::Value>(&packet.body)
                .ok()
                .and_then(|value| value.get("code").and_then(serde_json::Value::as_i64));
            match code {
                Some(0) => Ok(vec![LiveEvent::Authenticated]),
                None => Err(ApiError::Parse("invalid authentication response".into())),
                Some(code) => Err(ApiError::Business {
                    code,
                    message: "弹幕认证被拒绝".into(),
                }),
            }
        }
        // 心跳(2)/认证(7)是上行包,服务端不下发;其余未知操作码(协议演进)
        // 静默忽略,不断事件流。
        _ => Ok(Vec::new()),
    }
}

/// 命令包(OP_MESSAGE)正文 → 事件:压缩正文(2/3)解压后是「多个带头
/// 普通包」,按包头逐包递归;不压缩正文(0/1)是单个 JSON 命令。
fn message_events(protover: u16, body: &[u8]) -> Result<Vec<LiveEvent>, ApiError> {
    let mut events = Vec::new();
    match protover {
        PROTOVER_JSON | PROTOVER_NO_COMPRESS => events.extend(cmd_events(body)),
        PROTOVER_ZLIB | PROTOVER_BROTLI => {
            let raw = inflate(protover, body)?;
            let (packets, _) = take_packets(&raw)?;
            for packet in &packets {
                events.extend(packet_events(packet)?);
            }
        }
        other => return Err(ApiError::Parse(format!("未知弹幕正文协议版本: {other}"))),
    }
    Ok(events)
}

fn cmd_events(body: &[u8]) -> Vec<LiveEvent> {
    serde_json::from_slice(body)
        .ok()
        .and_then(|v| crate::protocol::parse(&v))
        .map(LiveEvent::Message)
        .into_iter()
        .collect()
}

pub struct LiveSocket {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    /// 已读取但尚未凑齐完整包的字节(一个 WS 帧可含多个包,包也可能跨帧)。
    buffer: Vec<u8>,
    /// 已解码待交付的事件。
    pending: VecDeque<LiveEvent>,
    closed: bool,
}

impl LiveSocket {
    /// 连接并认证:按 `info.hosts` 顺序逐个尝试 wss 入口
    /// (TCP+TLS+WS 升级 + 发送认证包),全部失败时报最后一个错误。
    pub fn connect(info: &DanmakuInfo) -> Result<Self, ApiError> {
        let mut last = Err(ApiError::InvalidInput("弹幕入口列表为空".into()));
        for host in &info.hosts {
            last = dial(info, host, "wss");
            if last.is_ok() {
                return last;
            }
        }
        last
    }

    /// 发送心跳包(调用方按 [`HEARTBEAT_INTERVAL`] 周期触发)。
    pub fn heartbeat(&mut self) -> Result<(), ApiError> {
        self.send(heartbeat_packet())
    }

    /// 取下一个事件:`timeout` 内无事件返回 `Ok(None)`。协议层内部消化
    /// 心跳回复、认证回复与压缩解包,只把业务事件交给调用方。
    ///
    /// 注意:`timeout` 是**单次底层读**的上界而非调用整体时界——服务端
    /// 慢速滴流喂字节时可长期不返回,调用方不得把心跳排在本方法返回之后
    /// (见模块头「消费约定」)。
    ///
    /// 返回 [`LiveEvent::Disconnected`] 表示服务端已关闭(断网/踢下线/
    /// 认证失效);之后本会话恒返回 `Ok(None)`,调用方应重建会话。
    pub fn next_event(&mut self, timeout: Duration) -> Result<Option<LiveEvent>, ApiError> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(Some(event));
        }
        if self.closed {
            return Ok(None);
        }
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }
            // 先消化缓冲里已完整的包(可能是之前帧的遗留)。
            let (packets, consumed) = take_packets(&self.buffer)?;
            self.buffer = self.buffer.split_off(consumed);
            for packet in &packets {
                self.pending.extend(packet_events(packet)?);
            }
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }
            self.set_read_timeout(timeout)?;
            match self.socket.read() {
                Ok(Message::Binary(data)) => self.buffer.extend_from_slice(&data),
                Ok(Message::Text(text)) => self.buffer.extend_from_slice(text.as_str().as_bytes()),
                // 服务端 ping:tungstenite 已自动排队 pong,flush 发出。
                Ok(Message::Ping(_) | Message::Pong(_)) => {
                    self.socket
                        .flush()
                        .map_err(|error| ApiError::Network(error.to_string()))?;
                }
                Ok(Message::Close(_)) => {
                    self.closed = true;
                    return Ok(Some(LiveEvent::Disconnected));
                }
                // 读路径不返回 Frame,防御分支。
                Ok(Message::Frame(_)) => {}
                Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                    self.closed = true;
                    return Ok(Some(LiveEvent::Disconnected));
                }
                Err(tungstenite::Error::Io(io))
                    if matches!(
                        io.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    std::thread::sleep(timeout.min(Duration::from_millis(20)));
                    return Ok(None); // Yield so heartbeats and cancellation run independently of frame completion.
                }
                Err(error) => return Err(ApiError::Network(error.to_string())),
            }
        }
    }

    fn send(&mut self, packet: Vec<u8>) -> Result<(), ApiError> {
        let write = self.socket.write(Message::Binary(packet.into()));
        write.map_err(|error| ApiError::Network(error.to_string()))?;
        let flush = self.socket.flush();
        flush.map_err(|error| ApiError::Network(error.to_string()))?;
        Ok(())
    }

    /// 把读超时下钻到底层 TcpStream(rustls 流透传超时语义)。
    fn set_read_timeout(&mut self, timeout: Duration) -> Result<(), ApiError> {
        let tcp = match self.socket.get_mut() {
            MaybeTlsStream::Plain(stream) => stream,
            MaybeTlsStream::Rustls(tls) => &mut tls.sock,
            // non_exhaustive:未知传输形态如实报错(当前 feature 组合不可达)。
            other => {
                return Err(ApiError::Network(format!("未知弹幕传输流: {other:?}")));
            }
        };
        tcp.set_nonblocking(true)
            .map_err(|_| ApiError::Network("chat socket unavailable".into()))?;
        tcp.set_read_timeout(Some(timeout))
            .map_err(|error| ApiError::Network(error.to_string()))
    }
}

/// 单入口连接:DNS → TCP(带连接超时)→ WS 升级(`scheme` 为 wss,测试用
/// ws 直连 mock)→ 发认证包。
fn dial(info: &DanmakuInfo, host: &LiveHost, scheme: &str) -> Result<LiveSocket, ApiError> {
    let url = format!("{scheme}://{}:{}/sub", host.host, host.wss_port);
    let addrs: Vec<_> = (host.host.as_str(), host.wss_port)
        .to_socket_addrs()
        .map_err(|error| ApiError::Network(format!("解析弹幕服务器 {host:?} 失败: {error}")))?
        .collect();
    let mut stream = None;
    let mut last = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, DIAL_TIMEOUT) {
            Ok(tcp) => {
                stream = Some(tcp);
                break;
            }
            Err(error) => last = Some(error),
        }
    }
    let stream = stream
        .ok_or_else(|| ApiError::Network(format!("连接弹幕服务器 {host:?} 失败: {last:?}")))?;
    stream
        .set_nodelay(true)
        .map_err(|error| ApiError::Network(error.to_string()))?;
    stream
        .set_write_timeout(Some(DIAL_TIMEOUT))
        .map_err(|error| ApiError::Network(error.to_string()))?;
    stream
        .set_read_timeout(Some(DIAL_TIMEOUT))
        .map_err(|error| ApiError::Network(error.to_string()))?;
    // 从 URL 构造请求(tungstenite 自动生成 Sec-WebSocket-Key 等必需头),
    // 再补 web 端惯例的 UA/Origin。
    let mut request = url
        .into_client_request()
        .map_err(|error| ApiError::Network(format!("弹幕握手请求构造失败: {error}")))?;
    let headers = request.headers_mut();
    headers.insert("User-Agent", http::HeaderValue::from_static(USER_AGENT));
    headers.insert(
        "Origin",
        http::HeaderValue::from_static("https://live.bilibili.com"),
    );
    // 显式收紧 WS 单条消息上限:tungstenite 默认 64MiB 过宽;单包上限
    // MAX_PACKET_LEN(1MiB),一个 WS 帧通常只装一两个包,放宽到 4MiB
    // 兼顾服务端批量,量级与 MAX_PACKET_LEN 对齐。超限帧由 tungstenite
    // 报 Capacity 错 → Network 错 → 调用方重建会话,与畸形包头口径一致。
    let config = WebSocketConfig::default().max_message_size(Some(MAX_WS_MESSAGE));
    let (socket, _response) = client_tls_with_config(request, stream, Some(config), None)
        .map_err(|error| ApiError::Network(format!("弹幕握手失败: {error}")))?;
    let mut session = LiveSocket {
        socket,
        buffer: Vec::new(),
        pending: VecDeque::new(),
        closed: false,
    };
    // 认证包须在连接后尽快发送(文档:5 秒内)。
    session.send(auth_packet(info))?;
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn loopback_auth_compressed_chat_and_heartbeat() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (tcp, _) = listener.accept().unwrap();
            tcp.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut ws = tungstenite::accept(tcp).unwrap();
            let auth = ws.read().unwrap().into_data();
            assert_eq!(read_header(&auth).unwrap().operation, OP_AUTH);
            let body: serde_json::Value = serde_json::from_slice(&auth[HEADER_LEN..]).unwrap();
            assert_eq!(body["uid"], 7);
            assert_eq!(body["buvid"], "device");
            ws.send(Message::Binary(
                encode_packet(OP_AUTH_REPLY, 1, br#"{"code":0}"#).into(),
            ))
            .unwrap();
            let packet = encode_packet(
                OP_MESSAGE,
                0,
                br#"{"cmd":"DANMU_MSG","info":[[],"hello",[9,"viewer"]]}"#,
            );
            let mut compressed =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            compressed.write_all(&packet).unwrap();
            ws.send(Message::Binary(
                encode_packet(OP_MESSAGE, 2, &compressed.finish().unwrap()).into(),
            ))
            .unwrap();
            let heartbeat = ws.read().unwrap().into_data();
            assert_eq!(read_header(&heartbeat).unwrap().operation, OP_HEARTBEAT);
        });
        let host = LiveHost {
            host: "127.0.0.1".into(),
            wss_port: port,
        };
        let info = DanmakuInfo {
            room_id: 1,
            uid: 7,
            token: "fixture".into(),
            buvid: Some("device".into()),
            hosts: vec![host.clone()],
            protover: 3,
        };
        let mut socket = dial(&info, &host, "ws").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut authenticated = false;
        let mut chat = false;
        while std::time::Instant::now() < deadline && !(authenticated && chat) {
            match socket.next_event(Duration::from_millis(30)).unwrap() {
                Some(LiveEvent::Authenticated) => authenticated = true,
                Some(LiveEvent::Message(crate::ChatEvent::Text {
                    author, content, ..
                })) => {
                    assert_eq!(author.uid, "9");
                    assert_eq!(content, "hello");
                    chat = true;
                }
                _ => {}
            }
        }
        assert!(authenticated && chat);
        socket.heartbeat().unwrap();
        server.join().unwrap();
    }
    #[test]
    fn malformed_auth_and_compression_are_errors() {
        let packet = Packet {
            operation: OP_AUTH_REPLY,
            protover: 1,
            body: br#"{"code":-101}"#.to_vec(),
        };
        assert!(matches!(
            packet_events(&packet),
            Err(ApiError::Business { code: -101, .. })
        ));
        let packet = Packet {
            operation: OP_AUTH_REPLY,
            protover: 1,
            body: b"bad".to_vec(),
        };
        assert!(packet_events(&packet).is_err());
        assert!(message_events(2, b"invalid compressed packet").is_err());
        assert!(message_events(99, b"{}").is_err());
    }
}
