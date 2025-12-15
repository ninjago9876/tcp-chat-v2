mod net;

use chrono::Local;
use std::{
    fs,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    str::FromStr,
    vec,
};
use tokio::sync::{mpsc, oneshot};
use tracing::info;
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use uuid::Uuid;

struct PeerInfo {
    uid: Uuid,
    display_name: String,
    addr: SocketAddr,
}

enum UI2NetPacket {
    SendMessage {
        msg: String,
    },
    Disconnect,
    Connect {
        addr: SocketAddr,
    },
    GetPeerList {
        uid: Uuid,
        reply: oneshot::Sender<Vec<PeerInfo>>,
    },
}

enum Net2UIPacket {
    MessageReceived { msg: String },
    PeerDisconnected { uid: Uuid },
    PeerConnected { uid: Uuid },
    Disconnected,
    Connected,
}

fn main() {
    if std::mem::size_of::<usize>() != 8 {
        eprintln!(
            "Warning: this program is intended for 64-bit platforms. \
             32-bit platforms are unsupported."
        );
        return;
    }

    init_logging();

    let (ui2net_tx, ui2net_rx) = tokio::sync::mpsc::unbounded_channel::<UI2NetPacket>();
    let (net2ui_tx, net2ui_rx) = tokio::sync::mpsc::unbounded_channel::<Net2UIPacket>();

    std::thread::spawn(move || {
        net::init(ui2net_rx, net2ui_tx);
    });

    loop {
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() {
            continue;
        }
        match input.trim().split_once(" ") {
            Some(("exit", _)) => {
                tracing::info!("Shutting down");
                break;
            }
            Some(("connect", addr)) => ui2net_tx
                .send(UI2NetPacket::Connect {
                    addr: SocketAddr::from_str(addr).unwrap(),
                })
                .unwrap(),
            Some(_) => ui2net_tx
                .send(UI2NetPacket::SendMessage { msg: input })
                .unwrap(),
            None => {}
        }
    }
}

fn init_logging() {
    // Make sure logs/ folder exists
    fs::create_dir_all("logs").unwrap();

    let latest_path = "logs/latest.log";

    // 1️⃣ Archive old latest.log if it exists
    if fs::metadata(latest_path).is_ok() {
        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
        let archive_path = format!("logs/run_{}.log", timestamp);
        fs::copy(latest_path, &archive_path).unwrap();
    }

    // 2️⃣ Clear latest.log so we can write fresh logs
    fs::write(latest_path, "").unwrap();

    // 3️⃣ Create non-blocking writer to latest.log
    let file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(latest_path)
        .unwrap();
    let (non_blocking, _guard) = non_blocking(file);

    // Keep guard alive for program duration
    Box::leak(Box::new(_guard));

    // 4️⃣ Initialize tracing subscriber
    fmt::Subscriber::builder()
        .with_writer(non_blocking)
        .with_ansi(false) // no colors in file
        .init();

    info!("Logging started, writing to {}", latest_path);
}
