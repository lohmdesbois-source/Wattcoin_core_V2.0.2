mod block;
mod blockchain;
mod wallet;
mod transaction;
mod network; 

use std::env;
use std::sync::{Arc, Mutex};
use wallet::Wallet;
use transaction::Transaction;
use blockchain::Blockchain;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { return; }
    let role = args[1].as_str();

    let mempool: Arc<Mutex<Vec<Transaction>>> = Arc::new(Mutex::new(Vec::new()));

    match role {
        "alice" => {
            println!("🔥 Démarrage du Nœud ALICE...");
            
            // Alice a sa propre blockchain locale !
            let db_file = "alice_chain.json";
            let wattcoin_alice = Blockchain::load_from_disk(2, db_file);
            let shared_chain = Arc::new(Mutex::new(wattcoin_alice));
            
            let mempool_alice = Arc::clone(&mempool);
            let chain_for_server = Arc::clone(&shared_chain);
            
            tokio::spawn(async move { network::start_server("8000", mempool_alice, chain_for_server).await; });
            
            // Alice ne mine pas, elle écoute juste le réseau éternellement
            println!("👂 Alice est synchronisée et écoute le réseau...");
            loop { tokio::time::sleep(tokio::time::Duration::from_secs(10)).await; }
        }
        
        "bob" => {
            println!("🔥 Démarrage du Nœud BOB (Mineur Continu)...");
            
            let db_file = "bob_chain.json";
            // Bob a sa blockchain, qu'on emballe dans un Mutex pour la partager
            let shared_chain = Arc::new(Mutex::new(Blockchain::load_from_disk(2, db_file))); 
            
            let bob_wallet = Wallet::load_or_create("bob_wallet.txt");
            
            let mempool_bob = Arc::clone(&mempool);
            let chain_for_server = Arc::clone(&shared_chain);
            tokio::spawn(async move { network::start_server("8001", mempool_bob, chain_for_server).await; });
            
            println!("⏳ Bob commence le minage...");

            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                let pending_txs = {
                    let mut pool = mempool.lock().unwrap();
                    let txs = pool.clone(); 
                    pool.clear();           
                    txs
                };

                // Bob verrouille la chaîne juste le temps de miner
                let (block_found, block_json) = {
                    let mut chain = shared_chain.lock().unwrap();
                    let current_height = chain.chain.len();
                    println!("\n⛏️  Bob tente de forger le Bloc {}...", current_height);
                    
                    let before_len = chain.chain.len();
                    chain.mine_and_add_block(pending_txs, &bob_wallet.get_address());
                    
                    // Si la taille a augmenté, c'est qu'il a réussi !
                    if chain.chain.len() > before_len {
                        chain.save_to_disk(db_file);
                        let latest_block = chain.chain.last().unwrap();
                        (true, serde_json::to_string(latest_block).unwrap())
                    } else {
                        (false, String::new())
                    }
                }; // Le verrou de la chaîne est lâché ici !

                // S'il a gagné, il hurle sa victoire au monde entier (à Alice) !
                if block_found {
                    println!("📢 PROPAGATION : Bob diffuse son nouveau bloc sur le réseau !");
                    network::send_message("8000", &block_json).await;
                }
            }
        }
        _ => println!("Rôle inconnu."),
    }
}