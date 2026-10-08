//! Localhost WebSocket broadcast. Every event goes to every client as JSON:
//! `{"type":"expression","name":"smile","intensity":null}`.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};
use tungstenite::{Message, WebSocket};

use crate::{BridgeEvent, BridgeStatus, Sink, StatusFn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsConfig {
    pub port: u16,
}

pub(crate) struct WsServer {
    config: WsConfig,
    tx: Sender<String>,
    _stop: Sender<()>,
}

impl WsServer {
    pub fn start(config: WsConfig, status: StatusFn) -> Self {
        let (tx, rx) = unbounded::<String>();
        let (stop_tx, stop_rx) = unbounded::<()>();
        let port = config.port;
        thread::Builder::new()
            .name("nanatype-ws".into())
            .spawn(move || run(port, rx, stop_rx, status))
            .expect("spawn ws thread");
        Self {
            config,
            tx,
            _stop: stop_tx,
        }
    }

    pub fn config(&self) -> &WsConfig {
        &self.config
    }

    pub fn emit(&self, event: &BridgeEvent) {
        if let Ok(json) = serde_json::to_string(event) {
            let _ = self.tx.send(json);
        }
    }
}

fn run(port: u16, rx: Receiver<String>, stop: Receiver<()>, status: StatusFn) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match TcpListener::bind(addr) {
        Ok(l) => l,
        Err(e) => {
            status(BridgeStatus::Error(
                Sink::WebSocket,
                format!("port {port}: {e}"),
            ));
            return;
        }
    };
    let _ = listener.set_nonblocking(true);
    status(BridgeStatus::Connected(Sink::WebSocket));
    let mut clients: Vec<WebSocket<TcpStream>> = Vec::new();
    let mut reported = usize::MAX;
    loop {
        if stop.try_recv() != Err(crossbeam_channel::TryRecvError::Empty) {
            for mut c in clients {
                let _ = c.close(None);
            }
            return;
        }
        while let Ok((stream, _)) = listener.accept() {
            let _ = stream.set_nonblocking(false);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            if let Ok(ws) = tungstenite::accept(stream) {
                let _ = ws
                    .get_ref()
                    .set_read_timeout(Some(Duration::from_millis(1)));
                clients.push(ws);
            }
        }
        if let Ok(first) = rx.recv_timeout(Duration::from_millis(50)) {
            let mut batch = vec![first];
            batch.extend(rx.try_iter());
            clients.retain_mut(|c| {
                batch
                    .iter()
                    .all(|m| c.send(Message::text(m.clone())).is_ok())
            });
        }
        // Read (and discard) client frames so pings are answered and closes noticed.
        clients.retain_mut(|c| match c.read() {
            Ok(Message::Close(_)) => false,
            Ok(_) => true,
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                true
            }
            Err(_) => false,
        });
        if clients.len() != reported {
            reported = clients.len();
            status(BridgeStatus::Clients(reported));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn clients_receive_broadcast_json() {
        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let server = WsServer::start(WsConfig { port }, Arc::new(|_| {}));
        let mut client = None;
        for _ in 0..50 {
            if let Ok(stream) = TcpStream::connect(("127.0.0.1", port))
                && let Ok((ws, _)) = tungstenite::client(format!("ws://127.0.0.1:{port}"), stream)
            {
                client = Some(ws);
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let mut client = client.expect("server accepts");
        client
            .get_ref()
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        // The server may not have registered the client yet; send until it arrives.
        let mut got = None;
        for _ in 0..30 {
            server.emit(&BridgeEvent::Speaking { speaking: true });
            match client.read() {
                Ok(Message::Text(t)) => {
                    got = Some(t.to_string());
                    break;
                }
                _ => thread::sleep(Duration::from_millis(20)),
            }
        }
        let text = got.expect("event delivered");
        assert_eq!(text, r#"{"type":"speaking","speaking":true}"#);
    }
}
