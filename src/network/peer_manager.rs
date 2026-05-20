use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::sync::mpsc;

use crate::network::message::NetworkMessage;

/// Информация о подключённом пире
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub addr: SocketAddr,
    pub version: u32,
    pub user_agent: String,
    pub start_height: u64,
    pub is_outbound: bool,
}

/// Менеджер пиров
pub struct PeerManager {
    /// Активные соединения: addr -> sender
    peers: HashMap<SocketAddr, mpsc::Sender<NetworkMessage>>,
    /// Информация о пирах
    peer_info: HashMap<SocketAddr, PeerInfo>, // ← Оставляем приватным
    /// Genesis hash для проверки совместимости
    genesis_hash: String,
}

impl PeerManager {
    pub fn new(genesis_hash: String) -> Self {
        Self {
            peers: HashMap::new(),
            peer_info: HashMap::new(),
            genesis_hash,
        }
    }

    /// Зарегистрировать новое соединение
    pub fn register(
        &mut self,
        addr: SocketAddr,
        sender: mpsc::Sender<NetworkMessage>,
        info: PeerInfo,
    ) {
        self.peers.insert(addr, sender);
        self.peer_info.insert(addr, info);
    }

    /// Удалить отключившегося пира
    pub fn remove(&mut self, addr: &SocketAddr) -> Option<PeerInfo> {
        self.peers.remove(addr);
        self.peer_info.remove(addr)
    }

    /// Отправить сообщение всем пирам (кроме exclude)
    pub fn broadcast(&self, msg: NetworkMessage, exclude: Option<SocketAddr>) {
        for (addr, sender) in &self.peers {
            if exclude.as_ref() == Some(addr) {
                continue;
            }
            let _ = sender.try_send(msg.clone());
        }
    }

    /// Отправить сообщение конкретному пиру
    pub fn send_to(&self, addr: &SocketAddr, msg: NetworkMessage) -> bool {
        if let Some(sender) = self.peers.get(addr) {
            sender.try_send(msg).is_ok()
        } else {
            false
        }
    }

    /// Проверить совместимость сети по chain_hash
    pub fn check_chain(&self, chain_hash: &str) -> bool {
        chain_hash == self.genesis_hash
    }

    /// Обновить информацию о пире (публичный метод вместо прямого доступа)
    /// ← ИСПРАВЛЕНО: заменяем прямой доступ к peer_info
    pub fn update_peer_info(
        &mut self,
        addr: &SocketAddr,
        version: u32,
        user_agent: String,
        start_height: u64,
    ) {
        if let Some(info) = self.peer_info.get_mut(addr) {
            info.version = version;
            info.user_agent = user_agent;
            info.start_height = start_height;
        }
    }

    /// Получить высоту пира
    #[allow(dead_code)]
    pub fn get_peer_height(&self, addr: &SocketAddr) -> Option<u64> {
        self.peer_info.get(addr).map(|i| i.start_height)
    }

    /// Количество активных пиров
    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        self.peers.len()
    }
}
