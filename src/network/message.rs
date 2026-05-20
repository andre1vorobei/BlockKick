use crate::types::{Block, Transaction};
use serde::{Deserialize, Serialize};

/// Сетевые сообщения между нодами
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NetworkMessage {
    /// Handshake при подключении
    Handshake {
        version: u32,
        chain_hash: String, // ← ИСПРАВЛЕНО: было genesis_hash
        user_agent: String,
        start_height: u64,
    },

    /// Ответ на handshake
    HandshakeAck {
        version: u32,
        chain_hash: String, // ← ДОБАВЛЕНО: для проверки совместимости
        start_height: u64,
    },

    /// Новая транзакция (broadcast)
    Tx {
        tx: Transaction,
    },

    /// Новый блок (broadcast)
    Block {
        block: Block,
    },

    /// Запрос блоков (при отставании)
    GetBlocks {
        from_height: u64,
        count: Option<u64>, // ← ИСПРАВЛЕНО: было limit
    },

    /// Ответ с блоками
    Blocks {
        blocks: Vec<Block>,
    },

    /// Ping/Pong для keepalive
    Ping,
    Pong,

    /// Запрос состояния
    GetPeerInfo,
    PeerInfo {
        height: u64,
        best_block_hash: String,
    },
}

impl NetworkMessage {
    /// Название сообщения для логирования
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        // ← ДОБАВЛЕНО
        match self {
            NetworkMessage::Handshake { .. } => "Handshake",
            NetworkMessage::HandshakeAck { .. } => "HandshakeAck",
            NetworkMessage::Tx { .. } => "Tx",
            NetworkMessage::Block { .. } => "Block",
            NetworkMessage::GetBlocks { .. } => "GetBlocks",
            NetworkMessage::Blocks { .. } => "Blocks",
            NetworkMessage::Ping => "Ping",
            NetworkMessage::Pong => "Pong",
            NetworkMessage::GetPeerInfo => "GetPeerInfo",
            NetworkMessage::PeerInfo { .. } => "PeerInfo",
        }
    }
}
/// Сериализация сообщения в байты (JSON + длина префикс)
pub fn encode_message(msg: &NetworkMessage) -> Result<Vec<u8>, String> {
    // Используем serde_json вместо bincode
    let data = serde_json::to_vec(msg).map_err(|e| format!("JSON serialize error: {}", e))?;

    // Префикс: 4 байта длина в big-endian
    let len = (data.len() as u32).to_be_bytes();
    let mut buf = Vec::with_capacity(4 + data.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(&data);
    Ok(buf)
}

/// Десериализация сообщения из байтов
pub fn decode_message(buf: &[u8]) -> Result<Option<(NetworkMessage, usize)>, String> {
    if buf.len() < 4 {
        return Ok(None); // Ждём больше данных
    }

    // Читаем длину сообщения
    let msg_len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() < 4 + msg_len {
        return Ok(None); // Ждём больше данных
    }

    // Десериализуем JSON
    let msg_data = &buf[4..4 + msg_len];
    let msg =
        serde_json::from_slice(msg_data).map_err(|e| format!("JSON deserialize error: {}", e))?;

    Ok(Some((msg, 4 + msg_len)))
}
