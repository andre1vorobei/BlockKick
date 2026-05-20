use serde::Serialize;
use std::sync::{Arc, Mutex};
use tiny_http::{Request, Response, StatusCode};
use url::form_urlencoded;

use crate::consensus::{BLOCK_REWARD, DIFFICULTY};
use crate::mempool::Mempool;
use crate::storage::Blockchain;
use crate::types::{Block, BlockHeader, Transaction};

// === Response структуры ===

#[derive(Serialize)]
pub struct MiningCandidate {
    pub block_template: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub prev_hash: String,
    pub difficulty: u32,
    pub reward: u64,
}

#[derive(Serialize)]
pub struct SubmitResponse {
    pub status: String,
    pub reward: u64,
}

// GET /api/v1/mining/candidate?miner=<address>
pub fn handle_get_candidate(
    request: Request,
    blockchain: &Arc<Mutex<Blockchain>>,
    mempool: &Arc<Mutex<Mempool>>,
) -> Result<(), String> {
    let chain = blockchain.lock().unwrap();
    let mp = mempool.lock().unwrap();

    let latest = chain
        .get_latest_block()
        .ok_or_else(|| "Blockchain is empty")?;

    let prev_hash = latest.calculate_hash();
    let next_height = latest.header.index + 1;

    // Парсим ?miner=<address> из запроса
    let miner_address = parse_query_param(request.url(), "miner").unwrap_or_else(|| {
        "0000000000000000000000000000000000000000000000000000000000000000".to_string()
    });

    // Берём транзакции из мемпула
    let mut transactions: Vec<Transaction> =
        mp.get_transactions().iter().take(100).cloned().collect();

    // Создаём coinbase СРАЗУ с правильным адресом майнера
    let coinbase = Transaction::create_coinbase(
        miner_address.clone(), // ← Реальный адрес из запроса!
        BLOCK_REWARD,
        next_height,
    );
    transactions.insert(0, coinbase);

    // Вычисляем Merkle Root (уже с правильным coinbase)
    let merkle_root = Block::calculate_merkle_root(&transactions);

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let block_template = BlockHeader {
        index: next_height,
        timestamp: current_time,
        prev_hash: prev_hash.clone(),
        merkle_root,
        nonce: 0,
    };

    let candidate = MiningCandidate {
        block_template,
        transactions,
        prev_hash,
        difficulty: DIFFICULTY,
        reward: BLOCK_REWARD,
    };

    send_json(request, StatusCode(200), &candidate)
}

// Вспомогательная функция для парсинга query params
fn parse_query_param(url: &str, param: &str) -> Option<String> {
    url.split('?').nth(1).and_then(|query| {
        form_urlencoded::parse(query.as_bytes())
            .find(|(k, _)| k == param)
            .map(|(_, v)| v.into_owned())
    })
}
// === POST /api/v1/mining/submit ===

pub fn handle_submit(
    mut request: Request,
    blockchain: &Arc<Mutex<Blockchain>>,
    mempool: &Arc<Mutex<Mempool>>,
) -> Result<(), String> {
    // Читаем блок из тела запроса
    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .map_err(|e| format!("Failed to read body: {}", e))?;

    let block: Block = serde_json::from_str(&body).map_err(|e| format!("Invalid JSON: {}", e))?;

    let mut chain = blockchain.lock().unwrap();

    {
        // Берём лок на мемпул (отдельно от chain, чтобы не было дедлока)
        let mut mp = mempool.lock().unwrap();

        for tx in &block.transactions {
            if tx.tx_type != crate::types::TransactionType::Coinbase {
                mp.remove_transaction(&tx.id);
            }
        }
    }

    // 1. Проверяем связь с последним блоком
    let latest = chain
        .get_latest_block()
        .ok_or_else(|| "Blockchain is empty")?;

    let expected_prev = latest.calculate_hash();
    if block.header.prev_hash != expected_prev {
        return send_error(request, StatusCode(400), "Invalid prev_hash");
    }

    // 2. Проверяем PoW
    let block_hash = block.calculate_hash();
    if !crate::consensus::verify_pow(&block_hash) {
        return send_error(request, StatusCode(400), "Invalid PoW");
    }

    // 3. Валидируем и добавляем блок
    if let Err(e) = chain.validate_and_add_block(block.clone()) {
        return send_error(request, StatusCode(400), &format!("Invalid block: {}", e));
    }

    if let Some(net) = crate::network::get_global_network() {
        net.broadcast_block(block.clone(), None); // None = всем пирам
    }
    // Успех
    send_json(
        request,
        StatusCode(200),
        &SubmitResponse {
            status: "accepted".to_string(),
            reward: BLOCK_REWARD,
        },
    )
}

// === Вспомогательные функции ===

fn send_json<T: Serialize>(request: Request, status: StatusCode, data: &T) -> Result<(), String> {
    let json = serde_json::to_string(data).unwrap();
    let response = Response::new(status, vec![], json.as_bytes(), None, None);
    request.respond(response).map_err(|e| e.to_string())
}

fn send_error(request: Request, status: StatusCode, message: &str) -> Result<(), String> {
    let response = Response::new(status, vec![], message.as_bytes(), None, None);
    request.respond(response).map_err(|e| e.to_string())
}
