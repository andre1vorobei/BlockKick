use serde::Serialize;
use std::sync::{Arc, Mutex};
use tiny_http::{Request, Response, StatusCode};

use crate::storage::Blockchain;

// === Response структуры ===

#[derive(Serialize)]
pub struct BalanceResponse {
    pub balance: u64,
}

#[derive(Serialize)]
pub struct ChainInfoResponse {
    pub height: u64,
    pub latest_hash: String,
}

#[derive(Serialize)]
pub struct BlockInfoResponse {
    pub index: u64,
    pub hash: String,
    pub timestamp: u64,
    pub transaction_count: usize,
}

#[derive(Serialize)]
pub struct ProjectSummary {
    pub project_id: String,
    pub name: String,
    pub goal_amount: u64,
    pub raised_amount: u64,
    pub status: String,
    pub creator_wallet: String,
}

// === GET /api/v1/balance/{address} ===

pub fn handle_balance(request: Request, blockchain: &Arc<Mutex<Blockchain>>) -> Result<(), String> {
    let url = request.url().to_string();
    let parts: Vec<&str> = url.split('/').collect();

    if parts.len() < 5 {
        return send_error(request, StatusCode(400), "Invalid URL");
    }

    let address = parts[4];
    let chain = blockchain.lock().unwrap();

    // Баланс вычисляется on-demand по всем блокам
    let balance = chain.get_balance(address);

    send_json(request, StatusCode(200), &BalanceResponse { balance })
}

// === GET /api/v1/chain ===

pub fn handle_chain_info(
    request: Request,
    blockchain: &Arc<Mutex<Blockchain>>,
) -> Result<(), String> {
    let chain = blockchain.lock().unwrap();

    let response = match chain.get_latest_block() {
        Some(block) => ChainInfoResponse {
            height: block.header.index,
            latest_hash: block.calculate_hash(),
        },
        None => ChainInfoResponse {
            height: 0,
            latest_hash: String::new(),
        },
    };

    send_json(request, StatusCode(200), &response)
}

// === GET /api/v1/block/{height} ===

pub fn handle_block(request: Request, blockchain: &Arc<Mutex<Blockchain>>) -> Result<(), String> {
    let url = request.url().to_string();
    let parts: Vec<&str> = url.split('/').collect();

    if parts.len() < 5 {
        return send_error(request, StatusCode(400), "Invalid URL");
    }

    let height: u64 = parts[4].parse().map_err(|_| "Invalid block height")?;

    let chain = blockchain.lock().unwrap();

    match chain.get_block(height) {
        Some(block) => {
            let response = BlockInfoResponse {
                index: block.header.index,
                hash: block.calculate_hash(),
                timestamp: block.header.timestamp,
                transaction_count: block.transactions.len(),
            };
            send_json(request, StatusCode(200), &response)
        }
        None => send_error(request, StatusCode(404), "Block not found"),
    }
}

// === GET /api/v1/projects ===

pub fn handle_projects(
    request: Request,
    blockchain: &Arc<Mutex<Blockchain>>,
) -> Result<(), String> {
    let chain = blockchain.lock().unwrap();

    // Собираем все проекты из цепи
    let mut projects: Vec<ProjectSummary> = Vec::new();

    for block in chain.get_blocks() {
        for tx in &block.transactions {
            if let crate::types::TransactionData::CreateProject(data) = &tx.data {
                // Вычисляем raised_amount из fund_project транзакций
                let mut raised = 0u64;
                for inner_block in chain.get_blocks() {
                    for inner_tx in &inner_block.transactions {
                        if let crate::types::TransactionData::FundProject(fund) = &inner_tx.data {
                            if fund.project_id == data.project_id {
                                raised += fund.amount;
                            }
                        }
                    }
                }

                // Определяем статус
                let status = determine_project_status(&data, raised);

                projects.push(ProjectSummary {
                    project_id: data.project_id.clone(),
                    name: data.name.clone(),
                    goal_amount: data.goal_amount,
                    raised_amount: raised,
                    status,
                    creator_wallet: data.creator_wallet.clone(),
                });
            }
        }
    }

    send_json(request, StatusCode(200), &projects)
}

// === Вспомогательные функции ===

fn determine_project_status(data: &crate::types::CreateProjectData, raised: u64) -> String {
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if current_time > data.deadline_timestamp {
        if raised >= data.goal_amount {
            "SUCCESS".to_string()
        } else {
            "FAILED".to_string()
        }
    } else if raised > 0 {
        "ACTIVE".to_string()
    } else {
        "CREATED".to_string()
    }
}

fn send_json<T: Serialize>(request: Request, status: StatusCode, data: &T) -> Result<(), String> {
    let json = serde_json::to_string(data).unwrap();
    let response = Response::new(status, vec![], json.as_bytes(), None, None);
    request.respond(response).map_err(|e| e.to_string())
}

fn send_error(request: Request, status: StatusCode, message: &str) -> Result<(), String> {
    let response = Response::new(status, vec![], message.as_bytes(), None, None);
    request.respond(response).map_err(|e| e.to_string())
}
