//! State module - упрощённая версия для checkpoint'ов
//! 
//! Балансы вычисляются из блокчейна (get_balance), 
//! этот модуль хранит только данные проектов и используется для future checkpoints.

use std::collections::HashMap;
use crate::types::{Transaction, TransactionData};
use crate::state::project::Project;

/// Snapshot состояния для checkpoint'ов (будущая оптимизация)
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    /// Балансы на момент snapshot (кэш для ускорения)
    pub balances: HashMap<String, u64>,
    /// Проекты на момент snapshot
    pub projects: HashMap<String, Project>,
}

/// State - используется для checkpoint'ов и вычисления состояния
/// НЕ хранится в Blockchain постоянно, вычисляется по запросу
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Балансы адресов
    pub balances: HashMap<String, u64>,
    /// Проекты
    pub projects: HashMap<String, Project>,
}

impl State {
    /// Создаёт пустое состояние
    pub fn new() -> Self {
        State {
            balances: HashMap::new(),
            projects: HashMap::new(),
        }
    }

    /// Вычисляет состояние из всех блоков
    /// Это авторитетный способ получить состояние
    pub fn compute_from_blocks(blocks: &[crate::types::Block]) -> Result<Self, String> {
        let mut state = State::new();

        for block in blocks {
            let block_time = block.header.timestamp;
            
            for tx in &block.transactions {
                match &tx.data {
                    TransactionData::Coinbase(data) => {
                        if let Some(to) = &tx.to {
                            let balance = state.balances.entry(to.clone()).or_insert(0);
                            *balance += data.reward;
                        }
                    }
                    TransactionData::Transfer(data) => {
                        if let Some(from) = &tx.from {
                            let balance = state.balances.entry(from.clone()).or_insert(0);
                            *balance = balance.saturating_sub(data.amount);
                        }
                        if let Some(to) = &tx.to {
                            let balance = state.balances.entry(to.clone()).or_insert(0);
                            *balance += data.amount;
                        }
                    }
                    TransactionData::FundProject(data) => {
                        if let Some(from) = &tx.from {
                            let balance = state.balances.entry(from.clone()).or_insert(0);
                            *balance = balance.saturating_sub(data.amount);
                        }
                        if let Some(to) = &tx.to {
                            let balance = state.balances.entry(to.clone()).or_insert(0);
                            *balance += data.amount;
                        }
                    }
                    TransactionData::CreateProject(data) => {
                        let project = Project::new(
                            data.project_id.clone(),
                            data.name.clone(),
                            data.description.clone(),
                            data.goal_amount,
                            data.deadline_timestamp,
                            data.creator_wallet.clone(),
                        );
                        state.projects.insert(data.project_id.clone(), project);
                    }
                }
            }
            
            // Обновляем raised_amount для проектов
            for tx in &block.transactions {
                if let TransactionData::FundProject(data) = &tx.data {
                    if let Some(project) = state.projects.get_mut(&data.project_id) {
                        project.raised_amount += data.amount;
                        if let Some(backer) = &tx.from {
                            if !project.backers.contains(backer) {
                                project.backers.push(backer.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(state)
    }

    /// Создаёт snapshot текущего состояния
    pub fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            balances: self.balances.clone(),
            projects: self.projects.clone(),
        }
    }

    /// Получает баланс
    pub fn get_balance(&self, address: &str) -> u64 {
        *self.balances.get(address).unwrap_or(&0)
    }

    /// Получает проект
    pub fn get_project(&self, project_id: &str) -> Option<&Project> {
        self.projects.get(project_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::block::Block;
    use crate::types::transaction::*;

    fn create_coinbase(to: &str, reward: u64, height: u64) -> Transaction {
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

    #[test]
    fn test_compute_from_blocks() {
        let mut blocks = vec![Block::genesis()];

        // Block 1: Alice gets 100 coins
        let block1 = Block::new(
            1,
            blocks.last().unwrap().calculate_hash(),
            vec![create_coinbase("alice", 100, 1)],
            0,
        );
        blocks.push(block1);

        // Block 2: Bob gets 50 coins
        let block2 = Block::new(
            2,
            blocks.last().unwrap().calculate_hash(),
            vec![create_coinbase("bob", 50, 2)],
            0,
        );
        blocks.push(block2);

        let state = State::compute_from_blocks(&blocks).unwrap();

        assert_eq!(state.get_balance("alice"), 100);
        assert_eq!(state.get_balance("bob"), 50);
    }

    #[test]
    fn test_snapshot() {
        let mut state = State::new();
        state.balances.insert("alice".to_string(), 100);

        let snapshot = state.snapshot();
        assert_eq!(snapshot.balances.get("alice"), Some(&100));
    }
}
