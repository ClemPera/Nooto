use aes_gcm::{
    aead::Aead,
    AeadCore, Aes256Gcm, Key, KeyInit, Nonce,
};
use anyhow::{Context, Result};
use argon2::{
    password_hash::{
        rand_core::{OsRng, RngCore},
        PasswordHasher, SaltString,
    },
    Argon2,
};
use bip39::Language;
use serde::{Deserialize, Serialize};
use shared::LoginRequest;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NoteData {
    pub id: String,
    pub title: String,
    pub parent_id: Option<String>,
    pub is_folder: bool,
    pub folder_open: bool,
    pub content: String,
    pub updated_at: i64,
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NoteMetadata {
    pub title: String,
    pub parent_id: Option<String>,
    pub is_folder: bool,
    pub folder_open: bool,
}

#[derive(Debug)]
pub struct AccountEncryptionData {
    pub recovery_key_auth: String,
    pub salt_auth: SaltString,
    pub salt_data: SaltString,
    pub salt_recovery_auth: SaltString,
    pub salt_server_auth: SaltString,
    pub salt_server_recovery: SaltString,

    pub mek_password_nonce: Vec<u8>,

    pub encrypted_mek_password: Vec<u8>,

    pub stored_password_hash: String,
    pub stored_recovery_hash: String,
}

#[derive(Debug)]
pub struct WorkspaceEncryptionData {
    pub master_encryption_key: Key<Aes256Gcm>,
    pub recovery_key_data: String,
    pub salt_recovery_data: SaltString,
    pub mek_recovery_nonce: Vec<u8>,
    pub encrypted_mek_recovery: Vec<u8>,
}

pub fn create_workspace() -> Result<WorkspaceEncryptionData> {
    let master_encryption_key: Key<Aes256Gcm> = Aes256Gcm::generate_key(OsRng).into();

    let recovery_key_data = bip39::Mnemonic::generate_in(Language::English, 24)
        .context("Failed to generate recovery mnemonic")?
        .to_string();

    let argon2 = Argon2::default();
    let salt_recovery_data = SaltString::generate(&mut OsRng);

    let recovery_hash_data = argon2
        .hash_password(recovery_key_data.as_bytes(), &salt_recovery_data)
        .map_err(|e| anyhow::anyhow!("Failed to hash recovery key: {e}"))?;

    let recovery_key_hash = recovery_hash_data
        .hash
        .context("Recovery key hash is missing")?;

    let recovery_key = Key::<Aes256Gcm>::from_slice(recovery_key_hash.as_bytes());
    let cipher = Aes256Gcm::new(recovery_key);

    let mek_recovery_nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let encrypted_mek_recovery = cipher
        .encrypt(&mek_recovery_nonce, master_encryption_key.as_slice())
        .map_err(|e| anyhow::anyhow!("Failed to encrypt MEK with recovery key: {e}"))?;

    Ok(WorkspaceEncryptionData {
        master_encryption_key,
        recovery_key_data,
        salt_recovery_data,
        mek_recovery_nonce: mek_recovery_nonce.to_vec(),
        encrypted_mek_recovery,
    })
}

pub fn create_account(password: String, mek: Key<Aes256Gcm>) -> Result<AccountEncryptionData> {
    let recovery_key_auth = bip39::Mnemonic::generate_in(Language::English, 24)
        .context("Failed to generate recovery mnemonic")?
        .to_string();

    let argon2 = Argon2::default();

    let salt_auth = SaltString::generate(&mut OsRng);
    let salt_data = SaltString::generate(&mut OsRng);
    let salt_recovery_auth = SaltString::generate(&mut OsRng);
    let salt_server_auth = SaltString::generate(&mut OsRng);
    let salt_server_recovery = SaltString::generate(&mut OsRng);

    let password_hash_auth = argon2
        .hash_password(password.as_bytes(), &salt_auth)
        .map_err(|e| anyhow::anyhow!("Failed to hash password (auth): {e}"))?
        .to_string();

    let recovery_hash_auth = argon2
        .hash_password(recovery_key_auth.as_bytes(), &salt_recovery_auth)
        .map_err(|e| anyhow::anyhow!("Failed to hash recovery key (auth): {e}"))?
        .to_string();

    let password_hash_data = argon2
        .hash_password(password.as_bytes(), &salt_data)
        .map_err(|e| anyhow::anyhow!("Failed to hash password (data): {e}"))?;

    let password_key_hash = password_hash_data
        .hash
        .context("Password key hash is missing")?;

    let password_key = Key::<Aes256Gcm>::from_slice(password_key_hash.as_bytes());
    let cipher = Aes256Gcm::new(password_key);

    let mek_password_nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let encrypted_mek_password = cipher
        .encrypt(&mek_password_nonce, mek.as_slice())
        .map_err(|e| anyhow::anyhow!("Failed to encrypt MEK with password: {e}"))?;

    let stored_password_hash = argon2
        .hash_password(password_hash_auth.as_bytes(), &salt_server_auth)
        .map_err(|e| anyhow::anyhow!("Failed to hash password for server storage: {e}"))?
        .to_string();

    let stored_recovery_hash = argon2
        .hash_password(recovery_hash_auth.as_bytes(), &salt_server_recovery)
        .map_err(|e| anyhow::anyhow!("Failed to hash recovery key for server storage: {e}"))?
        .to_string();

    Ok(AccountEncryptionData {
        recovery_key_auth,
        salt_auth,
        salt_data,
        salt_recovery_auth,
        salt_server_auth,
        salt_server_recovery,
        mek_password_nonce: mek_password_nonce.to_vec(),
        encrypted_mek_password,
        stored_password_hash,
        stored_recovery_hash,
    })
}

/// Returns login hash
pub fn login(login_request: LoginRequest, password: String) -> Result<String> {
    let argon2 = Argon2::default();

    let salt_auth = SaltString::from_b64(&login_request.salt_auth)
        .map_err(|e| anyhow::anyhow!("Invalid salt_auth from server: {e}"))?;
    let salt_server_auth = SaltString::from_b64(&login_request.salt_server_auth)
        .map_err(|e| anyhow::anyhow!("Invalid salt_server_auth from server: {e}"))?;

    let password_hash_auth = argon2
        .hash_password(password.as_bytes(), &salt_auth)
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {e}"))?
        .to_string();

    argon2
        .hash_password(password_hash_auth.as_bytes(), &salt_server_auth)
        .map_err(|e| anyhow::anyhow!("Failed to hash password for server auth: {e}"))
        .map(|h| h.to_string())
}

pub fn decrypt_mek(
    password: String,
    encrypted_mek_password: Vec<u8>,
    salt_data: String,
    mek_password_nonce: Vec<u8>,
) -> Result<Key<Aes256Gcm>> {
    let argon2 = Argon2::default();

    let salt_data = SaltString::from_b64(&salt_data)
        .map_err(|e| anyhow::anyhow!("Invalid salt_data from server: {e}"))?;

    let password_hash_data = argon2
        .hash_password(password.as_bytes(), &salt_data)
        .map_err(|e| anyhow::anyhow!("Failed to hash password for decryption: {e}"))?;

    let password_key_hash = password_hash_data
        .hash
        .context("Password key hash is missing")?;

    let password_key = Key::<Aes256Gcm>::from_slice(password_key_hash.as_bytes());
    let cipher = Aes256Gcm::new(password_key);

    let mek_slice = cipher
        .decrypt(
            Nonce::from_slice(&mek_password_nonce),
            encrypted_mek_password.as_slice(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to decrypt master encryption key: {e}"))?;

    let mek = Key::<Aes256Gcm>::from_slice(&mek_slice);

    Ok(mek.to_owned())
}

pub fn encrypt_data(data: &[u8], key: &Key<Aes256Gcm>) -> Result<(Vec<u8>, Vec<u8>)> {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let cipher = Aes256Gcm::new(key);
    let ciphertext = cipher
        .encrypt(&nonce, data)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {e}"))?;
    Ok((ciphertext, nonce.to_vec()))
}

pub fn decrypt_data(ciphertext: &[u8], nonce: &[u8], key: &Key<Aes256Gcm>) -> Result<Vec<u8>> {
    let nonce = Nonce::from_slice(nonce);
    let cipher = Aes256Gcm::new(key);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {e}"))?;
    Ok(plaintext)
}
