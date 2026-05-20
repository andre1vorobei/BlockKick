use serde::Serialize;
use std::sync::{Arc, Mutex};
use tiny_http::{Request, Response, StatusCode};

use crate::mempool::Mempool;
use crate::storage::Blockchain;
use crate::types::Transaction;

// === Response структуры ===

#[derive(Serialize)]
pub struct TxResponse {
    pub tx_id: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct TxStatusResponse {
    pub status: String,
    pub block_height: Option<u64>,
}

// === POST /api/v1/transactions ===

pub fn handle_post(
    mut request: Request,
    blockchain: &Arc<Mutex<Blockchain>>,
    mempool: &Arc<Mutex<Mempool>>,
) -> Result<(), String> {
    // Читаем тело запроса
    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .map_err(|e| format!("Failed to read body: {}", e))?;

    // Десериализуем транзакцию
    let tx: Transaction =
        serde_json::from_str(&body).map_err(|e| format!("Invalid JSON: {}", e))?;

    // Валидация и добавление в мемпул
    {
        let chain = blockchain.lock().unwrap();
        let mut mp = mempool.lock().unwrap();

        // Проверяем что транзакция ещё не подтверждена
        if chain.is_transaction_confirmed(&tx.id) {
            return send_json(
                request,
                StatusCode(200),
                &TxResponse {
                    tx_id: tx.id.clone(),
                    status: "already_confirmed".to_string(),
                },
            );
        }

        // Проверяем что нет дубликата в мемпуле
        if mp.is_in_mempool(&tx.id) {
            return send_json(
                request,
                StatusCode(200),
                &TxResponse {
                    tx_id: tx.id.clone(),
                    status: "already_pending".to_string(),
                },
            );
        }

        // Валидируем и добавляем в мемпул
        if let Err(e) = mp.add_transaction(tx.clone(), &chain) {
            return send_json(
                request,
                StatusCode(400),
                &TxResponse {
                    tx_id: tx.id.clone(),
                    status: format!("rejected: {}", e),
                },
            );
        }

        if let Some(net) = crate::network::get_global_network() {
            net.broadcast_tx(tx.clone(), None); // None = всем пирам
        }
    }

    // Успех
    send_json(
        request,
        StatusCode(200),
        &TxResponse {
            tx_id: tx.id.clone(),
            status: "pending".to_string(),
        },
    )?;

    // TODO: Broadcast to P2P network

    Ok(())
}

// === GET /api/v1/transactions/{tx_id} ===

pub fn handle_get(
    request: Request,
    blockchain: &Arc<Mutex<Blockchain>>,
    mempool: &Arc<Mutex<Mempool>>,
) -> Result<(), String> {
    // Парсим tx_id из URL: /api/v1/transactions/{tx_id}
    let url = request.url().to_string();
    let parts: Vec<&str> = url.split('/').collect();

    if parts.len() < 5 {
        return send_error(request, StatusCode(400), "Invalid URL");
    }

    let tx_id = parts[4];

    let chain = blockchain.lock().unwrap();
    let mp = mempool.lock().unwrap();

    // Проверяем подтверждённые транзакции
    if let Some((height, _)) = chain.get_transaction_location(tx_id) {
        return send_json(
            request,
            StatusCode(200),
            &TxStatusResponse {
                status: "confirmed".to_string(),
                block_height: Some(height),
            },
        );
    }

    // Проверяем мемпул
    if mp.is_in_mempool(tx_id) {
        return send_json(
            request,
            StatusCode(200),
            &TxStatusResponse {
                status: "pending".to_string(),
                block_height: None,
            },
        );
    }

    // Не найдено
    send_error(request, StatusCode(404), "Transaction not found")
}

// === Вспомогательные функции ===

fn send_json<T: Serialize>(request: Request, status: StatusCode, data: &T) -> Result<(), String> {
    let json = serde_json::to_string(data).unwrap();
    let response = Response::new(
        status,
        vec![],
        json.as_bytes(), // &[u8] implements Read
        None,
        None,
    );
    request.respond(response).map_err(|e| e.to_string())
}

fn send_error(request: Request, status: StatusCode, message: &str) -> Result<(), String> {
    let response = Response::new(status, vec![], message.as_bytes(), None, None);
    request.respond(response).map_err(|e| e.to_string())
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::TransferData;
//     use crate::blockchain::Blockchain;
//     use crate::crypto::keys::KeyPair;
//     use crate::mempool::Mempool;
//     use crate::types::{Transaction, TransactionData, TransactionType};
//     use std::io::Cursor;
//     use std::sync::{Arc, Mutex};
//     use tiny_http::{Request, Server, StatusCode};
//
//     // === Helper Functions ===
//
//     fn create_test_context() -> (Arc<Mutex<Blockchain>>, Arc<Mutex<Mempool>>) {
//         let blockchain = Arc::new(Mutex::new(Blockchain::new()));
//         let mempool = Arc::new(Mutex::new(Mempool::new()));
//         (blockchain, mempool)
//     }
//
//     fn create_signed_transfer() -> Transaction {
//         let keypair = KeyPair::generate();
//         let mut tx = Transaction::new(
//             TransactionType::Transfer,
//             Some(keypair.public_key.clone()),
//             Some("receiver_pubkey".to_string()),
//             TransactionData::Transfer(TransferData {
//                 amount: 100,
//                 message: "test".to_string(),
//             }),
//             1234567890,
//         );
//
//         let signing_data = tx.get_signing_data();
//         let signature = crate::crypto::sign_data(&keypair.private_key, signing_data.as_bytes());
//         tx.add_signature(signature);
//
//         tx
//     }
//
//     fn create_mock_request(method: &str, url: &str, body: &str) -> Request {
//         let method = tiny_http::Method::from_string(method.to_string()).unwrap();
//         let url = tiny_http::URL::from_string(url.to_string()).unwrap();
//         let headers = vec![
//             tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
//         ];
//         let data = Cursor::new(body.as_bytes().to_vec());
//         let remote_addr = "127.0.0.1:8080".parse().unwrap();
//
//         Request::new(method, url, headers, data, None, remote_addr)
//     }
//
//     // === POST /api/v1/transactions Tests ===
//
//     #[test]
//     fn test_handle_post_transaction_valid() {
//         let (blockchain, mempool) = create_test_context();
//         let tx = create_signed_transfer();
//         let body = serde_json::to_string(&tx).unwrap();
//
//         let request = create_mock_request("POST", "/api/v1/transactions", &body);
//         let result = handle_post(request, &blockchain, &mempool);
//
//         assert!(result.is_ok(), "Valid transaction should be accepted");
//
//         // Verify transaction is in mempool
//         let mp = mempool.lock().unwrap();
//         assert!(mp.is_in_mempool(&tx.id));
//     }
//
//     #[test]
//     fn test_handle_post_transaction_duplicate_in_mempool() {
//         let (blockchain, mempool) = create_test_context();
//         let tx = create_signed_transfer();
//
//         // Add to mempool first
//         {
//             let mut mp = mempool.lock().unwrap();
//             let chain = blockchain.lock().unwrap();
//             mp.add_transaction(tx.clone(), &chain).unwrap();
//         }
//
//         // Try to add again
//         let body = serde_json::to_string(&tx).unwrap();
//         let request = create_mock_request("POST", "/api/v1/transactions", &body);
//         let result = handle_post(request, &blockchain, &mempool);
//
//         assert!(result.is_ok());
//         // Should return status "already_pending"
//     }
//
//     #[test]
//     fn test_handle_post_transaction_invalid_json() {
//         let (blockchain, mempool) = create_test_context();
//         let body = "{\"invalid\": json}";
//
//         let request = create_mock_request("POST", "/api/v1/transactions", body);
//         let result = handle_post(request, &blockchain, &mempool);
//
//         assert!(result.is_err(), "Invalid JSON should be rejected");
//     }
//
//     #[test]
//     fn test_handle_post_transaction_invalid_signature() {
//         let (blockchain, mempool) = create_test_context();
//         let mut tx = create_signed_transfer();
//
//         // Tamper with signature
//         tx.signature = Some("invalid_signature".to_string());
//
//         let body = serde_json::to_string(&tx).unwrap();
//         let request = create_mock_request("POST", "/api/v1/transactions", &body);
//         let result = handle_post(request, &blockchain, &mempool);
//
//         assert!(result.is_ok()); // Returns 400 with error message
//     }
//
//     // === GET /api/v1/transactions/{tx_id} Tests ===
//
//     #[test]
//     fn test_handle_get_transaction_confirmed() {
//         let (blockchain, mempool) = create_test_context();
//         let tx = create_signed_transfer();
//
//         // Add transaction to blockchain (simulate confirmed)
//         {
//             let mut chain = blockchain.lock().unwrap();
//             // Add to a block and add block to chain
//             // For simplicity, we just check the method works
//         }
//
//         let request = create_mock_request("GET", &format!("/api/v1/transactions/{}", tx.id), "");
//         let result = handle_get(request, &blockchain, &mempool);
//
//         assert!(result.is_ok());
//     }
//
//     #[test]
//     fn test_handle_get_transaction_pending() {
//         let (blockchain, mempool) = create_test_context();
//         let tx = create_signed_transfer();
//
//         // Add to mempool
//         {
//             let mut mp = mempool.lock().unwrap();
//             let chain = blockchain.lock().unwrap();
//             mp.add_transaction(tx.clone(), &chain).unwrap();
//         }
//
//         let request = create_mock_request("GET", &format!("/api/v1/transactions/{}", tx.id), "");
//         let result = handle_get(request, &blockchain, &mempool);
//
//         assert!(result.is_ok());
//     }
//
//     #[test]
//     fn test_handle_get_transaction_not_found() {
//         let (blockchain, mempool) = create_test_context();
//
//         let request = create_mock_request("GET", "/api/v1/transactions/nonexistent", "");
//         let result = handle_get(request, &blockchain, &mempool);
//
//         assert!(result.is_ok()); // Returns 404
//     }
//
//     #[test]
//     fn test_handle_get_transaction_invalid_url() {
//         let (blockchain, mempool) = create_test_context();
//
//         let request = create_mock_request("GET", "/api/v1/transactions", "");
//         let result = handle_get(request, &blockchain, &mempool);
//
//         assert!(result.is_ok()); // Returns 400
//     }
// }
