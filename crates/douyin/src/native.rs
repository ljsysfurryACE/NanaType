use crate::{AccountState, ConnectionState, Event, handle::Session, protocol, signing};
use prost::Message;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};
use tungstenite::{
    Message as WsMessage, WebSocket, client::IntoClientRequest, stream::MaybeTlsStream,
};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
const SDK_VERSION: &str = "1.0.15";
// Protocol endpoint/SDK facts checked against MIT jwwsjlm/douyinLive,
// commit 0e506f3cf935beede946296602af2e52d57ecea2, constants.go.
const PUSH_ENDPOINT: &str = "wss://webcast100-ws-web-lf.douyin.com/webcast/im/push/v2/";
type Socket = WebSocket<MaybeTlsStream<TcpStream>>;
fn connect(request: tungstenite::http::Request<()>, limit: usize) -> Result<Socket, String> {
    connect_with(request, limit, &dial)
}
fn dial(host: &str, port: u16) -> Result<TcpStream, String> {
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|_| "transport-connect-failed")?;
    for address in addresses.take(4) {
        if let Ok(socket) = TcpStream::connect_timeout(&address, Duration::from_secs(3)) {
            return Ok(socket);
        }
    }
    Err("transport-connect-failed".into())
}
fn connect_with(
    request: tungstenite::http::Request<()>,
    limit: usize,
    dialer: &dyn Fn(&str, u16) -> Result<TcpStream, String>,
) -> Result<Socket, String> {
    let host = request.uri().host().ok_or("invalid-endpoint")?;
    let port = request
        .uri()
        .port_u16()
        .unwrap_or(if request.uri().scheme_str() == Some("wss") {
            443
        } else {
            80
        });
    let stream = dialer(host, port)?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let config = tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(limit))
        .max_frame_size(Some(limit));
    let (socket, _) = tungstenite::client_tls_with_config(request, stream, Some(config), None)
        .map_err(|_| "transport-connect-failed")?;
    timeout(&socket);
    Ok(socket)
}
fn timeout(socket: &Socket) {
    let stream = match socket.get_ref() {
        MaybeTlsStream::Plain(stream) => stream,
        MaybeTlsStream::Rustls(stream) => &stream.sock,
        _ => return,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
}
fn idle(error: &tungstenite::Error) -> bool {
    matches!(error, tungstenite::Error::Io(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut))
}

pub(crate) struct Client {
    agent: ureq::Agent,
    cookies: BTreeMap<String, String>,
    #[cfg(test)]
    dialer: Option<Box<TestDialer>>,
}
#[cfg(test)]
type TestDialer = dyn Fn(&str, u16) -> Result<TcpStream, String> + Send;
impl Client {
    pub fn new(cookie: &str) -> Result<Self, String> {
        if cookie.len() > 65536 || cookie.contains(['\r', '\n']) {
            return Err("invalid-cookie".into());
        }
        let mut cookies = BTreeMap::new();
        for part in cookie.split(';') {
            if let Some((key, value)) = part.trim().split_once('=') {
                if key.is_empty() || !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
                    return Err("invalid-cookie".into());
                }
                cookies.insert(key.into(), value.into());
            }
        }
        Ok(Self {
            agent: ureq::Agent::config_builder()
                .max_redirects(0)
                .timeout_global(Some(Duration::from_secs(5)))
                .build()
                .into(),
            cookies,
            #[cfg(test)]
            dialer: None,
        })
    }
    fn cookie(&self) -> String {
        self.cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
    fn get(&mut self, url: &str) -> Result<Vec<u8>, String> {
        let mut response = self
            .agent
            .get(url)
            .header("User-Agent", USER_AGENT)
            .header("Referer", "https://live.douyin.com/")
            .header("Accept", "*/*")
            .header("Accept-Encoding", "identity")
            .header("Cookie", &self.cookie())
            .call()
            .map_err(|_| "native-http-failed")?;
        for value in response
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|h| h.to_str().ok())
        {
            if let Some((name, value)) = value.split(';').next().and_then(|v| v.split_once('=')) {
                self.cookies.insert(name.into(), value.into());
            }
        }
        response
            .body_mut()
            .with_config()
            .limit(4 * 1024 * 1024)
            .read_to_vec()
            .map_err(|_| "native-http-failed".into())
    }
    fn signed(&mut self, path: &str, pairs: &[(&str, &str)]) -> Result<Vec<u8>, String> {
        let mut url = url::Url::parse(&format!("https://live.douyin.com{path}"))
            .map_err(|_| "invalid-endpoint")?;
        url.query_pairs_mut().extend_pairs(pairs);
        let sign = signing::http(url.query().unwrap_or_default(), USER_AGENT, now());
        url.query_pairs_mut().append_pair("a_bogus", &sign);
        self.get(url.as_str())
    }
    pub fn validate(&mut self) -> Result<AccountState, String> {
        if !["sessionid", "sessionid_ss"].iter().any(|name| {
            self.cookies
                .get(*name)
                .is_some_and(|value| !value.is_empty())
        }) {
            return Ok(AccountState::Expired);
        }
        let bytes = self.signed(
            "/webcast/user/me/",
            &[
                ("aid", "6383"),
                ("app_name", "douyin_web"),
                ("device_platform", "web"),
                ("live_id", "1"),
            ],
        )?;
        let json: Value = serde_json::from_slice(&bytes).map_err(|_| "account-unverified")?;
        Ok(account(&json).unwrap_or(AccountState::Expired))
    }
    fn connect(&mut self, room: &str) -> Result<Socket, String> {
        self.get("https://live.douyin.com/")?;
        let bytes = self.signed(
            "/webcast/room/web/enter/",
            &[
                ("aid", "6383"),
                ("app_name", "douyin_web"),
                ("live_id", "1"),
                ("device_platform", "web"),
                ("web_rid", room),
            ],
        )?;
        let data: Value = serde_json::from_slice(&bytes).map_err(|_| "native-room-failed")?;
        let entry = data
            .pointer("/data/data/0")
            .or_else(|| data.pointer("/data/room"))
            .ok_or("native-room-failed")?;
        if entry["status"].as_u64().is_some_and(|v| v != 2) {
            return Err("room-offline".into());
        }
        let room_id = value_id(&entry["id_str"])
            .or_else(|| value_id(&entry["id"]))
            .ok_or("native-room-failed")?;
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).map_err(|_| "random-unavailable")?;
        let unique = data
            .pointer("/data/user_unique_id")
            .and_then(value_id)
            .unwrap_or_else(|| (u64::from_le_bytes(random) & 0x7fff_ffff_ffff_ffff).to_string());
        let browser_version = USER_AGENT.strip_prefix("Mozilla/").unwrap_or(USER_AGENT);
        let mut token_bytes = [0u8; 128];
        getrandom::fill(&mut token_bytes).map_err(|_| "random-unavailable")?;
        use base64::Engine;
        let ms_token = self
            .cookies
            .get("msToken")
            .cloned()
            .unwrap_or_else(|| base64::engine::general_purpose::URL_SAFE.encode(token_bytes));
        let common = [
            ("aid", "6383"),
            ("app_name", "douyin_web"),
            ("version_code", "180800"),
            ("device_platform", "web"),
            ("live_id", "1"),
            ("identity", "audience"),
            ("room_id", room_id.as_str()),
            ("user_unique_id", unique.as_str()),
            ("resp_content_type", "protobuf"),
            ("browser_version", browser_version),
            ("browser_name", "Mozilla"),
            ("browser_platform", "Win32"),
            ("did_rule", "3"),
            ("endpoint", "live_pc"),
            ("support_wrds", "1"),
            ("cookie_enabled", "true"),
            ("screen_width", "1920"),
            ("screen_height", "1080"),
            ("browser_language", "zh-CN"),
            ("browser_online", "true"),
            ("tz_name", "Asia/Shanghai"),
            ("need_persist_msg_count", "15"),
            ("insert_task_id", ""),
            ("live_reason", ""),
        ];
        let mut fetch_pairs = common.to_vec();
        fetch_pairs.extend([
            ("device_id", ""),
            ("last_rtt", "0"),
            ("fetch_rule", "1"),
            ("cursor", ""),
            ("internal_ext", ""),
            ("msToken", ms_token.as_str()),
        ]);
        let initial = self.signed("/webcast/im/fetch/", &fetch_pairs)?;
        let response = protocol::Response::decode(initial.as_slice())
            .map_err(|_| "native-bootstrap-decode-failed")?;
        // An empty initial HTTP batch is allowed. Start without a history cursor;
        // the signed push connection supplies its own fresh state. Never reuse a
        // hard-coded cursor or fabricate server timestamps.
        let push_server = [
            &response.push_server_v2,
            &response.push_server,
            &response.proxy_server,
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .map_or(PUSH_ENDPOINT, |s| s.as_str());
        let mut endpoint = url::Url::parse(push_server).map_err(|_| "native-bootstrap-failed")?;
        if endpoint.scheme() != "wss"
            || !endpoint
                .host_str()
                .is_some_and(|h| h.ends_with(".douyin.com"))
        {
            return Err("invalid-endpoint".into());
        }
        let stub = signing::stub(&format!(
            "live_id=1,aid=6383,version_code=180800,webcast_sdk_version={SDK_VERSION},room_id={room_id},sub_room_id=,sub_channel_id=,did_rule=3,user_unique_id={unique},device_platform=web,device_type=,ac=,identity=audience"
        ));
        endpoint
            .query_pairs_mut()
            .extend_pairs(common)
            .extend_pairs([
                ("webcast_sdk_version", SDK_VERSION),
                ("update_version_code", SDK_VERSION),
                ("compress", "gzip"),
                ("host", "https://live.douyin.com"),
                ("im_path", "/webcast/im/fetch/"),
                ("heartbeatDuration", "0"),
            ])
            .append_pair("cursor", &response.cursor)
            .append_pair("internal_ext", &response.internal_ext)
            .append_pair(
                "signature",
                &signing::websocket(
                    stub,
                    1,
                    random[0] & 1 == 1,
                    random[1] % 255,
                    random[2] % 255,
                ),
            );
        let mut request = endpoint
            .as_str()
            .into_client_request()
            .map_err(|_| "native-connect-failed")?;
        for (name, value) in [
            ("User-Agent", USER_AGENT.to_owned()),
            ("Origin", "https://live.douyin.com".into()),
            ("Cookie", self.cookie()),
        ] {
            request.headers_mut().insert(
                tungstenite::http::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| "invalid-header")?,
                value.parse().map_err(|_| "invalid-cookie")?,
            );
        }
        #[cfg(test)]
        if let Some(dialer) = &self.dialer {
            return connect_with(request, protocol::MAX_FRAME, dialer.as_ref())
                .map_err(|_| "native-connect-failed".into());
        }
        connect(request, protocol::MAX_FRAME).map_err(|_| "native-connect-failed".into())
    }
}
fn value_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && *s != "0")
        .map(str::to_owned)
        .or_else(|| value.as_u64().filter(|n| *n != 0).map(|n| n.to_string()))
}
fn account(value: &Value) -> Option<AccountState> {
    if value["status_code"].as_i64() != Some(0) {
        return None;
    }
    let user = value.pointer("/data/user").unwrap_or(&value["data"]);
    if user["is_login"] == false || value["data"]["is_login"] == false {
        return None;
    }
    let uid = value_id(&user["id_str"])
        .or_else(|| value_id(&user["id"]))
        .or_else(|| value_id(&user["sec_uid"]))?;
    let name = user["nickname"].as_str()?.to_owned();
    if name.is_empty() {
        return None;
    }
    Some(AccountState::Ready { uid, name })
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn run(session: &Session, cookie: &str) -> Result<(), String> {
    let mut client = Client::new(cookie)?;
    session.emit(Event::Account(if cookie.is_empty() {
        AccountState::SignedOut
    } else {
        AccountState::Unverified
    }));
    if !cookie.is_empty() {
        session.emit(Event::Account(
            client.validate().unwrap_or(AccountState::Unverified),
        ));
    }
    receive(session, client)
}
fn reconnect_delay(failures: u32) -> Duration {
    Duration::from_secs((1u64 << failures.saturating_sub(1).min(5)).min(30))
}
fn receive(session: &Session, mut client: Client) -> Result<(), String> {
    let mut decoder = protocol::Decoder::default();
    let mut failures = 0u32;
    let mut connected_once = false;
    loop {
        if !session.current() {
            return Ok(());
        }
        session.emit(Event::Connection(ConnectionState::Connecting));
        let mut socket = match client.connect(&session.room) {
            Ok(socket) => socket,
            Err(error) => {
                failures = failures.saturating_add(1);
                if !connected_once && failures >= 4 {
                    return Err(error);
                }
                if !session.pause(reconnect_delay(failures)) {
                    return Ok(());
                }
                continue;
            }
        };
        let mut heartbeat = Instant::now();
        let mut heartbeat_interval = Duration::from_secs(10);
        let mut last_frame = Instant::now();
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut received = false;
        while session.current() {
            if heartbeat.elapsed() >= heartbeat_interval {
                let frame = protocol::PushFrame {
                    kind: "hb".into(),
                    ..Default::default()
                }
                .encode_to_vec();
                if socket.send(WsMessage::Binary(frame.into())).is_err() {
                    break;
                }
                heartbeat = Instant::now();
            }
            match socket.read() {
                Ok(WsMessage::Binary(bytes)) => {
                    if let Ok(packet) = protocol::decode(&bytes) {
                        last_frame = Instant::now();
                        if packet.response.heartbeat_seconds > 0 {
                            heartbeat_interval =
                                Duration::from_secs(packet.response.heartbeat_seconds.clamp(5, 60));
                        }
                        if let Some(ack) = packet.ack
                            && socket.send(WsMessage::Binary(ack.into())).is_err()
                        {
                            break;
                        }
                        let messages = decoder.messages(&packet.response);
                        if !received && !messages.is_empty() {
                            received = true;
                            connected_once = true;
                            failures = 0;
                            session.emit(Event::Connection(ConnectionState::Connected {
                                room: session.room.clone(),
                            }));
                        }
                        for event in messages {
                            session.emit(Event::Chat {
                                room: session.room.clone(),
                                event,
                            });
                        }
                    }
                }
                Ok(WsMessage::Close(_)) => break,
                Err(e) if idle(&e) => {}
                Err(_) => break,
                _ => {}
            }
            if !received && Instant::now() >= deadline {
                if !connected_once {
                    return Err("native-no-messages".into());
                }
                break;
            }
            if last_frame.elapsed() >= Duration::from_secs(70) {
                break;
            }
        }
        let _ = socket.close(None);
        if !session.current() {
            return Ok(());
        }
        if !received && !connected_once {
            return Err("native-no-messages".into());
        }
        session.emit(Event::Connection(ConnectionState::Connecting));
        failures = failures.saturating_add(1);
        if !session.pause(reconnect_delay(failures)) {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    mod live;
    use super::*;

    #[test]
    fn reconnect_backoff_is_bounded_even_after_a_long_outage() {
        assert_eq!(reconnect_delay(0), Duration::from_secs(1));
        assert_eq!(reconnect_delay(1), Duration::from_secs(1));
        assert_eq!(reconnect_delay(5), Duration::from_secs(16));
        assert_eq!(reconnect_delay(u32::MAX), Duration::from_secs(30));
    }

    #[test]
    fn cookies_alone_do_not_prove_account_identity() {
        assert!(account(&serde_json::json!({"status_code":0,"data":{}})).is_none());
        assert!(Client::new("sessionid=test\r\nInjected: x").is_err());
        assert!(matches!(
            account(
                &serde_json::json!({"status_code":0,"data":{"id_str":"42","nickname":"viewer"}})
            ),
            Some(AccountState::Ready { .. })
        ));
    }
}
