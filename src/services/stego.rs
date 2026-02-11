use crate::error::AppError;
use base64::{engine::general_purpose, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
    ChaCha20Poly1305, Nonce,
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};

#[derive(Clone)]
pub struct SteganographyService{
    cipher: ChaCha20Poly1305,
}

impl SteganographyService {
    pub fn new(salt: &str) -> Result<Self, AppError> {
        // derive a key from the salt
        let salt_string = SaltString::encode_b64(salt.as_bytes())
            .map_err(|e| AppError::Internal(format!("Invalid salt: {}", e)))?;

        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(salt.as_bytes(), &salt_string)
            .map_err(|e| AppError::Internal(format!("Key derivation failed: {}", e)))?;

        let hash_bytes = password_hash
            .hash
            .ok_or_else(|| AppError::Internal("Failed to extract hast".to_string()))?;

        let key_bytes = hash_bytes.as_bytes();
        if key_bytes.len() < 32 {
            return Err(AppError::Internal("Key too short".to_string()));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes[..32]);

        let cipher = ChaCha20Poly1305::new(&key.into());

        Ok(Self { cipher })
    }

    pub fn encode(&self, plaintext: &str) -> Result<String, AppError> {
        // Generate random nonce (12 bytes for ChaCha20Poly1305)
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| AppError::Internal(format!("Encryption failed: {}", e)))?;

        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);

        Ok(general_purpose::STANDARD.encode(result))
    }

    pub fn decode(&self, encoded: &str) -> Result<String, AppError> {
        let data = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| AppError::Internal(format!("Invalid base64: {}", e)))?;

        if data.len() < 12 {
            return Err(AppError::Internal("Data too short".to_string()));
        }

        let (nonce_bytes, cipher_text) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plain_text = self
            .cipher
            .decrypt(nonce, cipher_text)
            .map_err(|e| AppError::Internal(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plain_text)
            .map_err(|e| AppError::Internal(format!("Invalid UTF-8: {}", e)))
    }
}
