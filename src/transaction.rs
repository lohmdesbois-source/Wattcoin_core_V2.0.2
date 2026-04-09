use serde::{Serialize, Deserialize};
use ed25519_dalek::{VerifyingKey, Signature, Verifier};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub sender: String,     
    pub receiver: String,   
    pub amount: u64,        
    pub signature: String,  
    // NOUVEAU : Le bloc à partir duquel on a le droit de dépenser cet argent
    pub unlock_block: Option<u64>, 
}

impl Transaction {
    pub fn new_signed(wallet: &crate::wallet::Wallet, receiver: String, amount: u64) -> Self {
        let sender = wallet.get_address();
        let tx_data = format!("{}{}{}", sender, receiver, amount);
        let message_bytes = tx_data.as_bytes();
        let signature = wallet.sign_data(message_bytes);
        
        Transaction {
            sender,
            receiver,
            amount,
            signature: hex::encode(signature.to_bytes()),
            unlock_block: None, // Les transferts normaux ne sont pas gelés
        }
    }

    pub fn is_valid(&self) -> bool {
        if self.sender == "SYSTEM" { return true; }

        let tx_data = format!("{}{}{}", self.sender, self.receiver, self.amount);
        let message_bytes = tx_data.as_bytes();

        let public_key_bytes = match hex::decode(&self.sender) {
            Ok(bytes) => bytes, Err(_) => return false,
        };
        let public_key = match VerifyingKey::from_bytes(public_key_bytes.as_slice().try_into().unwrap()) {
            Ok(key) => key, Err(_) => return false,
        };
        let signature_bytes = match hex::decode(&self.signature) {
            Ok(bytes) => bytes, Err(_) => return false,
        };
        let signature = match Signature::from_slice(&signature_bytes) {
            Ok(sig) => sig, Err(_) => return false,
        };

        public_key.verify(message_bytes, &signature).is_ok()
    }
}