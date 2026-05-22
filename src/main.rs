use std::env;
use std::net::SocketAddr;
use std::process;
use std::sync::{Arc, Mutex};

// === Модули ===
mod api;
mod consensus;
mod crypto;
mod mempool;
mod network;
mod state;
mod storage;
mod types;
mod validator; // ← НОВЫЙ МОДУЛЬ для P2P

// === Импорт типов ===
use api::server::{ApiContext, start_server};
use mempool::mempool::Mempool;
use network::NetworkService;
use storage::blockchain::Blockchain;
use types::Block; // ← НОВЫЙ ИМПОРТ

/// Конфигурация ноды
struct NodeConfig {
    port: u16,                        // HTTP API порт
    p2p_port: u16,                    // P2P порт
    difficulty: u32,                  // PoW сложность
    bootstrap_peers: Vec<SocketAddr>, // Адреса для авто-подключения
}

impl Default for NodeConfig {
    fn default() -> Self {
        NodeConfig {
            port: 3000,
            p2p_port: 3001,
            difficulty: 4,
            bootstrap_peers: Vec::new(),
        }
    }
}

#[tokio::main] // ← Асинхронная точка входа для P2P
async fn main() {
    println!("BlockKick Node v0.4.0 (P2P Enabled)");
    println!("========================");

    // Парсим аргументы командной строки
    let config = parse_args();

    println!("Configuration:");
    println!("   HTTP Port:      {}", config.port);
    println!("   P2P Port:       {}", config.p2p_port);
    println!("   Difficulty:     {}", config.difficulty);
    println!(
        "   Bootstrap Peers: {}",
        config
            .bootstrap_peers
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!();

    // === Инициализация блокчейна ===
    println!("Initializing blockchain...");
    let blockchain = Arc::new(Mutex::new(Blockchain::new()));

    // Проверяем что genesis блок создан
    let genesis_hash = {
        let chain = blockchain.lock().unwrap();
        let hash = chain.get_latest_block().unwrap().calculate_hash();
        println!("   Genesis block hash: {}...", &hash[..16]);
        println!("   Chain height: {}", chain.height());
        hash // ← Сохраняем для проверки совместимости сети
    };

    // === Инициализация мемпула ===
    println!("Initializing mempool...");
    let mempool = Arc::new(Mutex::new(Mempool::new()));

    // === Запуск P2P сервиса ===
    println!("Starting P2P network on 0.0.0.0:{}...", config.p2p_port);

    let p2p_addr: SocketAddr = format!("0.0.0.0:{}", config.p2p_port)
        .parse()
        .expect("Invalid P2P address");

    let network = Arc::new(NetworkService::new(
        p2p_addr,
        genesis_hash.clone(),
        Arc::clone(&blockchain),
    ));

    crate::network::set_global_network(Arc::clone(&network));

    // Запускаем P2P сервер в фоне (асинхронно)
    let network_clone = Arc::clone(&network);
    let network_handle = tokio::spawn(async move {
        if let Err(e) = network_clone.start().await {
            eprintln!("P2P server error: {}", e);
        }
    });

    // Подключаемся к бутстрап-пирам (если указаны)
    for peer_addr in &config.bootstrap_peers {
        println!("Connecting to bootstrap peer: {}...", peer_addr);
        if let Err(e) = network.connect_to(*peer_addr).await {
            eprintln!("Failed to connect to {}: {}", peer_addr, e);
        }
    }

    // === Создаём контекст для HTTP API ===
    let ctx = ApiContext {
        blockchain: Arc::clone(&blockchain),
        mempool: Arc::clone(&mempool),
    };

    // === Запуск HTTP сервера ===
    println!("Starting API server on http://0.0.0.0:{}...", config.port);
    println!();
    println!("Available endpoints:");
    println!("   GET  /api/v1/chain              - Chain info");
    println!("   GET  /api/v1/balance/:address   - Get balance");
    println!("   GET  /api/v1/block/:height      - Get block by height");
    println!("   GET  /api/v1/projects           - List projects");
    println!("   GET  /api/v1/transactions/:id   - Transaction status");
    println!("   POST /api/v1/transactions       - Submit transaction");
    println!("   GET  /api/v1/mining/candidate   - Get mining template");
    println!("   POST /api/v1/mining/submit      - Submit mined block");
    println!();
    println!("P2P Network:");
    println!("   Listening on: 0.0.0.0:{}", config.p2p_port);
    println!("   Connected peers: 0 (will update dynamically)");
    println!();
    println!("Press Ctrl+C to stop the node");
    println!("========================");

    // === Запускаем HTTP сервер в отдельном потоке ===
    // (tiny_http — синхронный, поэтому в std::thread)
    let api_handle = std::thread::spawn(move || {
        if let Err(e) = start_server(ctx, config.port) {
            eprintln!("HTTP server error: {}", e);
        }
    });

    // === Ожидание сигнала завершения (Ctrl+C) ===
    // tokio::signal работает только внутри async runtime
    let shutdown_result = tokio::signal::ctrl_c().await;

    //println!("\n🛑 Shutdown signal received, cleaning up...");

    //// Останавливаем P2P сервер (аборт задачи)
    //network_handle.abort();

    //// Ждём завершения HTTP сервера (не блокирующе, с таймаутом)
    //// В простой реализации: просто выходим
    //let _ = api_handle.join();

    //println!("✅ Node stopped gracefully");
    process::exit(0);
}

/// Парсит аргументы командной строки
fn parse_args() -> NodeConfig {
    let mut config = NodeConfig::default();

    let args: Vec<String> = env::args().collect();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" | "-p" => {
                if i + 1 < args.len() {
                    if let Ok(port) = args[i + 1].parse::<u16>() {
                        config.port = port;
                        i += 1;
                    } else {
                        eprintln!("⚠️  Invalid port number, using default 3000");
                    }
                }
            }
            "--p2p-port" => {
                if i + 1 < args.len() {
                    if let Ok(port) = args[i + 1].parse::<u16>() {
                        config.p2p_port = port;
                        i += 1;
                    } else {
                        eprintln!("⚠️  Invalid P2P port, using default 3001");
                    }
                }
            }
            "--difficulty" | "-d" => {
                if i + 1 < args.len() {
                    if let Ok(diff) = args[i + 1].parse::<u32>() {
                        config.difficulty = diff;
                        i += 1;
                    } else {
                        eprintln!("⚠️  Invalid difficulty, using default 4");
                    }
                }
            }
            "--connect" | "--bootstrap" => {
                if i + 1 < args.len() {
                    if let Ok(addr) = args[i + 1].parse::<SocketAddr>() {
                        config.bootstrap_peers.push(addr);
                        i += 1;
                    } else {
                        eprintln!("⚠️  Invalid peer address: {}", args[i + 1]);
                    }
                }
            }
            "--help" | "-h" => {
                print_help();
                process::exit(0);
            }
            _ => {
                eprintln!("⚠️  Unknown argument: {}", args[i]);
            }
        }
        i += 1;
    }

    config
}

