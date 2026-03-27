use crate::crypto::hash_string;
use crate::types::transaction::Transaction;
use crate::validator::Validator;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub index: u64,
    pub timestamp: u64,
    pub prev_hash: String,
    pub merkle_root: String,
    pub nonce: u64,
}

impl BlockHeader {
    pub fn calculate_hash(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap_or_default();
        hash_string(&serialized)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    pub fn new(index: u64, prev_hash: String, transactions: Vec<Transaction>, nonce: u64) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let merkle_root = Self::calculate_merkle_root(&transactions);

        let header = BlockHeader {
            index,
            timestamp,
            prev_hash,
            merkle_root,
            nonce,
        };

        Block {
            header,
            transactions,
        }
    }

    pub fn genesis() -> Self {
        let merkle_root = hash_string("empty");
        Block {
            header: BlockHeader {
                index: 0,
                timestamp: 0,
                prev_hash: "0".repeat(64),
                merkle_root,
                nonce: 0,
            },
            transactions: Vec::new(),
        }
    }

    pub fn calculate_merkle_root(transactions: &[Transaction]) -> String {
        if transactions.is_empty() {
            return hash_string("empty");
        }

        // Collect transaction hashes (IDs)
        let mut hashes: Vec<String> = transactions.iter().map(|tx| tx.id.clone()).collect();

        // Build Merkle tree
        while hashes.len() > 1 {
            // If odd number of hashes, duplicate the last one
            if hashes.len() % 2 != 0 {
                if let Some(last) = hashes.last().cloned() {
                    hashes.push(last);
                }
            }

            let mut next_level = Vec::new();

            for i in (0..hashes.len()).step_by(2) {
                let left = &hashes[i];
                let right = &hashes[i + 1];
                let combined = format!("{}{}", left, right);
                next_level.push(hash_string(&combined));
            }

            hashes = next_level;
        }

        hashes.into_iter().next().unwrap_or_else(|| hash_string("empty"))
    }

    pub fn calculate_hash(&self) -> String {
        self.header.calculate_hash()
    }

    pub fn validate(&self) -> bool {
        let expected_merkle = Self::calculate_merkle_root(&self.transactions);
        if self.header.merkle_root != expected_merkle {
            return false;
        }

        for tx in &self.transactions {
            if !Validator::validate_transaction(tx) {
                return false;
            }
        }

        true
    }
}
