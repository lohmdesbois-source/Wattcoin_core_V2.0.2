use warp::Filter;
use crate::blockchain::Blockchain;
use crate::transaction::Transaction;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use rand::RngCore;

pub type SharedPool = Arc<Mutex<Vec<Order>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub order_type: String,
    pub amount_flames: u64,
    pub price_sats: u64,
    pub btc_address: String,
    pub watt_address: String,
}

#[derive(Serialize, Deserialize)]
pub struct SwapContract {
    pub buyer_btc_address: String,
    pub seller_watt_address: String,
    pub watt_amount_flames: u64,
    pub btc_amount_sats: u64,
    pub htlc_secret: String,
    pub htlc_hash: String,
}

#[derive(Serialize, Deserialize)]
pub struct BatchResult {
    pub success: bool,
    pub message: String,
    pub clearing_price_sats: u64,
    pub total_volume_flames: u64,
    pub swaps: Vec<SwapContract>,
}

// 💡 FIX : On remplace "peer_port" par "known_peers" (le carnet d'adresses)
pub async fn start_api_server(port: u16, host_ip: [u8; 4], mempool: Arc<Mutex<Vec<Transaction>>>, chain: Arc<Mutex<Blockchain>>, known_peers: crate::SharedPeers, dex_pool: SharedPool) {
    
    let mempool_filter = warp::any().map(move || Arc::clone(&mempool));
    let chain_filter = warp::any().map(move || Arc::clone(&chain));
    let dex_pool_filter = warp::any().map(move || Arc::clone(&dex_pool));
    
    // Le filtre pour accéder au carnet d'adresses depuis les routes
    let peers_filter = warp::any().map(move || Arc::clone(&known_peers));

    let send_tx = warp::post()
        .and(warp::path("send_tx"))
        .and(warp::body::json())
        .and(mempool_filter.clone())
        .and(peers_filter.clone())
        .map(|tx: Transaction, mempool: Arc<Mutex<Vec<Transaction>>>, peers: crate::SharedPeers| {
            if tx.is_valid() {
                let mut pool = mempool.lock().unwrap();
                
                if !pool.iter().any(|t| t.kyber_capsule == tx.kyber_capsule) {
                    pool.push(tx.clone());
                    println!("📥 [API] Nouvelle transaction ajoutée au Mempool !");

                    // 📢 PROPAGATION P2P : On crie à tous nos contacts !
                    let peers_list = peers.lock().unwrap().clone();
                    for peer in peers_list {
                        let tx_to_broadcast = tx.clone();
                        tokio::spawn(async move {
                            crate::network::broadcast_transaction(&peer, tx_to_broadcast).await;
                        });
                    }
                    
                    warp::reply::with_status(
                        warp::reply::json(&"✅ Transaction acceptée et propagée"),
                        warp::http::StatusCode::OK,
                    )
                } else {
                    warp::reply::with_status(
                        warp::reply::json(&"⚠️ Déjà dans le mempool"),
                        warp::http::StatusCode::BAD_REQUEST,
                    )
                }
            } else {
                warp::reply::with_status(
                    warp::reply::json(&"❌ Cryptographie invalide !"),
                    warp::http::StatusCode::BAD_REQUEST,
                )
            }
        });
    
    let get_all_txs = warp::get()
        .and(warp::path("all_transactions"))
        .and(chain_filter.clone())
        .map(|chain_arc: Arc<Mutex<Blockchain>>| {
            let chain_lock = chain_arc.lock().unwrap();
            let mut all_txs = Vec::new();
            for block in &chain_lock.chain {
                for tx in &block.transactions {
                    all_txs.push(tx.clone());
                }
            }
            warp::reply::json(&all_txs)
        });

    let get_decoys = warp::get()
        .and(warp::path!("get_decoys" / usize))
        .and(chain_filter.clone())
        .map(|count: usize, chain_arc: Arc<Mutex<Blockchain>>| {
            let chain_lock = chain_arc.lock().unwrap();
            let decoys = chain_lock.get_random_decoys(count);
            warp::reply::json(&decoys)
        });

    let get_balance = warp::get()
        .and(warp::path!("balance" / String))
        .and(chain_filter.clone())
        .map(|address: String, chain_arc: Arc<Mutex<Blockchain>>| {
            let chain_lock = chain_arc.lock().unwrap();
            let mut balance_flames: f64 = 0.0;
            
            let current_height = chain_lock.chain.len() as u64; 
            
            for block in &chain_lock.chain {
                for tx in &block.transactions {
                    if tx.stealth_address == address || tx.stealth_address == format!("COINBASE_{}", address) {
                        
                        let is_coinbase = tx.stealth_address.starts_with("COINBASE_");
                        let is_mature = current_height > (block.header.index + 3); 
                        
                        if !is_coinbase || is_mature {
                            if let Ok(amt) = tx.aes_vault.parse::<f64>() {
                                balance_flames += amt;
                            }
                        }
                    }
                }
            }
            let balance_watts = balance_flames / 1_000_000_000.0;
            warp::reply::json(&balance_watts)
        });
    
    let get_pool = warp::get()
        .and(warp::path("pool"))
        .and(dex_pool_filter.clone())
        .map(|pool: SharedPool| {
            let orders = pool.lock().unwrap().clone();
            warp::reply::json(&orders)
        });

    let submit_order = warp::post()
        .and(warp::path("order"))
        .and(warp::body::json())
        .and(dex_pool_filter.clone())
        .and(peers_filter.clone())
        .map(|order: Order, pool: SharedPool, peers: crate::SharedPeers| {
            println!("📡 [API DEX] Ordre reçu du Wallet : {} {} WATT", order.order_type, order.amount_flames);
            
            let mut is_new = false;
            {
                let mut p = pool.lock().unwrap();
                if !p.iter().any(|o| o.id == order.id) {
                    p.push(order.clone());
                    is_new = true;
                }
            }

            if is_new {
                let peers_list = peers.lock().unwrap().clone();
                for peer in peers_list {
                    let order_clone = order.clone();
                    tokio::spawn(async move {
                        crate::network::broadcast_order(&peer, order_clone).await;
                    });
                }
            }
            
            warp::reply::json(&"Ordre ajouté et propagé")
        });

    let resolve_batch = warp::post()
        .and(warp::path("resolve"))
        .and(dex_pool_filter.clone())
        .map(|pool: SharedPool| { 
            let mut orders = pool.lock().unwrap();
            if orders.is_empty() {
                return warp::reply::json(&BatchResult { success: false, message: "Piscine vide.".to_string(), clearing_price_sats: 0, total_volume_flames: 0, swaps: vec![] });
            }

            let mut buys: Vec<Order> = orders.iter().filter(|o| o.order_type == "buy").cloned().collect();
            let mut sells: Vec<Order> = orders.iter().filter(|o| o.order_type == "sell").cloned().collect();

            buys.sort_by(|a, b| b.price_sats.cmp(&a.price_sats));
            sells.sort_by(|a, b| a.price_sats.cmp(&b.price_sats));

            let mut clearing_price_sats = 0;
            let mut total_volume_flames = 0;
            let mut generated_swaps = Vec::new();
            let mut i = 0; let mut j = 0;

            while i < buys.len() && j < sells.len() {
                if buys[i].price_sats >= sells[j].price_sats {
                    clearing_price_sats = (buys[i].price_sats + sells[j].price_sats) / 2;
                    let trade_amount_flames = buys[i].amount_flames.min(sells[j].amount_flames);
                    total_volume_flames += trade_amount_flames;

                    let mut secret_bytes = [0u8; 32];
                    rand::thread_rng().fill_bytes(&mut secret_bytes);
                    let htlc_hash = hex::encode(blake3::hash(&secret_bytes).as_bytes());

                    let w_amt = trade_amount_flames as f64 / 1_000_000_000.0;
                    let btc_amount_sats = (w_amt * clearing_price_sats as f64) as u64;

                    generated_swaps.push(SwapContract {
                        buyer_btc_address: buys[i].btc_address.clone(),
                        seller_watt_address: sells[j].watt_address.clone(),
                        watt_amount_flames: trade_amount_flames,
                        btc_amount_sats,
                        htlc_secret: hex::encode(secret_bytes),
                        htlc_hash,
                    });

                    buys[i].amount_flames -= trade_amount_flames;
                    sells[j].amount_flames -= trade_amount_flames;
                    if buys[i].amount_flames == 0 { i += 1; }
                    if sells[j].amount_flames == 0 { j += 1; }
                } else { break; }
            }

            orders.clear();

            if total_volume_flames > 0 {
                warp::reply::json(&BatchResult { success: true, message: "Ordres croisés !".to_string(), clearing_price_sats, total_volume_flames, swaps: generated_swaps })
            } else {
                warp::reply::json(&BatchResult { success: false, message: "Aucun croisement.".to_string(), clearing_price_sats: 0, total_volume_flames: 0, swaps: vec![] })
            }
        });
    
    let info_route = warp::path("info")
        .and(warp::get())
        .and(chain_filter.clone())
        .and(peers_filter.clone()) // 💡 On utilise le carnet d'adresses ici aussi !
        .map(|chain_arc: Arc<Mutex<Blockchain>>, peers: crate::SharedPeers| {
            
            let chain_lock = chain_arc.lock().unwrap();
            let true_height = chain_lock.chain.len(); 
            let active_connections = peers.lock().unwrap().len(); // On compte nos amis !

            warp::reply::json(&serde_json::json!({
                "blocks": true_height, 
                "connected_peers": active_connections, 
                "difficulty_decimal": 5.17, 
                "version": "Wattcoin V2.0.0 (Ano PQ)",
                "target_hex": "00000fffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            }))
        });
    
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST"]);

    let routes = send_tx.or(get_all_txs).or(get_decoys).or(get_balance)
        .or(get_pool).or(submit_order).or(resolve_batch).or(info_route)
        .with(cors); 
    
    println!("📡 API Serveur active sur le port {}...", port);
    println!("🔍 Routes disponibles : GET /info | GET /all_transactions | POST /send_tx ...");
    
    warp::serve(routes).run((host_ip, port)).await;
}