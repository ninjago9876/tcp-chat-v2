use std::{collections::HashMap, io::Error};

use serde::{Deserialize, Serialize};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt, WriteHalf},
    net::{TcpListener, TcpStream},
    select,
    sync::{mpsc, oneshot},
};
use uuid::Uuid;

use crate::{
    net::{P2PPacket, Peer, PeerId},
    Net2UIPacket, PeerInfo, UI2NetPacket,
};

fn send_packet<T>(packet: T, stream: &mut WriteHalf<TcpStream>)
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    let mut buf = Vec::new();
    bincode::serde::encode_into_slice(packet, &mut buf, bincode::config::standard());
    let len_buf = (buf.len() as u64).to_be_bytes();
    stream.write_all(&len_buf);
    stream.write_all(buf.as_slice());
}

struct ConnectionManager {
    peers: HashMap<PeerId, Peer>,
    my_uid: PeerId,
    port: u16,
    ui2net_rx: mpsc::Receiver<UI2NetPacket>,
    net2ui_tx: mpsc::Sender<Net2UIPacket>,
}

impl ConnectionManager {
    async fn handle_connection(&mut self, stream: TcpStream) -> Result<(), Error> {
        let mut id = Uuid::nil();

        let (mut read_half, mut write_half) = split(stream);

        loop {
            let mut len: [u8; 8] = [0u8; 8];
            read_half.read_exact(&mut len);

            // let buf = [0u8; usize::from_be_bytes(LEN)];
            let mut buf = vec![0u8; usize::from_be_bytes(len)];
            read_half.read_exact(&mut buf);
            let packet =
                bincode::serde::decode_from_slice(buf.as_slice(), bincode::config::standard());

            if packet.is_err() {
                return Ok(());
            }
            let packet = packet.unwrap().0;

            match packet {
                P2PPacket::SendMessage { msg } => {
                    tracing::info!("Received message: \"{}\" from peer with UUID: {}", msg, id);
                }
                P2PPacket::Connect { uid } => {
                    if !self.peers.contains_key(&id) {
                        id = uid;
                        send_packet(P2PPacket::Connect { uid: self.my_uid }, &mut write_half);
                        let peer = Peer {
                            stream: write_half,
                            id,
                        };
                        self.peers.insert(id, peer);
                    }
                }
                P2PPacket::Disconnect => {
                    send_packet(P2PPacket::Disconnect, &mut write_half);
                    self.peers.remove(&id);
                    return Ok(());
                }
            }
        }
    }
    async fn handle_ui2net_packet(&mut self, packet: UI2NetPacket) {
        match packet {
            UI2NetPacket::SendMessage { msg } => {
                for (uid, peer) in self.peers {
                    let p2p_packet = P2PPacket::SendMessage { msg };

                    send_packet(p2p_packet, peer.stream);
                }
            }
            UI2NetPacket::Connect { addr } => {
                let stream = TcpStream::connect(addr);
                send_packet(P2PPacket::Connect { uid: self.my_uid }, stream);
            }
            UI2NetPacket::Disconnect => {
                for (uid, peer) in self.peers {
                    send_packet(P2PPacket::Disconnect, peer.stream);
                }
            }
            UI2NetPacket::GetPeerList { uid, reply } => {
                let list: Vec<PeerInfo> = Vec::new();
                for (uid, peer) in self.peers {
                    list.append(PeerInfo {
                        uid,
                        display_name: peer.id,
                        addr: peer.stream.peer_addr(),
                    });
                }
                reply.send(list);
            }
        }
    }
    async fn new(
        port: u16,
        ui2net_rx: Receiver<UI2NetPacket>,
        net2ui_tx: Sender<Net2UIPacket>,
    ) -> ConnectionManager {
        return ConnectionManager {
            peers: HashMap::new(),
            my_uid: Uuid::new_v4(),
            port: port,
            ui2net_rx,
            net2ui_tx,
        };
    }
    async fn start_loop(&mut self) {
        let listener = TcpListener::bind(("0.0.0.0", self.port)).await?;

        loop {
            select! {
                Ok((stream, addr)) = listener.accept() => {
                    tokio::spawn(async move || {
                        self.handle_connection(stream).await?;
                    });
                }
                Some(packet) = self.ui2net_rx.recv() => {
                    self.handle_ui2net_packet(packet).await;
                }
            }
        }
    }
}

pub async fn run_connection_manager(
    port: u16,
    ui2net_rx: Receiver<UI2NetPacket>,
    net2ui_tx: Sender<Net2UIPacket>,
) -> Result<(), std::io::Error> {
    tracing::info!("Starting connection manager");

    let manager = ConnectionManager::new(53412, ui2net_rx, net2ui_tx).await;

    manager.start_loop();
}
