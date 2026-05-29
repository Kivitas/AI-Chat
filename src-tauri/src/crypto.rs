use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{password_hash::SaltString, Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityFile {
    pub salt: String,
    pub memory_cost_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub wrapped_master_key: String,
    pub wrapped_master_key_nonce: String,
}

pub fn default_security_file(password: &str) -> AppResult<(SecurityFile, Vec<u8>)> {
    let mut salt_bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|error| AppError::Message(format!("Failed to encode salt: {error}")))?;

    let memory_cost_kib = 65_536;
    let iterations = 3;
    let parallelism = 1;

    let kek = derive_key(
        password,
        salt.as_ref(),
        memory_cost_kib,
        iterations,
        parallelism,
    )?;
    let mut master_key = vec![0_u8; 32];
    OsRng.fill_bytes(&mut master_key);
    let (wrapped_master_key, wrapped_master_key_nonce) = encrypt_bytes(&kek, &master_key)?;

    Ok((
        SecurityFile {
            salt: salt.to_string(),
            memory_cost_kib,
            iterations,
            parallelism,
            wrapped_master_key,
            wrapped_master_key_nonce,
        },
        master_key,
    ))
}

pub fn unlock_master_key(password: &str, security: &SecurityFile) -> AppResult<Vec<u8>> {
    let kek = derive_key(
        password,
        &security.salt,
        security.memory_cost_kib,
        security.iterations,
        security.parallelism,
    )?;
    decrypt_bytes(
        &kek,
        &security.wrapped_master_key_nonce,
        &security.wrapped_master_key,
    )
    .map_err(|_| AppError::PasswordVerificationFailed)
}

pub fn derive_key(
    password: &str,
    salt: &str,
    memory_cost_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> AppResult<Vec<u8>> {
    let params = Params::new(memory_cost_kib, iterations, parallelism, Some(32))
        .map_err(|error| AppError::Message(format!("Argon2 parameter error: {error}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut output = vec![0_u8; 32];
    argon
        .hash_password_into(password.as_bytes(), salt.as_bytes(), &mut output)
        .map_err(|error| AppError::Message(format!("Argon2 hashing error: {error}")))?;
    Ok(output)
}

pub fn encrypt_bytes(key: &[u8], plaintext: &[u8]) -> AppResult<(String, String)> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AppError::Encryption)?;
    let mut nonce_bytes = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| AppError::Encryption)?;
    Ok((STANDARD.encode(ciphertext), STANDARD.encode(nonce_bytes)))
}

pub fn decrypt_bytes(key: &[u8], nonce_b64: &str, ciphertext_b64: &str) -> AppResult<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AppError::Encryption)?;
    let nonce_bytes = STANDARD.decode(nonce_b64)?;
    let ciphertext = STANDARD.decode(ciphertext_b64)?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| AppError::Encryption)
}
