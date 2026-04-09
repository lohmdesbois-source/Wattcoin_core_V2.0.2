use crate::block::{Block, BlockHeader};
use crate::transaction::Transaction;
use chrono::Utc;
use std::collections::HashMap;
use std::fs;

// Paramètres du réseau (en vrai : 100 et 1000)
const MATURITY_BLOCKS: u64 = 3; 
const GRACE_PERIOD: usize = 3; 

pub struct Blockchain {
    pub chain: Vec<Block>,
    pub difficulty: usize,
}

impl Blockchain {
    pub fn new(difficulty: usize) -> Self {
        let mut blockchain = Blockchain {
            chain: Vec::new(),
            difficulty,
        };
        blockchain.chain.push(Block::genesis());
        blockchain
    }
	
	// NOUVEAU : 1. Charger depuis le disque
    pub fn load_from_disk(difficulty: usize, filename: &str) -> Self {
        if let Ok(data) = fs::read_to_string(filename) {
            // Si le fichier existe, on le lit et on recrée l'objet Blockchain complet !
            if let Ok(chain) = serde_json::from_str::<Vec<Block>>(&data) {
                println!("💾 HISTORIQUE CHARGÉ : {} blocs retrouvés sur le disque.", chain.len());
                return Blockchain {
                    chain,
                    difficulty,
                };
            }
        }
        // Si le fichier n'existe pas (premier lancement), on crée le Genesis normal
        println!("🌱 Aucun historique trouvé. Création du Genesis Block...");
        Blockchain::new(difficulty)
    }

    // NOUVEAU : 2. Sauvegarder sur le disque
    pub fn save_to_disk(&self, filename: &str) {
        let json = serde_json::to_string_pretty(&self.chain).unwrap();
        fs::write(filename, json).expect("Impossible d'écrire sur le disque !");
        println!("💾 Blockchain sauvegardée en toute sécurité dans '{}'.", filename);
    }

    // NOUVEAU : On calcule la balance en ignorant l'argent gelé !
    pub fn get_available_balance(&self, address: &str, current_height: u64) -> u64 {
        let mut balance: u64 = 0;
        for block in &self.chain {
            for tx in &block.transactions {
                if tx.receiver == address {
                    // L'argent est-il dégelé ?
                    let is_unlocked = match tx.unlock_block {
                        Some(unlock_height) => current_height >= unlock_height,
                        None => true,
                    };
                    if is_unlocked { balance += tx.amount; }
                }
                if tx.sender == address {
                    balance = balance.saturating_sub(tx.amount);
                }
            }
        }
        balance
    }

    pub fn mine_and_add_block(&mut self, transactions: Vec<Transaction>, miner_address: &str) {
        let current_height = self.chain.len() as u64;
        println!("⏳ Début de la proposition du Bloc {}...", current_height);

        // --- LA PEAU EN JEU (Avec période de grâce au démarrage) ---
        if self.chain.len() > GRACE_PERIOD {
            let miner_balance = self.get_available_balance(miner_address, current_height);
            if miner_balance < 20 {
                println!("   🛑 ACCÈS REFUSÉ : Le mineur {} n'a que {} Watts DÉGELÉS. Il faut 20 Watts disponibles pour miner !", 
                    &miner_address[0..8], miner_balance);
                return;
            }
            println!("   ⚖️  SÉQUESTRE VALIDÉ : Caution de 20 Watts reconnue.");
        } else {
            println!("   🕊️  PÉRIODE DE GRÂCE : Amorçage du réseau, séquestre non requis.");
        }

        let mut valid_transactions = Vec::new();
        let mut pending_spent: HashMap<String, u64> = HashMap::new();

        // --- FILTRE ANTI-FRAUDE ---
        for tx in transactions {
            if !tx.is_valid() {
                println!("   ❌ FRAUDE DÉTECTÉE : Signature invalide !");
                continue;
            }
            // On vérifie avec la balance DÉGELÉE
            let base_balance = self.get_available_balance(&tx.sender, current_height);
            let already_spent_now = pending_spent.get(&tx.sender).unwrap_or(&0);
            let current_available = base_balance.saturating_sub(*already_spent_now);

            if current_available < tx.amount {
                println!("   ❌ FRAUDE DÉTECTÉE : Fonds insuffisants ou gelés ({} essaie d'envoyer {} mais n'a que {} dégelés).", 
                    &tx.sender[0..8], tx.amount, current_available);
                continue;
            }
            pending_spent.insert(tx.sender.clone(), already_spent_now + tx.amount);
            println!("   ✅ Transaction valide : {} Watts autorisés.", tx.amount);
            valid_transactions.push(tx);
        }
        
        // --- LE PROTOCOLE PAIE LE MINEUR (AVEC LE CADENAS TEMPOREL) ---
        let coinbase_tx = Transaction {
            sender: String::from("SYSTEM"),
            receiver: miner_address.to_string(),
            amount: 50, 
            signature: String::from("BlockReward"),
            // LE VERROU : L'argent est bloqué pour X blocs
            unlock_block: Some(current_height + MATURITY_BLOCKS), 
        };
        valid_transactions.insert(0, coinbase_tx);

        let previous_block = self.chain.last().unwrap();
        let mut new_header = BlockHeader {
            index: current_height,
            timestamp: Utc::now().timestamp(),
            previous_hash: previous_block.header.hash.clone(),
            hash: String::new(),
            nonce: 0,
        };

        println!("🧠 Allocation de la RAM (10 Mo)...");
        let ram_buffer = Block::generate_ram_buffer(&new_header.previous_hash);
        let target_prefix = "0".repeat(self.difficulty);

        loop {
            new_header.hash = Block::calculate_ram_hash(&new_header, &ram_buffer);
            if new_header.hash.starts_with(&target_prefix) {
                println!("⛏️  Eurêka ! Nouveau Hash : {}\n", new_header.hash);
                break;
            }
            new_header.nonce += 1; 
        }

        self.chain.push(Block { header: new_header, transactions: valid_transactions });
    }
}