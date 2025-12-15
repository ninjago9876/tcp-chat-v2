// ---------------------
// \ Networking Module \
// ---------------------
//
// Responsibilities:
//  - Listen for incoming tcp connections
//  - Maintain a HashMap<PeerId, Peer> (local to the connection manager)
//  - Communicate with the TUI / main Thread using typed (UI2Net, Net2UI) packets
//      - UI2Net examples:
//          SendMessage (Sends message to specific peer)
//          Disconnect (Disconnects for network without shutting down thread)
//          Connect (Connects to a node)
//      - Net2UI examples:
//          MessageReceived (On message received)
//          PeerDisconnected (On Peer disconnected)
//          PeerConnected (On Peer connected)
//          Disconnected (On loss of connection)
//          Connected (On connection aquisition)
//
// Architecture:
//  No extra Threads per Peer are used, all Peers are handled asynchronously in the
//      ConnectionManager
//  Single Tokio ConnectionManager task
//      - Listens to incoming connections
//      - Has ownership of Peer Data (e.g. HashMap<PeerID, Peer>)
//      - Uses tokio::select! for concurrency
//      - Sends and Responds to Main thread
//  A P2PPacket enum detailing layout of packets sent between peers

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::{
    io::WriteHalf,
    net::TcpStream,
    runtime::Runtime,
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
};
use uuid::Uuid;

use crate::{net::connection_manager::run_connection_manager, Net2UIPacket, UI2NetPacket};

pub mod connection_manager;

type PeerId = Uuid;

struct Peer {
    stream: WriteHalf<TcpStream>,
    id: PeerId,
}

struct PeerInfo {
    uid: PeerId,
    display_name: String,
    addr: SocketAddr,
}

#[derive(Deserialize, Serialize, Debug)]
enum P2PPacket {
    SendMessage { msg: String },
    Connect { uid: PeerId }, // Has to be first packet sent.
    Disconnect,
}

pub fn init(ui2net_rx: UnboundedReceiver<UI2NetPacket>, net2ui_tx: UnboundedSender<Net2UIPacket>) {
    let rt = Runtime::new().unwrap();

    rt.block_on(async move {
        run_connection_manager(8080, ui2net_rx, net2ui_tx).await;
    });
}
