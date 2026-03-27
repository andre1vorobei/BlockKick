//! Integration tests for Stage 2: State & Chain
//! 
//! Критерий готовности:
//! - Можно создать цепочку из 5 блоков вручную
//! - Если изменить транзакцию в 3-м блоке, валидация цепи падает
//! - Балансы кошельков корректно вычисляются из блоков
//! - Мемпул правильно проверяет балансы с учётом pending

use BlockKick::{
    Blockchain, Mempool,
    types::{Block, Transaction, TransactionData, TransactionType},
    types::transaction::{CoinbaseData, CreateProjectData, FundProjectData, TransferData},
};

/// Создаёт coinbase транзакцию для награды майнеру
fn create_coinbase(miner: &str, reward: u64, block_height: u64) -> Transaction {
    Transaction::new(
        TransactionType::Coinbase,
        None,
        Some(miner.to_string()),
        TransactionData::Coinbase(CoinbaseData {
            reward,
            block_height,
        }),
        1234567890,
        None,
    )
}

/// Создаёт транзакцию перевода
fn create_transfer(from: &str, to: &str, amount: u64) -> Transaction {
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

/// Создаёт транзакцию создания проекта
fn create_project(creator: &str, project_id: &str) -> Transaction {
    Transaction::new(
        TransactionType::CreateProject,
        Some(creator.to_string()),
        None,
        TransactionData::CreateProject(CreateProjectData {
            project_id: project_id.to_string(),
            name: "Test Project".to_string(),
            description: "Integration test project".to_string(),
            goal_amount: 1000,
            deadline_timestamp: 9999999999,
            creator_wallet: creator.to_string(),
        }),
        1234567890,
        Some("signature".to_string()),
    )
}

/// Создаёт транзакцию взноса в проект
fn create_fund_project(backer: &str, creator: &str, project_id: &str, amount: u64) -> Transaction {
    Transaction::new(
        TransactionType::FundProject,
        Some(backer.to_string()),
        Some(creator.to_string()),
        TransactionData::FundProject(FundProjectData {
            project_id: project_id.to_string(),
            amount,
            backer_note: "Support!".to_string(),
        }),
        1234567890,
        Some("signature".to_string()),
    )
}

#[test]
fn test_create_5_block_chain() {
    // Создаём блокчейн с genesis блоком
    let mut chain = Blockchain::new();
    assert_eq!(chain.height(), 1);

    // Блок 1: Coinbase для Alice (100 coins)
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block1 = Block::new(
        1,
        prev_hash,
        vec![create_coinbase("alice", 100, 1)],
        0,
    );
    chain.add_block(block1).unwrap();
    assert_eq!(chain.height(), 2);

    // Блок 2: Coinbase для Bob (100 coins) + Transfer Alice -> Charlie (30 coins)
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block2 = Block::new(
        2,
        prev_hash,
        vec![
            create_coinbase("bob", 100, 2),
            create_transfer("alice", "charlie", 30),
        ],
        0,
    );
    chain.add_block(block2).unwrap();
    assert_eq!(chain.height(), 3);

    // Блок 3: Create Project + Fund Project
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block3 = Block::new(
        3,
        prev_hash,
        vec![
            create_coinbase("dave", 50, 3),
            create_project("eve", "proj_game"),
            create_fund_project("bob", "eve", "proj_game", 25),
        ],
        0,
    );
    chain.add_block(block3).unwrap();
    assert_eq!(chain.height(), 4);

    // Блок 4: Multiple transfers
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block4 = Block::new(
        4,
        prev_hash,
        vec![
            create_coinbase("frank", 75, 4),
            create_transfer("charlie", "alice", 10),
            create_transfer("bob", "eve", 20),
        ],
        0,
    );
    chain.add_block(block4).unwrap();
    assert_eq!(chain.height(), 5);

    // Блок 5: Final coinbase
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block5 = Block::new(
        5,
        prev_hash,
        vec![create_coinbase("grace", 60, 5)],
        0,
    );
    chain.add_block(block5).unwrap();
    assert_eq!(chain.height(), 6); // genesis + 5 blocks

    // Валидация всей цепи
    assert!(chain.validate_chain(), "Chain should be valid");
}

#[test]
fn test_balance_computation_from_genesis() {
    let mut chain = Blockchain::new();

    // Блок 1: Alice получает 100 монет
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block1 = Block::new(
        1,
        prev_hash,
        vec![create_coinbase("alice", 100, 1)],
        0,
    );
    chain.add_block(block1).unwrap();

    // Баланс вычисляется из блоков
    assert_eq!(chain.get_balance("alice"), 100);
    assert_eq!(chain.get_balance("bob"), 0);

    // Блок 2: Alice отправляет 30 Bob
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block2 = Block::new(
        2,
        prev_hash,
        vec![
            create_coinbase("miner", 50, 2),
            create_transfer("alice", "bob", 30),
        ],
        0,
    );
    chain.add_block(block2).unwrap();

    // Балансы пересчитываются
    assert_eq!(chain.get_balance("alice"), 70);  // 100 - 30
    assert_eq!(chain.get_balance("bob"), 30);
    assert_eq!(chain.get_balance("miner"), 50);
}

#[test]
fn test_tamper_block_invalidates_chain() {
    // Создаём цепочку из 5 блоков вручную
    let genesis = Block::genesis();
    
    // Блок 1
    let block1 = Block::new(
        1,
        genesis.calculate_hash(),
        vec![create_coinbase("miner_1", 50, 1)],
        0,
    );
    
    // Блок 2
    let block2 = Block::new(
        2,
        block1.calculate_hash(),
        vec![create_coinbase("miner_2", 50, 2)],
        0,
    );
    
    // Блок 3
    let block3 = Block::new(
        3,
        block2.calculate_hash(),
        vec![create_coinbase("miner_3", 50, 3)],
        0,
    );
    
    // Блок 4
    let block4 = Block::new(
        4,
        block3.calculate_hash(),
        vec![create_coinbase("miner_4", 50, 4)],
        0,
    );
    
    // Блок 5
    let block5 = Block::new(
        5,
        block4.calculate_hash(),
        vec![create_coinbase("miner_5", 50, 5)],
        0,
    );
    
    // Создаём цепочку
    let mut chain = Blockchain::from_blocks(vec![
        genesis,
        block1,
        block2,
        block3,
        block4,
        block5,
    ]);

    assert_eq!(chain.height(), 6);
    assert!(chain.validate_chain(), "Chain should be valid before tampering");

    // "Взламываем" транзакцию в 3-м блоке (индекс 3, т.к. 0 = genesis)
    let blocks = chain.get_blocks();
    let mut tampered_block = blocks[3].clone();
    
    if let TransactionData::Coinbase(ref mut data) = tampered_block.transactions[0].data {
        data.reward = 999999;
        tampered_block.transactions[0].id = tampered_block.transactions[0].calculate_id();
    }
    // НЕ пересчитываем merkle root - это симуляция атаки

    // Заменяем блок в цепочке
    chain.get_blocks_mut()[3] = tampered_block;

    // Валидация цепи должна упасть
    assert!(!chain.validate_chain(), "Chain should be invalid after tampering");
}

#[test]
fn test_mempool_with_balance_check() {
    let mut chain = Blockchain::new();
    let mut mempool = Mempool::new();

    // Alice получает 100 монет
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block = Block::new(
        1,
        prev_hash,
        vec![create_coinbase("alice", 100, 1)],
        0,
    );
    chain.add_block(block).unwrap();

    // Проверяем баланс
    assert_eq!(chain.get_balance("alice"), 100);

    // Добавляем транзакцию в мемпул
    let tx1 = create_transfer("alice", "bob", 30);
    assert!(mempool.add_transaction(tx1, &chain).is_ok());
    assert_eq!(mempool.len(), 1);

    // Вторая транзакция должна учитывать pending
    let tx2 = create_transfer("alice", "charlie", 50);
    assert!(mempool.add_transaction(tx2, &chain).is_ok());  // 100 - 30 - 50 = 20 >= 0

    // Третья транзакция должна отклониться (недостаточно баланса)
    let tx3 = create_transfer("alice", "dave", 50);
    assert!(mempool.add_transaction(tx3, &chain).is_err());  // 100 - 30 - 50 - 50 = -30 < 0

    assert_eq!(mempool.len(), 2);
}

#[test]
fn test_can_spend_with_pending() {
    let mut chain = Blockchain::new();
    let mut mempool = Mempool::new();

    // Alice получает 100 монет
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block = Block::new(
        1,
        prev_hash,
        vec![create_coinbase("alice", 100, 1)],
        0,
    );
    chain.add_block(block).unwrap();

    // Alice может потратить 100
    assert!(chain.can_spend("alice", 100, &mempool));

    // Добавляем pending транзакцию
    let tx = create_transfer("alice", "bob", 60);
    mempool.add_transaction(tx, &chain).unwrap();

    // Теперь Alice может потратить только 40 (100 - 60 pending)
    assert!(!chain.can_spend("alice", 50, &mempool));
    assert!(chain.can_spend("alice", 40, &mempool));

    // Баланс с pending
    assert_eq!(chain.get_balance_with_pending("alice", &mempool), 40);
}

#[test]
fn test_project_computation() {
    let mut chain = Blockchain::new();

    // Блок 1: Создаём проект
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block1 = Block::new(
        1,
        prev_hash,
        vec![
            create_coinbase("alice", 100, 1),
            create_project("alice", "proj_test"),
        ],
        0,
    );
    chain.add_block(block1).unwrap();

    // Проверяем проект
    let project = chain.get_project("proj_test").unwrap();
    assert_eq!(project.name, "Test Project");
    assert_eq!(project.raised_amount, 0);

    // Блок 2: Взнос в проект
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block2 = Block::new(
        2,
        prev_hash,
        vec![
            create_coinbase("bob", 200, 2),
            create_fund_project("bob", "alice", "proj_test", 50),
        ],
        0,
    );
    chain.add_block(block2).unwrap();

    // Проверяем проект после взноса
    let project = chain.get_project("proj_test").unwrap();
    assert_eq!(project.raised_amount, 50);
    assert!(project.backers.contains(&"bob".to_string()));
}
