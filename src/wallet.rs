use bip39::{Mnemonic, Language};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer};
use rand::RngCore; // Pour générer le chaos (entropie)
use std::fs;
use std::path::Path;

pub struct Wallet {
    signing_key: SigningKey,
    pubk: VerifyingKey,
}

impl Wallet {
    // Portefeuille jetable pour les tests rapides (Alice)
    pub fn new() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes); // On crée 32 octets au hasard
        
        let signing_key = SigningKey::from_bytes(&bytes);
        let pubk = VerifyingKey::from(&signing_key);

        Wallet { signing_key, pubk }
    }

    // Le Portefeuille Persistant (Bob)
    pub fn load_or_create(filename: &str) -> Self {
        if Path::new(filename).exists() {
            if let Ok(phrase) = fs::read_to_string(filename) {
                let phrase = phrase.trim();
                
                // NOUVELLE SYNTAXE : On lit la phrase
                if let Ok(mnemonic) = Mnemonic::parse_in_normalized(Language::French, phrase) {
                    let entropy = mnemonic.to_entropy(); // On récupère les octets
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(&entropy[0..32]);
                    
                    let signing_key = SigningKey::from_bytes(&bytes);
                    let pubk = VerifyingKey::from(&signing_key);
                    
                    println!("🔑 Portefeuille restauré avec succès depuis '{}' !", filename);
                    return Wallet { signing_key, pubk };
                }
            }
            println!("⚠️ Fichier portefeuille corrompu ! On en recrée un...");
        }

        // Création d'un nouveau portefeuille
        println!("🌱 Création d'un nouveau portefeuille persistant...");
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        
        // NOUVELLE SYNTAXE : On génère les 24 mots à partir de notre chaos
        let mnemonic = Mnemonic::from_entropy_in(Language::French, &bytes)
            .expect("Erreur de génération des mots");
        
        // On sauvegarde la phrase (avec .to_string() pour la v2)
        fs::write(filename, mnemonic.to_string()).expect("Impossible d'écrire sur le disque !");
        println!("💾 Les 24 mots secrets ont été sauvegardés dans '{}'.", filename);

        let signing_key = SigningKey::from_bytes(&bytes);
        let pubk = VerifyingKey::from(&signing_key);

        Wallet { signing_key, pubk }
    }

    pub fn get_address(&self) -> String {
        hex::encode(self.pubk.to_bytes())
    }

    pub fn sign_data(&self, data: &[u8]) -> Signature {
        self.signing_key.sign(data)
    }
}