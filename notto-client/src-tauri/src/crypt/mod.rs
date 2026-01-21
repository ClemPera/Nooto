use aes_gcm::{AeadCore, Aes256Gcm, Key, KeyInit, Nonce, aead::{Aead, Payload}, aes::Aes256};
use argon2::{
    Argon2, password_hash::{
        self, PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::{OsRng, RngCore}
    }
};
use bip39::Language;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::from_slice;
use shared::LoginRequest;
use tauri_plugin_log::log::{trace, debug, info};

use crate::db::schema;

#[derive(Serialize, Deserialize, Debug)]
pub struct NoteData {
    pub id: Vec<u8>,
    pub title: String,
    pub content: String,
    pub updated_at: i64,
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

pub fn create_workspace() -> WorkspaceEncryptionData {
    //Generate encryption key
    let master_encryption_key: Key<Aes256Gcm> = Aes256Gcm::generate_key(OsRng).into();

    //Generate recovery keys for auth and data
    let recovery_key_data = bip39::Mnemonic::generate_in(Language::English, 24)
        .unwrap()
        .to_string();

    //Init AesGcm and Argon2
    let argon2 = Argon2::default();

    //Generate needed salts
    let salt_recovery_data = SaltString::generate(&mut OsRng);

    let recovery_hash_data = argon2
        .hash_password(recovery_key_data.as_bytes(), &salt_recovery_data)
        .unwrap();

    let recovery_key_hash = recovery_hash_data.hash.unwrap();

    let recovery_key = Key::<Aes256Gcm>::from_slice(recovery_key_hash.as_bytes());
    let cipher = Aes256Gcm::new(recovery_key);
    
    //Generate nonce for mek password/recovery
    let mek_recovery_nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    
    //Generate hash for mek password and recovery
    let encrypted_mek_recovery = cipher
    .encrypt(&mek_recovery_nonce, master_encryption_key.as_slice())
    .unwrap();

    WorkspaceEncryptionData {
        master_encryption_key,
        recovery_key_data,
        salt_recovery_data,
        mek_recovery_nonce:  mek_recovery_nonce.to_vec(),
        encrypted_mek_recovery,
    }
}


pub fn create_account(password: String, mek: Key<Aes256Gcm>) -> AccountEncryptionData {
    //Generate recovery keys for auth and data
    let recovery_key_auth = bip39::Mnemonic::generate_in(Language::English, 24)
        .unwrap()
        .to_string();

    //Init AesGcm and Argon2
    let argon2 = Argon2::default();

    //Generate needed salts
    let salt_auth = SaltString::generate(&mut OsRng);
    let salt_data = SaltString::generate(&mut OsRng);
    let salt_recovery_auth = SaltString::generate(&mut OsRng);
    let salt_server_auth = SaltString::generate(&mut OsRng);
    let salt_server_recovery = SaltString::generate(&mut OsRng);

    //Generate hash for password and data
    let password_hash_auth = argon2
        .hash_password(password.as_bytes(), &salt_auth)
        .unwrap()
        .to_string();
    let recovery_hash_auth = argon2
        .hash_password(recovery_key_auth.as_bytes(), &salt_recovery_auth)
        .unwrap()
        .to_string();
    let password_hash_data = argon2
        .hash_password(password.as_bytes(), &salt_data)
        .unwrap();

    let password_key_hash = password_hash_data.hash.unwrap();

    let password_key = Key::<Aes256Gcm>::from_slice(password_key_hash.as_bytes());
    let cipher = Aes256Gcm::new(password_key);

    //Generate nonce for mek password/recovery
    let mek_password_nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    //Generate hash for mek password and recovery
    let encrypted_mek_password = cipher
        .encrypt(&mek_password_nonce, mek.as_slice())
        .unwrap();

    //Generate hashs for password and recovery stored on server
    let stored_password_hash = argon2
        .hash_password(password_hash_auth.as_bytes(), &salt_server_auth)
        .unwrap()
        .to_string();
    let stored_recovery_hash = argon2
        .hash_password(recovery_hash_auth.as_bytes(), &salt_server_recovery)
        .unwrap()
        .to_string();


    AccountEncryptionData {
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
    }
}

/// Returns login hash
pub fn login(login_request: LoginRequest, password: String) -> String {
    let argon2 = Argon2::default();

    let salt_auth = SaltString::from_b64(&login_request.salt_auth).unwrap();
    let salt_server_auth = SaltString::from_b64(&login_request.salt_server_auth).unwrap();

    let password_hash_auth = argon2.hash_password(password.as_bytes(), &salt_auth)
        .unwrap()
        .to_string();

    argon2.hash_password(password_hash_auth.as_bytes(), &salt_server_auth)
        .unwrap()
        .to_string()
}

pub fn decrypt_mek(password: String, encrypted_mek_password: Vec<u8>, salt_data: String, mek_password_nonce: Vec<u8>) -> Key<Aes256Gcm> {
    let argon2 = Argon2::default();

    let salt_data = SaltString::from_b64(&salt_data).unwrap();

    let password_hash_data = argon2
        .hash_password(password.as_bytes(), &salt_data)
        .unwrap();

    let password_key_hash = password_hash_data.hash.unwrap();
    let password_key = Key::<Aes256Gcm>::from_slice(password_key_hash.as_bytes());
    
    let cipher = Aes256Gcm::new(password_key);

    let mek_slice = cipher.decrypt(Nonce::from_slice(&mek_password_nonce), encrypted_mek_password.as_slice()).unwrap();

    let mek = Key::<Aes256Gcm>::from_slice(&mek_slice);

    mek.to_owned()
}

pub fn encrypt_note(
    content: String,
    master_encryption_key: Key<Aes256Gcm>,
) -> Result<(Vec<u8>, Vec<u8>), Box<dyn std::error::Error>> {
    //Encrypt
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let cipher = Aes256Gcm::new(&master_encryption_key);

    let ciphertext = cipher.encrypt(&nonce, content.as_bytes()).unwrap();

    Ok((ciphertext, nonce.to_vec()))
}

pub fn decrypt_note(note: schema::Note, mek: Key<Aes256Gcm>) -> Result<NoteData, Box<dyn std::error::Error>> {
    let nonce_array: [u8; 12] = note.nonce.try_into().expect("nonce must be 12 bytes");
    let nonce = Nonce::from(nonce_array);

    let cipher = Aes256Gcm::new(&mek);
    let plaintext = cipher.decrypt(&nonce, note.content.as_ref()).unwrap();
    let data_unser = NoteData {
        id: note.uuid,
        title: note.title,
        content: String::from_utf8(plaintext).unwrap(),
        updated_at: note.updated_at
    };

    Ok(data_unser)
}
