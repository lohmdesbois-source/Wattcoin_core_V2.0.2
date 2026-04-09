use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::{Arc, Mutex};
use crate::transaction::Transaction;
use crate::block::Block; 
use crate::blockchain::Blockchain; 

pub async fn start_server(
    port: &str, 
    mempool: Arc<Mutex<Vec<Transaction>>>,
    blockchain: Arc<Mutex<Blockchain>> 
) {
    let address = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&address).await.unwrap();
    println!("📡 Nœud à l'écoute sur le port {}...", port);

    loop {
        let (mut socket, _peer_addr) = listener.accept().await.unwrap();
        let mempool_clone = Arc::clone(&mempool);
        let blockchain_clone = Arc::clone(&blockchain);

        tokio::spawn(async move {
            let mut buffer = [0; 8192]; 
            if let Ok(n) = socket.read(&mut buffer).await {
                if n > 0 {
                    let message = String::from_utf8_lossy(&buffer[..n]);
                    
                    // 1. Est-ce une TRANSACTION entrante ?
                    if let Ok(tx) = serde_json::from_str::<Transaction>(&message) {
                        println!("\n💸 ALERTE RÉSEAU : Nouvelle transaction reçue !");
                        if tx.is_valid() {
                            // PATCH : On verrouille et on relâche tout de suite
                            {
                                let mut pool = mempool_clone.lock().unwrap();
                                pool.push(tx);
                                println!("   📥 Transaction ajoutée au Mempool.");
                            } // <--- Le verrou est lâché !
                            let _ = socket.write_all(b"Tx OK\n").await;
                        } else {
                            let _ = socket.write_all(b"Tx Invalide\n").await;
                        }
                        
                    // 2. Est-ce un BLOC entrant ?
                    } else if let Ok(block) = serde_json::from_str::<Block>(&message) {
                        println!("\n🌍 ALERTE RÉSEAU : Un nouveau BLOC a été diffusé par un mineur !");
                        
                        let mut is_valid = false;
                        
                        // PATCH : On verrouille la chaîne pour vérifier et ajouter
                        {
                            let mut chain = blockchain_clone.lock().unwrap();
                            if block.header.previous_hash == chain.chain.last().unwrap().header.hash {
                                println!("   ✅ Le Bloc est valide et s'emboîte parfaitement !");
                                chain.chain.push(block);
                                is_valid = true;
                            } else {
                                println!("   ❌ Le Bloc ne correspond pas à notre historique (Fork potentiel).");
                            }
                        } // <--- Le verrou de la chaîne est lâché !

                        if is_valid {
                            // PATCH : On verrouille le mempool pour le nettoyer
                            {
                                let mut pool = mempool_clone.lock().unwrap();
                                pool.clear();
                                println!("   🧹 Mempool nettoyé. Mise à jour de l'historique local terminée.");
                            } // <--- Le verrou du mempool est lâché !
                            
                            let _ = socket.write_all(b"Bloc accepte\n").await;
                        } else {
                            let _ = socket.write_all(b"Bloc rejete\n").await;
                        }
                    }
                }
            }
        });
    }
}

pub async fn send_message(target_port: &str, message: &str) {
    let address = format!("127.0.0.1:{}", target_port);
    if let Ok(mut stream) = TcpStream::connect(&address).await {
        let _ = stream.write_all(message.as_bytes()).await;
    }
}