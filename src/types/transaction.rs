use crate::crypto::hash_string;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "tx_type")]
pub enum TransactionType {
    CreateProject,
    FundProject,
    Transfer,
    Coinbase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectData {
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub goal_amount: u64,
    pub deadline_timestamp: u64,
    pub creator_wallet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundProjectData {
    pub project_id: String,
    pub amount: u64,
    #[serde(default)]
    pub backer_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferData {
    pub amount: u64,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseData {
    pub reward: u64,
    pub block_height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TransactionData {
    CreateProject(CreateProjectData),
    FundProject(FundProjectData),
    Transfer(TransferData),
    Coinbase(CoinbaseData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    #[serde(flatten)]
    pub tx_type: TransactionType,
    pub from: Option<String>,
    pub to: Option<String>,
    pub data: TransactionData,
    pub timestamp: u64,
    pub signature: Option<String>,
}

impl Transaction {
    pub fn new(
        tx_type: TransactionType,
        from: Option<String>,
        to: Option<String>,
        data: TransactionData,
        timestamp: u64,
        signature: Option<String>,
    ) -> Self {
        let tx = Transaction {
            id: String::new(),
            tx_type,
            from,
            to,
            data,
            timestamp,
            signature,
        };

        let id = tx.calculate_id();

        Transaction { id, ..tx }
    }

    /// Вычисляет ID транзакции как SHA-256 хеш от данных для подписи
    pub fn calculate_id(&self) -> String {
        let mut tx_copy = self.clone();
        tx_copy.signature = None;
        tx_copy.id = String::new();
        let serialized = serde_json::to_string(&tx_copy).unwrap_or_default();
        hash_string(&serialized)
    }

    pub fn get_signing_data(&self) -> String {
        let mut tx_copy = self.clone();
        tx_copy.signature = None;
        serde_json::to_string(&tx_copy).unwrap_or_default()
    }

    pub fn requires_signature(&self) -> bool {
        !matches!(self.tx_type, TransactionType::Coinbase)
    }
}

/// тесты нужны для проврки детерминированности сериализации структуры при недетерминированности
/// json объектов
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_id_deterministic() {
        let tx1 = Transaction::new(
            TransactionType::Transfer,
            Some("from_key".to_string()),
            Some("to_address".to_string()),
            TransactionData::Transfer(TransferData {
                amount: 100,
                message: "test".to_string(),
            }),
            1234567890,
            Some("signature_abc".to_string()),
        );

        let tx2 = Transaction::new(
            TransactionType::Transfer,
            Some("from_key".to_string()),
            Some("to_address".to_string()),
            TransactionData::Transfer(TransferData {
                amount: 100,
                message: "test".to_string(),
            }),
            1234567890,
            Some("signature_xyz".to_string()),
        );

        assert_eq!(
            tx1.id, tx2.id,
            "ID транзакции должен быть детерминированным"
        );
    }

    #[test]
    fn test_signing_data_deterministic() {
        let tx = Transaction::new(
            TransactionType::Transfer,
            Some("from_key".to_string()),
            Some("to_address".to_string()),
            TransactionData::Transfer(TransferData {
                amount: 100,
                message: "test".to_string(),
            }),
            1234567890,
            Some("signature".to_string()),
        );

        // get_signing_data() должен возвращать одинаковый JSON
        let json1 = tx.get_signing_data();
        let json2 = tx.get_signing_data();
        assert_eq!(
            json1, json2,
            "JSON для подписи должен быть детерминированным"
        );
    }
}
