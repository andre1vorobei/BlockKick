use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::network::fork_resolver::{ForkResolver, ForkResult};
use crate::network::message::{NetworkMessage, decode_message, encode_message};
use crate::network::peer_manager::{PeerInfo, PeerManager};
use crate::storage::Blockchain;

mod fork_resolver;
mod message;
mod peer_manager;

// ← ИСПРАВЛЕНО: убираем дублирующиеся pub use
//pub use fork_resolver::ForkResult;
//pub use message::NetworkMessage;
//pub use peer_manager::PeerInfo;

/// Сетевой сервис ноды
pub struct NetworkService {
    peer_manager: Arc<Mutex<PeerManager>>,
    fork_resolver: Arc<Mutex<ForkResolver>>,
    blockchain: Arc<Mutex<Blockchain>>,
    listen_addr: SocketAddr,
    chain_hash: String, // ← ИСПРАВЛЕНО: переименовано из genesis_hash
}

impl NetworkService {
    pub fn new(
        listen_addr: SocketAddr,
        chain_hash: String, // ← ИСПРАВЛЕНО
        blockchain: Arc<Mutex<Blockchain>>,
    ) -> Self {
        let pm = Arc::new(Mutex::new(PeerManager::new(chain_hash.clone())));
        let fr = Arc::new(Mutex::new(ForkResolver::new(Arc::clone(&blockchain))));

        Self {
            peer_manager: pm,
            fork_resolver: fr,
            blockchain,
            listen_addr,
            chain_hash,
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(self.listen_addr).await?;
        println!("🌐 P2P listening on {}", self.listen_addr);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    let svc = self.clone_for_peer();
                    tokio::spawn(async move {
                        if let Err(e) = handle_peer(socket, addr, svc).await {
                            eprintln!("Peer {} error: {}", addr, e);
                        }
                    });
                }
                Err(e) => eprintln!("Accept error: {}", e),
            }
        }
    }

    pub async fn connect_to(&self, addr: SocketAddr) -> Result<(), String> {
        let mut stream = TcpStream::connect(addr)
            .await
            .map_err(|e| format!("Connect failed: {}", e))?;

        let (tx, rx) = mpsc::channel(100);

        {
            let mut pm = self.peer_manager.lock().unwrap();
            pm.register(
                addr,
                tx,
                PeerInfo {
                    addr,
                    version: 1,
                    user_agent: "BlockKick/0.4.0".into(),
                    start_height: self.blockchain.lock().unwrap().height(),
                    is_outbound: true,
                },
            );
        }

        // ← ИСПРАВЛЕНО: используем chain_hash
        let handshake = NetworkMessage::Handshake {
            version: 1,
            chain_hash: self.chain_hash.clone(),
            user_agent: "BlockKick/0.4.0".into(),
            start_height: self.blockchain.lock().unwrap().height(),
        };

        if let Err(e) = send_message(&mut stream, &handshake).await {
            return Err(format!("Handshake send failed: {}", e));
        }

        let svc = self.clone_for_peer();
        tokio::spawn(async move {
            if let Err(e) = handle_peer_stream(stream, addr, rx, svc, true).await {
                eprintln!("Outbound peer {} error: {}", addr, e);
            }
        });

        Ok(())
    }

    pub fn broadcast_tx(&self, tx: crate::types::Transaction, exclude: Option<SocketAddr>) {
        let msg = NetworkMessage::Tx { tx };
        self.peer_manager.lock().unwrap().broadcast(msg, exclude);
    }

    pub fn broadcast_block(&self, block: crate::types::Block, exclude: Option<SocketAddr>) {
        let msg = NetworkMessage::Block { block };
        self.peer_manager.lock().unwrap().broadcast(msg, exclude);
    }

    fn clone_for_peer(&self) -> PeerContext {
        PeerContext {
            peer_manager: Arc::clone(&self.peer_manager),
            fork_resolver: Arc::clone(&self.fork_resolver),
            blockchain: Arc::clone(&self.blockchain),
            chain_hash: self.chain_hash.clone(),
        }
    }
}

impl Clone for NetworkService {
    fn clone(&self) -> Self {
        Self {
            peer_manager: Arc::clone(&self.peer_manager),
            fork_resolver: Arc::clone(&self.fork_resolver),
            blockchain: Arc::clone(&self.blockchain),
            listen_addr: self.listen_addr,
            chain_hash: self.chain_hash.clone(),
        }
    }
}

#[derive(Clone)]
struct PeerContext {
    peer_manager: Arc<Mutex<PeerManager>>,
    fork_resolver: Arc<Mutex<ForkResolver>>,
    blockchain: Arc<Mutex<Blockchain>>,
    chain_hash: String,
}

async fn handle_peer(socket: TcpStream, addr: SocketAddr, ctx: PeerContext) -> Result<(), String> {
    let (tx, rx) = mpsc::channel(100);

    {
        let mut pm = ctx.peer_manager.lock().unwrap();
        pm.register(
            addr,
            tx,
            PeerInfo {
                addr,
                version: 0,
                user_agent: String::new(),
                start_height: 0,
                is_outbound: false,
            },
        );
    }

    handle_peer_stream(socket, addr, rx, ctx, false).await
}

