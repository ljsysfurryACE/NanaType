//! VTube Studio public API client (`ws://127.0.0.1:<port>`).
//!
//! Flow: connect → authenticate with the stored token, or request one (VTube
//! Studio shows a confirmation popup) → list the current model's hotkeys →
//! trigger hotkeys for expression markers. Reconnects every few seconds while
//! VTube Studio is closed.

use std::collections::BTreeMap;
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

use crate::{BridgeEvent, BridgeStatus, Sink, StatusFn};

pub const PLUGIN_NAME: &str = "NanaType";
pub const PLUGIN_DEVELOPER: &str = "sena-nana";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VtsConfig {
    pub port: u16,
    pub token: Option<String>,
    /// Expression name → hotkey id. Unmapped names match a hotkey by name.
    pub expressions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VtsHotkey {
    pub id: String,
    pub name: String,
    pub kind: String,
}

enum Command {
    Event(BridgeEvent),
    RequestToken,
    RefreshHotkeys,
}

pub(crate) struct VtsClient {
    config: VtsConfig,
    tx: Sender<Command>,
    _stop: Sender<()>,
}

pub(crate) fn request(kind: &str, id: &str, data: Value) -> String {
    json!({
        "apiName": "VTubeStudioPublicAPI",
        "apiVersion": "1.0",
        "requestID": id,
        "messageType": kind,
        "data": data,
    })
    .to_string()
}

/// Pick the hotkey for an expression: explicit mapping first, then a hotkey
/// whose name equals the expression (ignoring case).
pub(crate) fn resolve_hotkey(
    name: &str,
    map: &BTreeMap<String, String>,
    hotkeys: &[VtsHotkey],
) -> Option<String> {
    if let Some(id) = map.get(name) {
        return Some(id.clone());
    }
    hotkeys
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.id.clone())
}

struct Session {
    socket: WebSocket<TcpStream>,
    authenticated: bool,
    hotkeys: Vec<VtsHotkey>,
}

impl VtsClient {
    pub fn start(config: VtsConfig, status: StatusFn) -> Self {
        let (tx, rx) = unbounded();
        let (stop_tx, stop_rx) = unbounded::<()>();
        let cfg = config.clone();
        thread::Builder::new()
            .name("nanatype-vts".into())
            .spawn(move || run(cfg, rx, stop_rx, status))
            .expect("spawn vts thread");
        Self {
            config,
            tx,
            _stop: stop_tx,
        }
    }

    pub fn config(&self) -> &VtsConfig {
        &self.config
    }

    pub fn emit(&self, event: &BridgeEvent) {
        if matches!(event, BridgeEvent::Expression { .. }) {
            let _ = self.tx.send(Command::Event(event.clone()));
        }
    }

    pub fn request_token(&self) {
        let _ = self.tx.send(Command::RequestToken);
    }

    pub fn refresh_hotkeys(&self) {
        let _ = self.tx.send(Command::RefreshHotkeys);
    }
}

fn connect(port: u16) -> Result<WebSocket<TcpStream>, String> {
    let addr = format!("127.0.0.1:{port}");
    let stream = TcpStream::connect_timeout(
        &addr
            .parse()
            .map_err(|e: std::net::AddrParseError| e.to_string())?,
        Duration::from_secs(2),
    )
    .map_err(|e| e.to_string())?;
    let (socket, _) =
        tungstenite::client(format!("ws://{addr}"), stream).map_err(|e| e.to_string())?;
    socket
        .get_ref()
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| e.to_string())?;
    Ok(socket)
}

