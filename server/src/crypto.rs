use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn cipher(secret: &str) -> Aes256Gcm { Aes256Gcm::new_from_slice(&Sha256::digest(secret.as_bytes())).expect("32-byte key") }

pub fn encrypt(secret: &str, plaintext: &[u8]) -> Result<String, String> {
    let nonce_source = Uuid::new_v4();
    let nonce_bytes = &nonce_source.as_bytes()[..12];
    let encrypted = cipher(secret).encrypt(Nonce::from_slice(nonce_bytes), plaintext).map_err(|_| "credential encryption failed")?;
    let mut value = nonce_bytes.to_vec(); value.extend(encrypted); Ok(STANDARD.encode(value))
}

pub fn decrypt(secret: &str, encoded: &str) -> Result<Vec<u8>, String> {
    let value = STANDARD.decode(encoded).map_err(|_| "invalid encrypted credential")?;
    if value.len() < 13 { return Err("invalid encrypted credential".into()); }
    cipher(secret).decrypt(Nonce::from_slice(&value[..12]), &value[12..]).map_err(|_| "credential decryption failed".into())
}
