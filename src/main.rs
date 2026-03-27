use BlockKick::{
    types::transaction::{CoinbaseData, CreateProjectData, FundProjectData, TransferData},
    types::{Block, Transaction, TransactionData, TransactionType},
    Blockchain, Mempool,
};

fn main() {
    println!("1. Создание блокчейна с Genesis блоком...");
    let mut chain = Blockchain::new();
    println!("Genesis блок создан");
    println!("Высота цепи: {}\n", chain.height());

    println!("2. Майнинг Блока 1 (Coinbase для Alice и Bob)...");
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block1 = Block::new(
        1,
        prev_hash,
        vec![
            create_coinbase("alice", 100, 1),
            create_coinbase("bob", 100, 1),
        ],
        0,
    );
    chain.add_block(block1).unwrap();
    println!("Блок 1 добавлен в цепь");
    println!("Баланс Alice: {} монет", chain.get_balance("alice"));
    println!("Баланс Bob: {} монет\n", chain.get_balance("bob"));

    println!("3. Блок 2: Перевод средств...");
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block2 = Block::new(
        2,
        prev_hash,
        vec![
            create_coinbase("miner", 50, 2),
            create_transfer("alice", "charlie", 30),
        ],
        0,
    );
    chain.add_block(block2).unwrap();
    println!("Блок 2 добавлен в цепь");
    println!(
        "Баланс Alice: {} монет (100 - 30)",
        chain.get_balance("alice")
    );
    println!("Баланс Charlie: {} монет", chain.get_balance("charlie"));
    println!(
        "Баланс Bob: {} монет (без изменений)\n",
        chain.get_balance("bob")
    );

    println!("4. Блок 3: Создание краудфандингового проекта...");
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block3 = Block::new(
        3,
        prev_hash,
        vec![
            create_coinbase("eve", 50, 3),
            create_project("eve", "proj_rust_game", "Open Source RPG Game", 500),
        ],
        0,
    );
    chain.add_block(block3).unwrap();
    println!("Блок 3 добавлен в цепь");
    println!("Проект создан: proj_rust_game");
    println!("Создатель: eve");
    println!("Цель: 500 монет\n");

    println!("5. Блок 4: Взнос в проект (Fund Project)...");
    let prev_hash = chain.get_latest_block().unwrap().calculate_hash();
    let block4 = Block::new(
        4,
        prev_hash,
        vec![
            create_coinbase("dave", 200, 4),
            create_fund_project("bob", "eve", "proj_rust_game", 100),
        ],
        0,
    );
    chain.add_block(block4).unwrap();
    println!("Блок 4 добавлен в цепь");
    println!("Bob внёс 100 монет в proj_rust_game");
    println!("Баланс Bob: {} монет (100 - 100)", chain.get_balance("bob"));
    println!(
        "Баланс Eve: {} монет (+100 от взноса)",
        chain.get_balance("eve")
    );

    if let Some(project) = chain.get_project("proj_rust_game") {
        println!(
            "Собрано: {} / {} монет",
            project.raised_amount, project.goal_amount
        );
        println!("Бэкеры: {:?}\n", project.backers);
    }

    println!("6. Валидация цепи...");
    println!("Высота цепи: {} блоков", chain.height());
    println!(
        "Валидация: {}",
        if chain.validate_chain() {
            "SUCCSESS"
        } else {
            "ERROR"
        }
    );
    println!(
        "Статус цепи: {}\n",
        if chain.validate_chain() {
            "Все блоки связаны корректно"
        } else {
            "Обнаружены нарушения"
        }
    );

    println!("7. Демонстрация вычисления баланса с нуля...");
    println!("Запрос баланса для 'alice'...");
    let start = std::time::Instant::now();
    let balance = chain.get_balance("alice");
    let elapsed = start.elapsed();
    println!("Баланс: {} монет", balance);
    println!("Время вычисления: {:?}", elapsed);
    println!(
        "(Баланс вычислен проходом по всем {} блокам)\n",
        chain.height()
    );

    println!("8. Мемпул: Проверка pending транзакций...");
    let mut mempool = Mempool::new();

    let tx1 = create_transfer("charlie", "frank", 10);
    match mempool.add_transaction(tx1, &chain) {
        Ok(_) => println!("Транзакция charlie -> frank (10 монет) добавлена в мемпул"),
        Err(e) => println!("Ошибка: {}", e),
    }

    let tx2 = create_transfer("charlie", "grace", 25);
    match mempool.add_transaction(tx2, &chain) {
        Ok(_) => println!("Транзакция charlie -> grace (25 монет) добавлена в мемпул"),
        Err(e) => println!("Отклонено: {}", e),
    }

    println!("Размер мемпула: {} транзакций", mempool.len());
    println!(
        "Ожидание отправки для charlie: {} монет",
        mempool.get_pending_outgoing("charlie")
    );
    println!(
        "Доступно для charlie: {} монет\n",
        chain.get_balance_with_pending("charlie", &mempool)
    );

    println!("9. Проверка возможности траты (can_spend)...");
    println!(
        "Может ли alice потратить 80 монет? {}",
        if chain.can_spend("alice", 80, &Mempool::new()) {
            "Да"
        } else {
            "Нет"
        }
    );
    println!(
        "Может ли alice потратить 100 монет? {}",
        if chain.can_spend("alice", 100, &Mempool::new()) {
            "Да"
        } else {
            "Нет"
        }
    );
    println!(
        "Может ли bob потратить 10 монет? {}",
        if chain.can_spend("bob", 10, &Mempool::new()) {
            "Да"
        } else {
            "Нет"
        }
    );

    println!("\nИтоговая статистика                    ");
    println!("  Балансы (вычислены из блоков):                           ");
    println!("    alice:   {:>4} монет", chain.get_balance("alice"));
    println!("    bob:     {:>4} монет", chain.get_balance("bob"));
    println!("    charlie: {:>4} монет", chain.get_balance("charlie"));
    println!("    dave:    {:>4} монет", chain.get_balance("dave"));
    println!("    eve:     {:>4} монет", chain.get_balance("eve"));
    println!("    miner:   {:>4} монет", chain.get_balance("miner"));
    println!("  Проекты:                                                  ");
    if let Some(project) = chain.get_project("proj_rust_game") {
        println!(
            "    proj_rust_game: собрано {}/{} монет",
            project.raised_amount, project.goal_amount
        );
    }
    println!("  Цепь: {} блоков", chain.height());
    println!("  Мемпул: {} pending транзакций", mempool.len());
}

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

