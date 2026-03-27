use crate::types::{Block, Transaction, TransactionData};
use crate::validator::Validator;
use crate::state::State;

/// Blockchain storage - holds the chain of blocks
#[derive(Debug, Clone)]
pub struct Blockchain {
    blocks: Vec<Block>,
}

impl Blockchain {
    /// Creates a new blockchain with genesis block
    pub fn new() -> Self {
        Blockchain {
            blocks: vec![Block::genesis()],
        }
    }

    /// Creates blockchain from existing blocks (for loading from disk)
    pub fn from_blocks(blocks: Vec<Block>) -> Self {
        Blockchain { blocks }
    }

    /// Returns the latest block
    pub fn get_latest_block(&self) -> Option<&Block> {
        self.blocks.last()
    }

    /// Returns the height of the chain
    pub fn height(&self) -> u64 {
        self.blocks.len() as u64
    }

    /// Returns a block at specified height
    pub fn get_block(&self, height: u64) -> Option<&Block> {
        self.blocks.get(height as usize)
    }

    /// Returns all blocks
    pub fn get_blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Returns mutable reference to blocks (for testing/forks)
    pub fn get_blocks_mut(&mut self) -> &mut Vec<Block> {
        &mut self.blocks
    }

    /// Adds a new block to the chain
    /// Returns error if block validation fails
    pub fn add_block(&mut self, block: Block) -> Result<(), String> {
        // Validate the block itself
        if !Validator::validate_block(&block) {
            return Err("Block validation failed".to_string());
        }

        // Check that block connects to the chain
        let latest_block = self
            .get_latest_block()
            .ok_or("Blockchain is empty")?;

        if block.header.prev_hash != latest_block.calculate_hash() {
            return Err(format!(
                "Block prev_hash {} does not match latest block hash {}",
                block.header.prev_hash,
                latest_block.calculate_hash()
            ));
        }

        // Check block height is sequential
        if block.header.index != latest_block.header.index + 1 {
            return Err(format!(
                "Block height {} does not follow latest block height {}",
                block.header.index,
                latest_block.header.index
            ));
        }

        self.blocks.push(block);
        Ok(())
    }

    /// Validates the entire chain
    /// Checks that each block's prev_hash matches the previous block's hash
    pub fn validate_chain(&self) -> bool {
        if self.blocks.is_empty() {
            return false;
        }

        // Validate genesis block
        if !Validator::validate_block(&self.blocks[0]) {
            return false;
        }

        // Validate chain links
        for i in 1..self.blocks.len() {
            let prev_block = &self.blocks[i - 1];
            let current_block = &self.blocks[i];

            // Check prev_hash linkage
            if current_block.header.prev_hash != prev_block.calculate_hash() {
                return false;
            }

            // Check height is sequential
            if current_block.header.index != prev_block.header.index + 1 {
                return false;
            }

            // Validate current block
            if !Validator::validate_block(current_block) {
                return false;
            }
        }

        true
    }

    /// Returns the genesis block
    pub fn genesis(&self) -> &Block {
        &self.blocks[0]
    }

    // =====================================================
    // BALANCE COMPUTATION (from genesis, on-demand)
    // =====================================================

    /// Computes balance for an address by iterating all blocks from genesis
    /// This is the authoritative way to get balance - no cached state
    pub fn get_balance(&self, address: &str) -> u64 {
        let mut balance = 0u64;

        for block in &self.blocks {
            for tx in &block.transactions {
                match &tx.data {
                    TransactionData::Coinbase(data) => {
                        // Coinbase: credit to miner (tx.to)
                        if tx.to.as_ref() == Some(&address.to_string()) {
                            balance += data.reward;
                        }
                    }
                    TransactionData::Transfer(data) => {
                        // Transfer: debit from sender (tx.from), credit to receiver (tx.to)
                        if tx.from.as_ref() == Some(&address.to_string()) {
                            balance = balance.saturating_sub(data.amount);
                        }
                        if tx.to.as_ref() == Some(&address.to_string()) {
                            balance += data.amount;
                        }
                    }
                    TransactionData::FundProject(data) => {
                        // FundProject: debit from backer (tx.from), credit to creator (tx.to)
                        if tx.from.as_ref() == Some(&address.to_string()) {
                            balance = balance.saturating_sub(data.amount);
                        }
                        if tx.to.as_ref() == Some(&address.to_string()) {
                            balance += data.amount;
                        }
                    }
                    TransactionData::CreateProject(_) => {
                        // CreateProject: doesn't affect balance
                    }
                }
            }
        }

        balance
    }

