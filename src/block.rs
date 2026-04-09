use chrono::Utc;
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use crate::transaction::Transaction;

// On définit une taille de RAM pour le réseau (ex: 10 Mégaoctets pour nos tests)
// Sur le vrai réseau, ça pourrait être 2 Gigaoctets.
const RAM_BUFFER_SIZE: usize = 10 * 1024 * 1024; 

#[derive(Debug, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>, 
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockHeader {
    pub index: u64,
    pub timestamp: i64,
    pub previous_hash: String,
    pub hash: String,
    pub nonce: u64, 
}

impl Block {
	pub fn genesis() -> Self {
        let mut header = BlockHeader {
            index: 0,
            timestamp: 1700000000, // LE FIX : Une date fixe pour l'éternité (au lieu de Utc::now())
            previous_hash: String::from("0000000000000000000000000000000000000000000000000000000000000000"),
            hash: String::new(),
            nonce: 0,
        };

        let tx = Transaction {
            sender: String::from("SYSTEM"),
            receiver: String::from("00000000000000000000000000000000"),
            amount: 0,
            signature: String::from("Genesis"),
            unlock_block: None,
        };

        header.hash = Self::calculate_ram_hash(&header, &Self::generate_ram_buffer(&header.previous_hash));

        Block {
            header,
            transactions: vec![tx],
        }
    }

    // NOUVEAU : Fonction qui simule le remplissage de la RAM avec des données utiles
    pub fn generate_ram_buffer(previous_hash: &str) -> Vec<u8> {
        let mut buffer = vec![0u8; RAM_BUFFER_SIZE];
        let seed = previous_hash.as_bytes();
        
        // On remplit la RAM de manière déterministe (pour que tout le réseau ait la même mémoire)
        for i in 0..buffer.len() {
            buffer[i] = seed[i % seed.len()].wrapping_add((i % 256) as u8);
        }
        buffer
    }

    // NOUVEAU : L'algorithme de hachage qui OBLIGE à lire la RAM
    pub fn calculate_ram_hash(header: &BlockHeader, ram_buffer: &[u8]) -> String {
        let mut hasher = Sha256::new();
        
        // 1. On hache l'en-tête classique
        let header_data = format!("{}{}{}{}", header.index, header.timestamp, header.previous_hash, header.nonce);
        hasher.update(header_data);

        // 2. MAGIE CYPHERPUNK : On utilise le nonce pour choisir un endroit aléatoire dans la RAM
        // Le processeur est obligé d'aller chercher cette donnée en mémoire vive.
        let index_in_ram = (header.nonce as usize * 41) % (ram_buffer.len() - 64);
        
        // 3. On ajoute ce fragment de RAM au calcul
        hasher.update(&ram_buffer[index_in_ram..index_in_ram + 64]);
        
        let result = hasher.finalize();
        format!("{:x}", result)
    }
}