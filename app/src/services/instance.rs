//! One NanaType per user session. The first instance listens on a loopback
//! port; a second launch connects, asks it to come forward, and exits.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

const PORT: u16 = 41_891;
const HELLO: &[u8] = b"nanatype:show\n";

pub enum Instance {
    /// We are the first; `listener` reports later launches.
    Primary(TcpListener),
    /// Another instance is running and was told to show itself.
    Secondary,
}

pub fn acquire() -> Instance {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, PORT));
    match TcpListener::bind(addr) {
        Ok(listener) => Instance::Primary(listener),
        Err(_) => {
            if let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
                let _ = stream.write_all(HELLO);
                Instance::Secondary
            } else {
                // The port is taken by something else; run anyway.
                match TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))) {
                    Ok(l) => Instance::Primary(l),
                    Err(_) => Instance::Secondary,
                }
            }
        }
    }
}

/// Call `show` whenever another launch knocks.
pub fn listen(listener: TcpListener, show: impl Fn() + Send + 'static) {
    thread::Builder::new()
        .name("nanatype-instance".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let mut buf = [0u8; 32];
                if let Ok(n) = stream.read(&mut buf)
                    && buf[..n].starts_with(b"nanatype:show")
                {
                    show();
                }
            }
        })
        .expect("spawn instance listener");
}