    /// Computes balance including pending transactions from mempool
    pub fn get_balance_with_pending(&self, address: &str, mempool: &crate::Mempool) -> u64 {
        let confirmed_balance = self.get_balance(address);
        let pending_outgoing = mempool.get_pending_outgoing(address);
        confirmed_balance.saturating_sub(pending_outgoing)
    }

    /// Checks if an address can spend a given amount (considering mempool pending)
    pub fn can_spend(&self, address: &str, amount: u64, mempool: &crate::Mempool) -> bool {
        let available = self.get_balance_with_pending(address, mempool);
        available >= amount
    }

    /// Gets project data by ID (computed from blocks)
    pub fn get_project(&self, project_id: &str) -> Option<crate::state::project::Project> {
        for block in &self.blocks {
            for tx in &block.transactions {
                if let TransactionData::CreateProject(data) = &tx.data {
                    if data.project_id == project_id {
                        let mut project = crate::state::project::Project::new(
                            data.project_id.clone(),
                            data.name.clone(),
                            data.description.clone(),
                            data.goal_amount,
                            data.deadline_timestamp,
                            data.creator_wallet.clone(),
                        );

                        // Calculate raised amount from FundProject transactions
                        for inner_block in &self.blocks {
                            for inner_tx in &inner_block.transactions {
                                if let TransactionData::FundProject(fund_data) = &inner_tx.data {
                                    if fund_data.project_id == project_id {
                                        project.raised_amount += fund_data.amount;
                                        if let Some(backer) = &inner_tx.from {
                                            if !project.backers.contains(backer) {
                                                project.backers.push(backer.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        return Some(project);
                    }
                }
            }
        }
        None
    }

    /// Validates a transaction against current chain state (balance check)
    pub fn validate_transaction_state(&self, tx: &Transaction, mempool: &crate::Mempool) -> Result<(), String> {
        // Coinbase doesn't need balance check
        if !tx.requires_signature() {
            return Ok(());
        }

        let from = tx.from.as_ref()
            .ok_or("Transaction must have 'from' address")?;

        let tx_amount = match &tx.data {
            TransactionData::Transfer(data) => data.amount,
            TransactionData::FundProject(data) => data.amount,
            _ => 0,
        };

        // Check balance including pending transactions
        if !self.can_spend(from, tx_amount, mempool) {
            let balance = self.get_balance(from);
            let pending = mempool.get_pending_outgoing(from);
            return Err(format!(
                "Insufficient balance: {} has {} ({} pending), needs {}",
                from, balance, pending, tx_amount + pending
            ));
        }

        // For FundProject, check project exists and deadline not passed
        if let TransactionData::FundProject(data) = &tx.data {
            let project = self.get_project(&data.project_id)
                .ok_or(format!("Project {} not found", data.project_id))?;

            if !project.can_accept_donations(tx.timestamp) {
                return Err(format!("Project {} deadline has passed", data.project_id));
            }
        }

        Ok(())
    }

    /// Applies all transactions from a block to compute state
    /// Returns the computed state (useful for checkpoints in future)
    pub fn compute_state(&self) -> State {
        State::compute_from_blocks(&self.blocks)
            .unwrap_or_else(|_| State::new())
    }
}

impl Default for Blockchain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::KeyPair;
    use crate::types::transaction::*;
    use crate::Mempool;

    fn create_coinbase_transaction(to: &str, reward: u64, height: u64) -> Transaction {
        Transaction::new(
            TransactionType::Coinbase,
            None,
            Some(to.to_string()),
            TransactionData::Coinbase(CoinbaseData {
                reward,
                block_height: height,
            }),
            1234567890,
            None,
        )
    }

    fn create_transfer_transaction(from: &str, to: &str, amount: u64) -> Transaction {
        Transaction::new(
            TransactionType::Transfer,
            Some(from.to_string()),
            Some(to.to_string()),
            TransactionData::Transfer(TransferData {
                amount,
                message: "test".to_string(),
            }),
            1234567890,
            Some("signature".to_string()),
        )
    }

    #[test]
    fn test_blockchain_creation() {
        let chain = Blockchain::new();
        assert_eq!(chain.height(), 1);
        assert!(chain.get_latest_block().is_some());
    }

    #[test]
    fn test_get_balance_from_genesis() {
        let mut chain = Blockchain::new();
        let mempool = Mempool::new();

        // Add block with coinbase
        let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
        let block = Block::new(
            1,
            prev_hash,
            vec![create_coinbase_transaction("alice", 100, 1)],
            0,
        );
        chain.add_block(block).unwrap();

        // Check balance computed from blocks
        assert_eq!(chain.get_balance("alice"), 100);
        assert_eq!(chain.get_balance("bob"), 0);
    }

    #[test]
    fn test_get_balance_with_transfers() {
        let mut chain = Blockchain::new();
        let mempool = Mempool::new();

        // Block 1: Alice gets 100 coins
        let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
        let block1 = Block::new(
            1,
            prev_hash,
            vec![create_coinbase_transaction("alice", 100, 1)],
            0,
        );
        chain.add_block(block1).unwrap();

        // Block 2: Alice sends 30 to Bob
        let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
        let block2 = Block::new(
            2,
            prev_hash,
            vec![
                create_coinbase_transaction("miner", 50, 2),
                create_transfer_transaction("alice", "bob", 30),
            ],
            0,
        );
        chain.add_block(block2).unwrap();

        // Check balances
        assert_eq!(chain.get_balance("alice"), 70);  // 100 - 30
        assert_eq!(chain.get_balance("bob"), 30);
        assert_eq!(chain.get_balance("miner"), 50);
    }

    #[test]
    fn test_can_spend_with_mempool() {
        let mut chain = Blockchain::new();

        // Alice gets 100 coins
        let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
        let block = Block::new(
            1,
            prev_hash,
            vec![create_coinbase_transaction("alice", 100, 1)],
            0,
        );
        chain.add_block(block).unwrap();

        let mut mempool = Mempool::new();

        // Alice can spend 100
        assert!(chain.can_spend("alice", 100, &mempool));

        // Add pending transaction
        let pending_tx = create_transfer_transaction("alice", "bob", 60);
        mempool.add_transaction(pending_tx, &chain).unwrap();

        // Now alice can only spend 40 more (100 - 60 pending)
        assert!(!chain.can_spend("alice", 50, &mempool));
        assert!(chain.can_spend("alice", 40, &mempool));
    }

    #[test]
    fn test_add_block() {
        let mut chain = Blockchain::new();
        let latest_hash = chain.get_latest_block().unwrap().calculate_hash();
        let new_block = Block::new(
            1,
            latest_hash,
            vec![create_coinbase_transaction("miner", 50, 1)],
            0,
        );

        assert!(chain.add_block(new_block).is_ok());
        assert_eq!(chain.height(), 2);
    }

    #[test]
    fn test_validate_chain() {
        let mut chain = Blockchain::new();
        let latest_hash = chain.get_latest_block().unwrap().calculate_hash();
        let new_block = Block::new(
            1,
            latest_hash,
            vec![create_coinbase_transaction("miner", 50, 1)],
            0,
        );

        chain.add_block(new_block).unwrap();
        assert!(chain.validate_chain());
    }

    #[test]
    fn test_validate_chain_tampered() {
        let mut chain = Blockchain::new();
        let latest_hash = chain.get_latest_block().unwrap().calculate_hash();
        let mut new_block = Block::new(
            1,
            latest_hash,
            vec![create_coinbase_transaction("miner", 50, 1)],
            0,
        );

        // Tamper with coinbase reward
        if let TransactionData::Coinbase(ref mut data) = new_block.transactions[0].data {
            data.reward = 999999;
            new_block.transactions[0].id = new_block.transactions[0].calculate_id();
        }
        // Don't recalculate merkle root - simulates tampering

        chain.blocks.push(new_block);
        assert!(!chain.validate_chain());
    }

    #[test]
    fn test_validate_chain_broken_link() {
        let mut chain = Blockchain::new();
        let bad_block = Block::new(
            1,
            "wrong_hash".to_string(),
            vec![create_coinbase_transaction("miner", 50, 1)],
            0,
        );
        chain.blocks.push(bad_block);
        assert!(!chain.validate_chain());
    }
}