fn create_transfer(from: &str, to: &str, amount: u64) -> Transaction {
    Transaction::new(
        TransactionType::Transfer,
        Some(from.to_string()),
        Some(to.to_string()),
        TransactionData::Transfer(TransferData {
            amount,
            message: String::new(),
        }),
        1234567890,
        Some("signature_stub".to_string()),
    )
}

fn create_project(creator: &str, project_id: &str, name: &str, goal: u64) -> Transaction {
    Transaction::new(
        TransactionType::CreateProject,
        Some(creator.to_string()),
        None,
        TransactionData::CreateProject(CreateProjectData {
            project_id: project_id.to_string(),
            name: name.to_string(),
            description: format!("{} description", name),
            goal_amount: goal,
            deadline_timestamp: 9999999999,
            creator_wallet: creator.to_string(),
        }),
        1234567890,
        Some("signature_stub".to_string()),
    )
}

fn create_fund_project(backer: &str, creator: &str, project_id: &str, amount: u64) -> Transaction {
    Transaction::new(
        TransactionType::FundProject,
        Some(backer.to_string()),
        Some(creator.to_string()),
        TransactionData::FundProject(FundProjectData {
            project_id: project_id.to_string(),
            amount,
            backer_note: String::new(),
        }),
        1234567890,
        Some("signature_stub".to_string()),
    )
}
