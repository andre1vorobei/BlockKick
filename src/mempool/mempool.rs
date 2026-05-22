use crate::Blockchain;
use crate::types::{Transaction, TransactionData, TransactionType};
use crate::validator::Validator;

/// Mempool - holds pending transactions waiting to be included in a block
#[derive(Debug, Clone, Default)]
pub struct Mempool {
    transactions: Vec<Transaction>,
}

impl Mempool {
    /// Creates a new empty mempool
    pub fn new() -> Self {
        Mempool {
            transactions: Vec::new(),
        }
    }

    pub fn remove_transaction(&mut self, tx_id: &str) -> bool {
        if let Some(pos) = self.transactions.iter().position(|t| t.id == tx_id) {
            self.transactions.remove(pos);
            true
        } else {
            false
        }
    }

    /// Creates mempool from existing transactions
    pub fn from_transactions(transactions: Vec<Transaction>) -> Self {
        Mempool { transactions }
    }

    /// Adds a transaction to the mempool
    /// Validates signature and checks balance against blockchain
    pub fn add_transaction(&mut self, tx: Transaction, chain: &Blockchain) -> Result<(), String> {
        // Validate transaction ID
        if tx.id != tx.calculate_id() {
            return Err("Invalid transaction ID".to_string());
        }

        // Validate signature for non-coinbase transactions
        if tx.requires_signature() {
            if tx.signature.is_none() {
                return Err("Transaction requires signature".to_string());
            }

            // Validate signature using validator (currently a stub)
            if !Validator::validate_transaction(&tx) {
                return Err("Invalid signature".to_string());
            }
        }

        // Check for duplicate transaction
        if self.transactions.iter().any(|t| t.id == tx.id) {
            return Err("Transaction already in mempool".to_string());
        }

        // Check balance for non-coinbase transactions
        if !matches!(tx.tx_type, TransactionType::Coinbase) {
            if let Some(from) = &tx.from {
                // Calculate total pending outgoing from mempool
                let pending_outgoing: u64 = self
                    .transactions
                    .iter()
                    .filter(|t| t.from.as_ref() == Some(from))
                    .filter_map(|t| match &t.data {
                        TransactionData::Transfer(data) => Some(data.amount),
                        TransactionData::FundProject(data) => Some(data.amount),
                        _ => None,
                    })
                    .sum();

                // Calculate amount in this transaction
                let tx_amount = match &tx.data {
                    TransactionData::Transfer(data) => data.amount,
                    TransactionData::FundProject(data) => data.amount,
                    _ => 0,
                };

                // Get confirmed balance from blockchain
                let confirmed_balance = chain.get_balance(from);
                let total_needed = pending_outgoing + tx_amount;

                if confirmed_balance < total_needed {
                    return Err(format!(
                        "Insufficient balance: {} has {} ({} pending), needs {}",
                        from, confirmed_balance, pending_outgoing, tx_amount
                    ));
                }

                // For FundProject, check project exists and deadline
                if let TransactionData::FundProject(data) = &tx.data {
                    if let Some(project) = chain.get_project(&data.project_id) {
                        if !project.can_accept_donations(tx.timestamp) {
                            return Err(format!("Project {} deadline has passed", data.project_id));
                        }
                    } else {
                        return Err(format!("Project {} not found", data.project_id));
                    }
                }
            }
        }

        self.transactions.push(tx);
        Ok(())
    }

    /// Removes multiple transactions by ID
    pub fn remove_transactions(&mut self, tx_ids: &[String]) {
        self.transactions.retain(|t| !tx_ids.contains(&t.id));
    }

    /// Gets all transactions in the mempool
    pub fn get_transactions(&self) -> &[Transaction] {
        &self.transactions
    }

    /// Gets transactions for inclusion in a block
    pub fn get_transactions_for_block(&self, max_count: usize) -> Vec<Transaction> {
        self.transactions.iter().take(max_count).cloned().collect()
    }

