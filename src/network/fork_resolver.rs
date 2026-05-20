use crate::storage::Blockchain;
use crate::types::Block;
use std::sync::{Arc, Mutex};

/// Результат обработки блока
#[derive(Debug, Clone, PartialEq)]
pub enum ForkResult {
    AcceptedMainChain,
    AcceptedFork,
    Rejected(String),
    Reorganized {
        old_tip: String,
        new_tip: String,
        reverted: u64,
        applied: u64,
    },
}

/// Менеджер разрешения форков
pub struct ForkResolver {
    blockchain: Arc<Mutex<Blockchain>>,
    orphan_pool: std::collections::HashMap<String, Vec<Block>>,
}

impl ForkResolver {
    pub fn new(blockchain: Arc<Mutex<Blockchain>>) -> Self {
        Self {
            blockchain,
            orphan_pool: std::collections::HashMap::new(),
        }
    }

    /// Обработать полученный блок
    pub fn handle_block(&mut self, block: Block) -> ForkResult {
        let block_hash = block.calculate_hash();
        let parent_hash = block.header.prev_hash.clone();

        if !self.validate_basic(&block) {
            return ForkResult::Rejected("Basic validation failed".into());
        }

        let chain = self.blockchain.lock().unwrap();

        // ← ИСПРАВЛЕНО: используем существующие методы Blockchain
        if Self::chain_has_parent(&chain, &parent_hash) {
            drop(chain);
            return self.try_add_block(block, &block_hash, &parent_hash);
        }

        drop(chain);
        self.handle_orphan(block, &parent_hash)
    }

    /// Вспомогательная: проверить наличие родителя в цепи
    /// ← ИСПРАВЛЕНО: заменяем chain.has_block() на существующий метод
    fn chain_has_parent(chain: &Blockchain, parent_hash: &str) -> bool {
        // Проверяем: есть ли блок с таким хешем в цепи
        chain
            .get_blocks()
            .iter()
            .any(|b| b.calculate_hash() == parent_hash)
    }

    /// Попытка добавить блок в цепь
    fn try_add_block(&mut self, block: Block, block_hash: &str, parent_hash: &str) -> ForkResult {
        let mut chain = self.blockchain.lock().unwrap();
        let current_tip = match chain.get_latest_block() {
            Some(b) => b.calculate_hash(),
            None => return ForkResult::Rejected("Empty chain".into()),
        };

        if parent_hash == &current_tip {
            match chain.add_block(block.clone()) {
                Ok(_) => {
                    drop(chain);
                    self.process_orphans(block_hash);
                    ForkResult::AcceptedMainChain
                }
                Err(e) => ForkResult::Rejected(format!("Add failed: {}", e)),
            }
        } else if Self::chain_has_parent(&chain, parent_hash) {
            // Форк — родитель есть, но не последний
            drop(chain);
            self.handle_fork(block, block_hash, parent_hash)
        } else {
            ForkResult::Rejected("Parent not in chain".into())
        }
    }

    /// Обработка форка
    fn handle_fork(&mut self, block: Block, block_hash: &str, parent_hash: &str) -> ForkResult {
        let chain = self.blockchain.lock().unwrap();
        let main_height = chain.height();

        // ← ИСПРАВЛЕНО: заменяем get_block_height() на существующий метод
        let fork_height = chain
            .get_blocks()
            .iter()
            .position(|b| b.calculate_hash() == parent_hash)
            .map(|h| h as u64 + 1)
            .unwrap_or(0);

        if fork_height > main_height {
            drop(chain);
            match self.reorganize(block, block_hash, parent_hash) {
                Ok((rev, app)) => ForkResult::Reorganized {
                    old_tip: "old".into(),
                    new_tip: block_hash.into(),
                    reverted: rev,
                    applied: app,
                },
                Err(e) => ForkResult::Rejected(format!("Reorg failed: {}", e)),
            }
        } else {
            ForkResult::AcceptedFork
        }
    }

    /// Реорганизация (упрощённая)
    fn reorganize(
        &self,
        new_tip: Block,
        _new_tip_hash: &str,
        _parent_hash: &str,
    ) -> Result<(u64, u64), String> {
        // Упрощённо: просто добавляем блок если валиден
        let mut chain = self.blockchain.lock().unwrap();
        chain.add_block(new_tip)?;
        Ok((0, 1))
    }

    /// Обработка орфан-блока
    fn handle_orphan(&mut self, block: Block, parent_hash: &str) -> ForkResult {
        self.orphan_pool
            .entry(parent_hash.to_string())
            .or_default()
            .push(block);
        ForkResult::Rejected(format!("Orphan (parent {} unknown)", parent_hash))
    }

    /// Обработать появление нового tip: проверить орфаны
    fn process_orphans(&mut self, new_tip_hash: &str) {
        if let Some(orphans) = self.orphan_pool.remove(new_tip_hash) {
            for orphan in orphans {
                self.handle_block(orphan);
            }
        }
    }

    /// Базовая валидация блока
    fn validate_basic(&self, block: &Block) -> bool {
        use crate::types::{TransactionData, TransactionType};

        if block.transactions.is_empty() {
            return false;
        }
        if block.transactions[0].tx_type != TransactionType::Coinbase {
            return false;
        }
        if let TransactionData::Coinbase(data) = &block.transactions[0].data {
            if data.reward != crate::consensus::BLOCK_REWARD {
                return false;
            }
        } else {
            return false;
        }
        let expected = crate::types::Block::calculate_merkle_root(&block.transactions);
        block.header.merkle_root == expected
    }
}