fn run(config: VtsConfig, rx: Receiver<Command>, stop: Receiver<()>, status: StatusFn) {
    let mut token = config.token.clone();
    let mut session: Option<Session> = None;
    let mut next_attempt = Instant::now();
    let mut counter = 0u64;
    let mut next_id = move || {
        counter += 1;
        format!("nt-{counter}")
    };
    loop {
        if stop.try_recv() != Err(crossbeam_channel::TryRecvError::Empty) {
            return;
        }
        if session.is_none() {
            if Instant::now() < next_attempt {
                // Drop events while disconnected; keep control requests short.
                if let Ok(Command::RequestToken | Command::RefreshHotkeys) =
                    rx.recv_timeout(Duration::from_millis(200))
                {
                    next_attempt = Instant::now();
                }
                continue;
            }
            status(BridgeStatus::Connecting(Sink::Vts));
            match connect(config.port) {
                Ok(mut socket) => {
                    if let Some(t) = &token {
                        let _ = socket.send(Message::text(request(
                            "AuthenticationRequest",
                            &next_id(),
                            json!({
                                "pluginName": PLUGIN_NAME,
                                "pluginDeveloper": PLUGIN_DEVELOPER,
                                "authenticationToken": t,
                            }),
                        )));
                    } else {
                        status(BridgeStatus::VtsNeedsToken);
                    }
                    session = Some(Session {
                        socket,
                        authenticated: false,
                        hotkeys: Vec::new(),
                    });
                }
                Err(e) => {
                    status(BridgeStatus::Error(Sink::Vts, e));
                    next_attempt = Instant::now() + Duration::from_secs(5);
                    continue;
                }
            }
        }
        let s = session.as_mut().expect("connected");
        while let Ok(cmd) = rx.try_recv() {
            let msg = match cmd {
                Command::RequestToken => Some(request(
                    "AuthenticationTokenRequest",
                    &next_id(),
                    json!({ "pluginName": PLUGIN_NAME, "pluginDeveloper": PLUGIN_DEVELOPER }),
                )),
                Command::RefreshHotkeys if s.authenticated => Some(request(
                    "HotkeysInCurrentModelRequest",
                    &next_id(),
                    json!({}),
                )),
                Command::Event(BridgeEvent::Expression { name, .. }) if s.authenticated => {
                    resolve_hotkey(&name, &config.expressions, &s.hotkeys).map(|id| {
                        request(
                            "HotkeyTriggerRequest",
                            &next_id(),
                            json!({ "hotkeyID": id }),
                        )
                    })
                }
                _ => None,
            };
            if let Some(m) = msg
                && s.socket.send(Message::text(m)).is_err()
            {
                session = None;
                break;
            }
        }
        let Some(s) = session.as_mut() else {
            continue;
        };
        match s.socket.read() {
            Ok(Message::Text(text)) => {
                let Ok(v) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                let data = &v["data"];
                match v["messageType"].as_str().unwrap_or_default() {
                    "AuthenticationTokenResponse" => {
                        if let Some(t) = data["authenticationToken"].as_str() {
                            token = Some(t.to_owned());
                            status(BridgeStatus::VtsToken(t.to_owned()));
                            let _ = s.socket.send(Message::text(request(
                                "AuthenticationRequest",
                                &next_id(),
                                json!({
                                    "pluginName": PLUGIN_NAME,
                                    "pluginDeveloper": PLUGIN_DEVELOPER,
                                    "authenticationToken": t,
                                }),
                            )));
                        }
                    }
                    "AuthenticationResponse" => {
                        s.authenticated = data["authenticated"].as_bool().unwrap_or(false);
                        if s.authenticated {
                            status(BridgeStatus::Connected(Sink::Vts));
                            let _ = s.socket.send(Message::text(request(
                                "HotkeysInCurrentModelRequest",
                                &next_id(),
                                json!({}),
                            )));
                        } else {
                            token = None;
                            status(BridgeStatus::VtsNeedsToken);
                        }
                    }
                    "HotkeysInCurrentModelResponse" => {
                        s.hotkeys = data["availableHotkeys"]
                            .as_array()
                            .map(|list| {
                                list.iter()
                                    .map(|h| VtsHotkey {
                                        id: h["hotkeyID"].as_str().unwrap_or_default().into(),
                                        name: h["name"].as_str().unwrap_or_default().into(),
                                        kind: h["type"].as_str().unwrap_or_default().into(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        status(BridgeStatus::VtsHotkeys(s.hotkeys.clone()));
                    }
                    "ModelLoadedEvent" => {
                        let _ = s.socket.send(Message::text(request(
                            "HotkeysInCurrentModelRequest",
                            &next_id(),
                            json!({}),
                        )));
                    }
                    "APIError" => {
                        let msg = data["message"].as_str().unwrap_or("API error").to_owned();
                        status(BridgeStatus::Error(Sink::Vts, msg));
                    }
                    _ => {}
                }
            }
            Ok(Message::Close(_)) => {
                session = None;
                status(BridgeStatus::Error(Sink::Vts, "VTube Studio closed".into()));
                next_attempt = Instant::now() + Duration::from_secs(3);
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => {
                session = None;
                status(BridgeStatus::Error(Sink::Vts, e.to_string()));
                next_attempt = Instant::now() + Duration::from_secs(3);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_carry_the_api_envelope() {
        let r: Value = serde_json::from_str(&request(
            "HotkeyTriggerRequest",
            "1",
            json!({"hotkeyID": "h"}),
        ))
        .unwrap();
        assert_eq!(r["apiName"], "VTubeStudioPublicAPI");
        assert_eq!(r["messageType"], "HotkeyTriggerRequest");
        assert_eq!(r["data"]["hotkeyID"], "h");
    }

    #[test]
    fn hotkeys_resolve_by_map_then_by_name() {
        let keys = vec![VtsHotkey {
            id: "abc".into(),
            name: "Smile".into(),
            kind: "ToggleExpression".into(),
        }];
        let mut map = BTreeMap::new();
        assert_eq!(resolve_hotkey("smile", &map, &keys), Some("abc".into()));
        map.insert("smile".into(), "override".into());
        assert_eq!(
            resolve_hotkey("smile", &map, &keys),
            Some("override".into())
        );
        assert_eq!(resolve_hotkey("cry", &map, &keys), None);
    }
}