    /// Gets the number of pending transactions
    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    pub fn clear(&mut self) {
        self.transactions.clear();
    }

    /// Gets total pending outgoing amount for an address
    pub fn get_pending_outgoing(&self, address: &str) -> u64 {
        self.transactions
            .iter()
            .filter(|t| t.from.as_ref() == Some(&address.to_string()))
            .filter_map(|t| match &t.data {
                TransactionData::Transfer(data) => Some(data.amount),
                TransactionData::FundProject(data) => Some(data.amount),
                _ => None,
            })
            .sum()
    }

    /// Prunes invalid transactions from mempool (e.g., after a block is added)
    pub fn prune_invalid(&mut self, chain: &Blockchain) {
        let mut valid = Vec::new();
        let mut pending_per_address: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();

        for tx in &self.transactions {
            let is_valid = if !tx.requires_signature() {
                // Coinbase is always valid
                true
            } else if let Some(from) = &tx.from {
                // Validate ID
                if tx.id != tx.calculate_id() {
                    false
                } else {
                    // Check balance with running pending total
                    let tx_amount = match &tx.data {
                        TransactionData::Transfer(data) => data.amount,
                        TransactionData::FundProject(data) => data.amount,
                        _ => 0,
                    };

                    let pending = *pending_per_address.get(from).unwrap_or(&0);
                    let confirmed_balance = chain.get_balance(from);

                    if confirmed_balance >= pending + tx_amount {
                        // Update pending for this address
                        *pending_per_address.entry(from.clone()).or_insert(0) += tx_amount;
                        true
                    } else {
                        false
                    }
                }
            } else {
                false
            };

            if is_valid {
                valid.push(tx.clone());
            }
        }

        self.transactions = valid;
    }

    /// Проверяет наличие транзакции в мемпуле
    pub fn is_in_mempool(&self, tx_id: &str) -> bool {
        self.transactions.iter().any(|t| t.id == tx_id)
    }
}

#[cfg(test)]
mod tests {
    // TODO: Надо сделать ебаное подписание транзакций в тестах после включения ебаной верификации
    // use super::*;
    // use crate::crypto::sign_data;
    // use crate::types::block::Block;
    // use crate::types::transaction::*;

    // fn create_coinbase(to: &str, reward: u64, height: u64) -> Transaction {
    //     Transaction::new(
    //         TransactionType::Coinbase,
    //         None,
    //         Some(to.to_string()),
    //         TransactionData::Coinbase(CoinbaseData {
    //             reward,
    //             block_height: height,
    //         }),
    //         1234567890,
    //     )
    // }

    // fn create_transfer(from: &str, to: &str, amount: u64) -> Transaction {
    //     let mut tx = Transaction::new(
    //         TransactionType::Transfer,
    //         Some(from.to_string()),
    //         Some(to.to_string()),
    //         TransactionData::Transfer(TransferData {
    //             amount,
    //             message: "test".to_string(),
    //         }),
    //         1234567890,
    //     );
    //     tx.signature = sign_data(tx);
    // }

    // #[test]
    // fn test_mempool_new() {
    //     let mempool = Mempool::new();
    //     assert!(mempool.is_empty());
    //     assert_eq!(mempool.len(), 0);
    // }

    // #[test]
    // fn test_add_coinbase_transaction() {
    //     let mut mempool = Mempool::new();
    //     let chain = Blockchain::new();

    //     let tx = create_coinbase("miner", 50, 1);
    //     assert!(mempool.add_transaction(tx, &chain).is_ok());
    //     assert_eq!(mempool.len(), 1);
    // }

    // #[test]
    // fn test_add_transaction_insufficient_balance() {
    //     let mut mempool = Mempool::new();
    //     let mut chain = Blockchain::new();

    //     // Add a block so chain isn't empty
    //     let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    //     let block = Block::new(1, prev_hash, vec![], 0);
    //     chain.add_block(block).unwrap();

