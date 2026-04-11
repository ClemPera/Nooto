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

/// Decrypted note data passed between commands and the frontend.
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

/// Plaintext metadata stored encrypted alongside each note.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NoteMetadata {
    pub title: String,
    pub parent_id: Option<String>,
    pub is_folder: bool,
    pub folder_open: bool,
}

/// Cryptographic material produced during account creation, sent to the server.
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

/// Cryptographic material generated when a workspace is created, stored locally.
#[derive(Debug)]
pub struct WorkspaceEncryptionData {
    pub master_encryption_key: Key<Aes256Gcm>,
    pub recovery_key_data: String,
    pub salt_recovery_data: SaltString,
    pub mek_recovery_nonce: Vec<u8>,
    pub encrypted_mek_recovery: Vec<u8>,
}

/// Generates a fresh AES-256-GCM master encryption key (MEK) and encrypts it with a
/// BIP-39 recovery key. Returns all material needed to bootstrap a new workspace.
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

/// Derives all server-side hashes and encrypts the MEK with the user's password.
/// Returns the data to be sent to the server during account registration.
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

/// Derives the login hash from the password using the salts returned by the server.
/// Returns a string to be sent as `login_hash` in `POST /login`.
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

/// Decrypts the master encryption key using the user's password and the server-provided salts.
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

/// Encrypts `data` with AES-256-GCM using a fresh random nonce.
/// Returns `(ciphertext, nonce)`.
pub fn encrypt_data(data: &[u8], key: &Key<Aes256Gcm>) -> Result<(Vec<u8>, Vec<u8>)> {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let cipher = Aes256Gcm::new(key);
    let ciphertext = cipher
        .encrypt(&nonce, data)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {e}"))?;
    Ok((ciphertext, nonce.to_vec()))
}

/// Decrypts AES-256-GCM `ciphertext` with the given `nonce` and `key`.
pub fn decrypt_data(ciphertext: &[u8], nonce: &[u8], key: &Key<Aes256Gcm>) -> Result<Vec<u8>> {
    let nonce = Nonce::from_slice(nonce);
    let cipher = Aes256Gcm::new(key);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {e}"))?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::{Aes256Gcm, KeyInit};
    use argon2::password_hash::rand_core::{OsRng, RngCore};

    fn random_key() -> Key<Aes256Gcm> {
        Aes256Gcm::generate_key(OsRng)
    }

    // --- encrypt_data / decrypt_data ---

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = random_key();
        let plaintext = b"hello, notto!";

        let (ciphertext, nonce) = encrypt_data(plaintext, &key).unwrap();
        let decrypted = decrypt_data(&ciphertext, &nonce, &key).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_produces_different_nonces_each_call() {
        let key = random_key();
        let data = b"same data";

        let (_, nonce1) = encrypt_data(data, &key).unwrap();
        let (_, nonce2) = encrypt_data(data, &key).unwrap();

        assert_ne!(nonce1, nonce2);
    }

    #[test]
    fn encrypt_produces_different_ciphertexts_each_call() {
        let key = random_key();
        let data = b"same data";

        let (ct1, _) = encrypt_data(data, &key).unwrap();
        let (ct2, _) = encrypt_data(data, &key).unwrap();

        assert_ne!(ct1, ct2);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let key = random_key();
        let wrong_key = random_key();
        let plaintext = b"secret";

        let (ciphertext, nonce) = encrypt_data(plaintext, &key).unwrap();
        let result = decrypt_data(&ciphertext, &nonce, &wrong_key);

        assert!(result.is_err());
    }

    #[test]
    fn decrypt_with_tampered_ciphertext_fails() {
        let key = random_key();
        let (mut ciphertext, nonce) = encrypt_data(b"data", &key).unwrap();
        ciphertext[0] ^= 0xFF;

        let result = decrypt_data(&ciphertext, &nonce, &key);
        assert!(result.is_err());
    }

    #[test]
    fn encrypt_empty_data_roundtrip() {
        let key = random_key();
        let (ciphertext, nonce) = encrypt_data(b"", &key).unwrap();
        let decrypted = decrypt_data(&ciphertext, &nonce, &key).unwrap();
        assert_eq!(decrypted, b"");
    }

    // --- decrypt_mek ---

    #[test]
    fn decrypt_mek_roundtrip() {
        let password = "correct_password".to_string();
        let mek = random_key();

        let account_data = create_account(password.clone(), mek).unwrap();

        let recovered_mek = decrypt_mek(
            password,
            account_data.encrypted_mek_password,
            account_data.salt_data.to_string(),
            account_data.mek_password_nonce,
        )
        .unwrap();

        assert_eq!(mek.as_slice(), recovered_mek.as_slice());
    }

    #[test]
    fn decrypt_mek_with_wrong_password_fails() {
        let mek = random_key();
        let account_data = create_account("correct".to_string(), mek).unwrap();

        let result = decrypt_mek(
            "wrong_password".to_string(),
            account_data.encrypted_mek_password,
            account_data.salt_data.to_string(),
            account_data.mek_password_nonce,
        );

        assert!(result.is_err());
    }
}