/// Выводит справку по использованию
fn print_help() {
    println!("BlockKick Node - Децентрализованная краудфандинговая платформа");
    println!();
    println!("USAGE:");
    println!("    cargo run [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -p, --port <PORT>              HTTP API port (default: 3000)");
    println!("        --p2p-port <PORT>          P2P network port (default: 3001)");
    println!("    -d, --difficulty <DIFFICULTY>  PoW difficulty (default: 4)");
    println!("        --connect <ADDR>           Connect to bootstrap peer (можно повторять)");
    println!("    -h, --help                     Print this help message");
    println!();
    println!("EXAMPLES:");
    println!("    cargo run");
    println!("    cargo run -- --port 8080 --p2p-port 8081");
    println!("    cargo run -- -p 9000 --connect 127.0.0.1:3001");
    println!("    cargo run -- --connect 192.168.1.100:3001 --connect 192.168.1.101:3001");
    println!();
    println!("API ENDPOINTS:");
    println!("    http://localhost:3000/api/v1/chain");
    println!("    http://localhost:3000/api/v1/mining/candidate");
    println!("    http://localhost:3000/api/v1/balance/<address>");
    println!();
    println!("P2P:");
    println!("    Нода слушает на 0.0.0.0:3001 (TCP)");
    println!("    Используйте --connect для подключения к другим нодам");
}