    //     // Try to add transfer from alice who has 0 balance
    //     let tx = create_transfer("alice", "bob", 100);
    //     assert!(mempool.add_transaction(tx, &chain).is_err());
    // }

    // #[test]
    // fn test_add_duplicate_transaction() {
    //     let mut mempool = Mempool::new();
    //     let chain = Blockchain::new();

    //     let tx = create_coinbase("miner", 50, 1);
    //     let tx_id = tx.id.clone();
    //     mempool.add_transaction(tx, &chain).unwrap();

    //     // Try to add same transaction again
    //     let mut tx2 = create_coinbase("miner", 100, 2);
    //     tx2.id = tx_id; // Force same ID
    //     assert!(mempool.add_transaction(tx2, &chain).is_err());
    // }

    // #[test]
    // fn test_remove_transaction() {
    //     let mut mempool = Mempool::new();
    //     let chain = Blockchain::new();

    //     let tx = create_coinbase("miner", 50, 1);
    //     let tx_id = tx.id.clone();
    //     mempool.add_transaction(tx, &chain).unwrap();

    //     assert!(mempool.remove_transaction(&tx_id));
    //     assert!(mempool.is_empty());
    // }

    // #[test]
    // fn test_get_transactions_for_block() {
    //     let mut mempool = Mempool::new();
    //     let chain = Blockchain::new();

    //     for i in 0..5 {
    //         let tx = create_coinbase(&format!("miner{}", i), 50, i as u64 + 1);
    //         mempool.add_transaction(tx, &chain).unwrap();
    //     }

    //     let txs = mempool.get_transactions_for_block(3);
    //     assert_eq!(txs.len(), 3);
    // }

    // #[test]
    // fn test_clear_mempool() {
    //     let mut mempool = Mempool::new();
    //     let chain = Blockchain::new();

    //     for i in 0..3 {
    //         let tx = create_coinbase(&format!("miner{}", i), 50, i as u64 + 1);
    //         mempool.add_transaction(tx, &chain).unwrap();
    //     }

    //     mempool.clear();
    //     assert!(mempool.is_empty());
    // }

    // #[test]
    // fn test_get_pending_outgoing() {
    //     let mut mempool = Mempool::new();
    //     let mut chain = Blockchain::new();

    //     // Give alice 100 coins
    //     let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    //     let block = Block::new(1, prev_hash, vec![create_coinbase("alice", 100, 1)], 0);
    //     chain.add_block(block).unwrap();

    //     let tx1 = create_transfer("alice", "bob", 30);
    //     let tx2 = create_transfer("alice", "charlie", 20);
    //     mempool.add_transaction(tx1, &chain).unwrap();
    //     mempool.add_transaction(tx2, &chain).unwrap();

    //     assert_eq!(mempool.get_pending_outgoing("alice"), 50);
    //     assert_eq!(mempool.get_pending_outgoing("bob"), 0);
    // }

    // #[test]
    // fn test_prune_invalid() {
    //     let mut mempool = Mempool::new();
    //     let mut chain = Blockchain::new();

    //     // Give alice 100 coins
    //     let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    //     let block = Block::new(1, prev_hash, vec![create_coinbase("alice", 100, 1)], 0);
    //     chain.add_block(block).unwrap();

    //     // Add transactions
    //     let tx1 = create_transfer("alice", "bob", 30); // Valid
    //     let tx2 = create_transfer("alice", "charlie", 40); // Valid (100 - 30 - 40 = 30 remaining)
    //     let tx3 = create_transfer("alice", "dave", 50); // Invalid (not enough after tx1 + tx2)

    //     mempool.add_transaction(tx1, &chain).unwrap();
    //     mempool.add_transaction(tx2, &chain).unwrap();
    //     mempool.add_transaction(tx3, &chain).unwrap_err(); // Should fail

    //     // Prune shouldn't remove anything since tx3 was already rejected
    //     mempool.prune_invalid(&chain);
    //     assert_eq!(mempool.len(), 2);
    // }
}