async fn handle_peer_stream(
    mut socket: TcpStream,
    addr: SocketAddr,
    mut rx: mpsc::Receiver<NetworkMessage>,
    ctx: PeerContext,
    is_outbound: bool,
) -> Result<(), String> {
    let mut buf = vec![0u8; 65536];
    let mut buf_pos = 0;
    let mut handshake_done = !is_outbound;

    loop {
        tokio::select! {
            result = socket.read(&mut buf[buf_pos..]) => {
                let n = result.map_err(|e| format!("Read error: {}", e))?;
                if n == 0 { break; }
                buf_pos += n;

                loop {
                    match decode_message(&buf[..buf_pos]) {
                        Ok(Some((msg, consumed))) => {
                            let msg_name = msg.name();
                            if let Err(e) = process_message(msg, addr, &mut socket, &ctx, &mut handshake_done).await {
                                eprintln!("Error processing {} from {}: {}", msg_name, addr, e);
                            }
                            buf.copy_within(consumed..buf_pos, 0);
                            buf_pos -= consumed;
                        }
                        Ok(None) => break,
                        Err(e) => {
                            eprintln!("Decode error from {}: {}", addr, e);
                            break;
                        }
                    }
                }
            }
            Some(msg) = rx.recv() => {
                if let Err(e) = send_message(&mut socket, &msg).await {
                    eprintln!("Send error to {}: {}", addr, e);
                    break;
                }
            }
        }
    }

    ctx.peer_manager.lock().unwrap().remove(&addr);
    Ok(())
}

async fn send_message(socket: &mut TcpStream, msg: &NetworkMessage) -> Result<(), String> {
    let data = encode_message(msg)?;
    socket
        .write_all(&data)
        .await
        .map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}

async fn process_message(
    msg: NetworkMessage,
    from: SocketAddr,
    socket: &mut TcpStream,
    ctx: &PeerContext,
    handshake_done: &mut bool,
) -> Result<(), String> {
    match msg {
        NetworkMessage::Handshake {
            version,
            chain_hash,
            user_agent,
            start_height,
        } => {
            if !ctx.peer_manager.lock().unwrap().check_chain(&chain_hash) {
                return Err("Incompatible network".into());
            }

            // ← ИСПРАВЛЕНО: используем публичный метод update_peer_info
            ctx.peer_manager.lock().unwrap().update_peer_info(
                &from,
                version,
                user_agent.clone(),
                start_height,
            );

            let ack = NetworkMessage::HandshakeAck {
                version: 1,
                chain_hash: ctx.chain_hash.clone(), // ← ИСПРАВЛЕНО
                start_height: ctx.blockchain.lock().unwrap().height(),
            };
            send_message(socket, &ack).await?;
            *handshake_done = true;
        }

        NetworkMessage::HandshakeAck {
            version,
            chain_hash: _,
            start_height,
        } => {
            // ← ИСПРАВЛЕНО: используем .. для игнорирования chain_hash
            ctx.peer_manager.lock().unwrap().update_peer_info(
                &from,
                version,
                String::new(),
                start_height,
            );
            *handshake_done = true;
        }

        NetworkMessage::Tx { tx } => {
            if !*handshake_done {
                return Ok(());
            }
            eprintln!("📥 Received tx {} from {}", tx.id, from);
            ctx.peer_manager
                .lock()
                .unwrap()
                .broadcast(NetworkMessage::Tx { tx }, Some(from));
        }

        NetworkMessage::Block { block } => {
            if !*handshake_done {
                return Ok(());
            }
            let result = ctx
                .fork_resolver
                .lock()
                .unwrap()
                .handle_block(block.clone());
            if matches!(
                result,
                ForkResult::AcceptedMainChain | ForkResult::Reorganized { .. }
            ) {
                ctx.peer_manager
                    .lock()
                    .unwrap()
                    .broadcast(NetworkMessage::Block { block }, Some(from));
            }
        }

        NetworkMessage::Ping => {
            send_message(socket, &NetworkMessage::Pong).await?;
        }

        // ← ИСПРАВЛЕНО: используем count вместо limit
        NetworkMessage::GetBlocks { from_height, count } => {
            let blocks = {
                let chain = ctx.blockchain.lock().unwrap();
                let mut blocks = Vec::new();
                let max = count.unwrap_or(100);

                for h in from_height..from_height + max {
                    if let Some(b) = chain.get_block(h) {
                        blocks.push(b.clone());
                    } else {
                        break;
                    }
                }
                blocks
            };

            if !blocks.is_empty() {
                send_message(socket, &NetworkMessage::Blocks { blocks }).await?;
            }
        }

        _ => {}
    }
    Ok(())
}
use std::sync::OnceLock;

static GLOBAL_NETWORK: OnceLock<Arc<NetworkService>> = OnceLock::new();

pub fn set_global_network(net: Arc<NetworkService>) -> bool {
    GLOBAL_NETWORK.set(net).is_ok()
}

pub fn get_global_network() -> Option<&'static Arc<NetworkService>> {
    GLOBAL_NETWORK.get()
}
