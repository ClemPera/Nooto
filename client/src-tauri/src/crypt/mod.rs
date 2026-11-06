use aes_gcm::{
    aead::{Aead, Generate, Nonce as AeadNonce},
    Aes256Gcm, Key, KeyInit,
};
use anyhow::{Context, Result};
use argon2::{
    Argon2, PasswordHasher,
    password_hash::phc::SaltString,
};
use bip39::{Language, WordCount};
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
    pub pinned: bool,
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
    // Notes encrypted before this field existed have no `pinned` key; default to unpinned.
    #[serde(default)]
    pub pinned: bool,
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
    let master_encryption_key: Key<Aes256Gcm> = Key::<Aes256Gcm>::generate();

    let recovery_key_data = bip39::Mnemonic::generate_in(Language::English, WordCount::Words24)
        .context("Failed to generate recovery mnemonic")?
        .to_string();

    let argon2 = Argon2::default();
    let salt_recovery_data = SaltString::generate();

    let recovery_hash_data = argon2
        .hash_password_with_salt(recovery_key_data.as_bytes(), &salt_recovery_data.to_salt())
        .map_err(|e| anyhow::anyhow!("Failed to hash recovery key: {e}"))?;

    let recovery_key_hash = recovery_hash_data
        .hash
        .context("Recovery key hash is missing")?;

    let recovery_key: &Key<Aes256Gcm> = recovery_key_hash.as_bytes().try_into()
        .expect("argon2 output must be 32 bytes");
    let cipher = Aes256Gcm::new(recovery_key);

    let mek_recovery_nonce = AeadNonce::<Aes256Gcm>::generate();

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
    let recovery_key_auth = bip39::Mnemonic::generate_in(Language::English, WordCount::Words24)
        .context("Failed to generate recovery mnemonic")?
        .to_string();

    let argon2 = Argon2::default();

    let salt_auth = SaltString::generate();
    let salt_data = SaltString::generate();
    let salt_recovery_auth = SaltString::generate();
    let salt_server_auth = SaltString::generate();
    let salt_server_recovery = SaltString::generate();

    let password_hash_auth = argon2
        .hash_password_with_salt(password.as_bytes(), &salt_auth.to_salt())
        .map_err(|e| anyhow::anyhow!("Failed to hash password (auth): {e}"))?
        .to_string();

    let recovery_hash_auth = argon2
        .hash_password_with_salt(recovery_key_auth.as_bytes(), &salt_recovery_auth.to_salt())
        .map_err(|e| anyhow::anyhow!("Failed to hash recovery key (auth): {e}"))?
        .to_string();

    let password_hash_data = argon2
        .hash_password_with_salt(password.as_bytes(), &salt_data.to_salt())
        .map_err(|e| anyhow::anyhow!("Failed to hash password (data): {e}"))?;

    let password_key_hash = password_hash_data
        .hash
        .context("Password key hash is missing")?;

    let password_key: &Key<Aes256Gcm> = password_key_hash.as_bytes().try_into()
        .expect("argon2 output must be 32 bytes");
    let cipher = Aes256Gcm::new(password_key);

    let mek_password_nonce = AeadNonce::<Aes256Gcm>::generate();

    let encrypted_mek_password = cipher
        .encrypt(&mek_password_nonce, mek.as_slice())
        .map_err(|e| anyhow::anyhow!("Failed to encrypt MEK with password: {e}"))?;

    let stored_password_hash = argon2
        .hash_password_with_salt(password_hash_auth.as_bytes(), &salt_server_auth.to_salt())
        .map_err(|e| anyhow::anyhow!("Failed to hash password for server storage: {e}"))?
        .to_string();

    let stored_recovery_hash = argon2
        .hash_password_with_salt(recovery_hash_auth.as_bytes(), &salt_server_recovery.to_salt())
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
        .hash_password_with_salt(password.as_bytes(), &salt_auth.to_salt())
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {e}"))?
        .to_string();

    argon2
        .hash_password_with_salt(password_hash_auth.as_bytes(), &salt_server_auth.to_salt())
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
        .hash_password_with_salt(password.as_bytes(), &salt_data.to_salt())
        .map_err(|e| anyhow::anyhow!("Failed to hash password for decryption: {e}"))?;

    let password_key_hash = password_hash_data
        .hash
        .context("Password key hash is missing")?;

    let password_key: &Key<Aes256Gcm> = password_key_hash.as_bytes().try_into()
        .expect("argon2 output must be 32 bytes");
    let cipher = Aes256Gcm::new(password_key);

    let mek_nonce: AeadNonce<Aes256Gcm> = mek_password_nonce.as_slice().try_into()
        .expect("MEK nonce must be 12 bytes");
    let mek_slice = cipher
        .decrypt(
            &mek_nonce,
            encrypted_mek_password.as_slice(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to decrypt master encryption key: {e}"))?;

    let mek: &Key<Aes256Gcm> = mek_slice.as_slice().try_into().expect("MEK must be 32 bytes");

    Ok(mek.to_owned())
}

/// Encrypts `data` with AES-256-GCM using a fresh random nonce.
/// Returns `(ciphertext, nonce)`.
pub fn encrypt_data(data: &[u8], key: &Key<Aes256Gcm>) -> Result<(Vec<u8>, Vec<u8>)> {
    let nonce = AeadNonce::<Aes256Gcm>::generate();
    let cipher = Aes256Gcm::new(key);
    let ciphertext = cipher
        .encrypt(&nonce, data)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {e}"))?;
    Ok((ciphertext, nonce.to_vec()))
}

/// Decrypts AES-256-GCM `ciphertext` with the given `nonce` and `key`.
pub fn decrypt_data(ciphertext: &[u8], nonce: &[u8], key: &Key<Aes256Gcm>) -> Result<Vec<u8>> {
    let nonce: &AeadNonce<Aes256Gcm> = nonce.try_into().expect("nonce must be 12 bytes");
    let cipher = Aes256Gcm::new(key);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed: {e}"))?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::password_hash::phc::PasswordHash;
    use bip39::Mnemonic;

    fn random_key() -> Key<Aes256Gcm> {
        Key::<Aes256Gcm>::generate()
    }

    fn parse_phc(s: &str) -> PasswordHash {
        PasswordHash::new(s).expect("expected valid PHC string")
    }

    fn word_count(phrase: &str) -> usize {
        phrase.split_whitespace().count()
    }

    fn assert_valid_24_word_mnemonic(phrase: &str) {
        assert_eq!(word_count(phrase), 24, "mnemonic must be 24 words");
        Mnemonic::parse_in(Language::English, phrase)
            .expect("mnemonic must be valid BIP39 English");
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = random_key();
        let plaintext = b"hello, nooto!";

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

    #[test]
    fn encrypt_data_output_lengths() {
        let key = random_key();

        let (ct_empty, nonce_empty) = encrypt_data(b"", &key).unwrap();
        assert_eq!(nonce_empty.len(), 12);
        assert_eq!(ct_empty.len(), 16, "empty plaintext + 16-byte tag");

        let (ct_5, nonce_5) = encrypt_data(b"hello", &key).unwrap();
        assert_eq!(nonce_5.len(), 12);
        assert_eq!(ct_5.len(), 5 + 16);
    }

    #[test]
    fn decrypt_data_tampered_nonce_fails() {
        let key = random_key();
        let (ciphertext, mut nonce) = encrypt_data(b"data", &key).unwrap();
        nonce[0] ^= 0xFF;

        assert!(decrypt_data(&ciphertext, &nonce, &key).is_err());
    }

    #[test]
    #[should_panic]
    fn decrypt_data_wrong_length_nonce_panics() {
        let key = random_key();
        let (ciphertext, nonce) = encrypt_data(b"data", &key).unwrap();
        let _ = decrypt_data(&ciphertext, &nonce[..8], &key);
    }

    #[test]
    fn decrypt_data_empty_ciphertext_fails() {
        let key = random_key();
        let (_, nonce) = encrypt_data(b"data", &key).unwrap();

        assert!(decrypt_data(&[], &nonce, &key).is_err());
    }

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

    #[test]
    fn decrypt_mek_invalid_salt_data_fails() {
        let account_data = create_account("password".to_string(), random_key()).unwrap();

        let result = decrypt_mek(
            "password".to_string(),
            account_data.encrypted_mek_password,
            "!!!not-base64!!!".to_string(),
            account_data.mek_password_nonce,
        );

        assert!(result.is_err());
    }

    #[test]
    fn decrypt_mek_tampered_ciphertext_and_nonce_fail() {
        let password = "password".to_string();
        let account_data = create_account(password.clone(), random_key()).unwrap();

        let mut bad_ct = account_data.encrypted_mek_password.clone();
        bad_ct[0] ^= 0xFF;
        assert!(decrypt_mek(
            password.clone(),
            bad_ct,
            account_data.salt_data.to_string(),
            account_data.mek_password_nonce.clone(),
        )
        .is_err());

        let mut bad_nonce = account_data.mek_password_nonce.clone();
        bad_nonce[0] ^= 0xFF;
        assert!(decrypt_mek(
            password,
            account_data.encrypted_mek_password,
            account_data.salt_data.to_string(),
            bad_nonce,
        )
        .is_err());
    }

    #[test]
    #[should_panic]
    fn decrypt_mek_truncated_nonce_panics() {
        let password = "password".to_string();
        let account_data = create_account(password.clone(), random_key()).unwrap();
        let short_nonce = account_data.mek_password_nonce[..8].to_vec();

        let _ = decrypt_mek(
            password,
            account_data.encrypted_mek_password,
            account_data.salt_data.to_string(),
            short_nonce,
        );
    }

    #[test]
    fn create_workspace_outputs_differ_across_calls() {
        let w1 = create_workspace().unwrap();
        let w2 = create_workspace().unwrap();

        assert_ne!(
            w1.master_encryption_key.as_slice(),
            w2.master_encryption_key.as_slice()
        );
        assert_ne!(w1.recovery_key_data, w2.recovery_key_data);
        assert_ne!(
            w1.salt_recovery_data.to_string(),
            w2.salt_recovery_data.to_string()
        );
        assert_ne!(w1.mek_recovery_nonce, w2.mek_recovery_nonce);
        assert_ne!(w1.encrypted_mek_recovery, w2.encrypted_mek_recovery);
    }

    #[test]
    fn create_workspace_field_shapes() {
        let w = create_workspace().unwrap();

        assert_valid_24_word_mnemonic(&w.recovery_key_data);
        SaltString::from_b64(&w.salt_recovery_data.to_string())
            .expect("salt_recovery_data must parse as SaltString");
        assert_eq!(w.mek_recovery_nonce.len(), 12);
        assert_eq!(w.encrypted_mek_recovery.len(), 32 + 16);
        assert_eq!(w.master_encryption_key.as_slice().len(), 32);
    }

    #[test]
    fn create_workspace_recovery_key_decrypts_mek() {
        let w = create_workspace().unwrap();
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password_with_salt(
                w.recovery_key_data.as_bytes(),
                &w.salt_recovery_data.to_salt(),
            )
            .unwrap();
        let output = hash.hash.expect("hash missing");
        let recovery_key: &Key<Aes256Gcm> = output
            .as_bytes()
            .try_into()
            .expect("argon2 output must be 32 bytes");
        let nonce: &AeadNonce<Aes256Gcm> = w.mek_recovery_nonce.as_slice().try_into().expect("nonce must be 12 bytes");
        let plaintext = Aes256Gcm::new(recovery_key)
            .decrypt(nonce, w.encrypted_mek_recovery.as_slice())
            .unwrap();

        assert_eq!(plaintext, w.master_encryption_key.as_slice());
    }

    #[test]
    fn create_account_outputs_differ_across_calls() {
        let mek = random_key();
        let a1 = create_account("same-password".to_string(), mek.clone()).unwrap();
        let a2 = create_account("same-password".to_string(), mek).unwrap();

        for (s1, s2) in [
            (&a1.salt_auth, &a2.salt_auth),
            (&a1.salt_data, &a2.salt_data),
            (&a1.salt_recovery_auth, &a2.salt_recovery_auth),
            (&a1.salt_server_auth, &a2.salt_server_auth),
            (&a1.salt_server_recovery, &a2.salt_server_recovery),
        ] {
            assert_ne!(s1.to_string(), s2.to_string());
        }
        assert_ne!(a1.mek_password_nonce, a2.mek_password_nonce);
        assert_ne!(a1.encrypted_mek_password, a2.encrypted_mek_password);
        assert_ne!(a1.stored_password_hash, a2.stored_password_hash);
        assert_ne!(a1.stored_recovery_hash, a2.stored_recovery_hash);
        assert_ne!(a1.recovery_key_auth, a2.recovery_key_auth);
    }

    #[test]
    fn create_account_salts_distinct_and_valid() {
        let a = create_account("password".to_string(), random_key()).unwrap();
        let salts = [
            a.salt_auth.to_string(),
            a.salt_data.to_string(),
            a.salt_recovery_auth.to_string(),
            a.salt_server_auth.to_string(),
            a.salt_server_recovery.to_string(),
        ];

        let mut dedup = salts.to_vec();
        dedup.sort();
        dedup.dedup();
        assert_eq!(dedup.len(), 5, "all 5 salts must be distinct");

        for s in &salts {
            SaltString::from_b64(s).expect("salt must parse as SaltString");
        }
    }

    #[test]
    fn create_account_field_shapes() {
        let a = create_account("password".to_string(), random_key()).unwrap();

        assert_valid_24_word_mnemonic(&a.recovery_key_auth);
        assert_eq!(a.mek_password_nonce.len(), 12);
        assert_eq!(a.encrypted_mek_password.len(), 32 + 16);
        parse_phc(&a.stored_password_hash);
        parse_phc(&a.stored_recovery_hash);
        assert_ne!(a.stored_password_hash, a.stored_recovery_hash);
    }

    fn login_request_for(
        password: &str,
        mek: Key<Aes256Gcm>,
    ) -> (AccountEncryptionData, LoginRequest) {
        let account = create_account(password.to_string(), mek).unwrap();
        let request = LoginRequest {
            salt_auth: account.salt_auth.to_string(),
            salt_server_auth: account.salt_server_auth.to_string(),
        };
        (account, request)
    }

    #[test]
    fn login_reproduces_stored_password_hash() {
        let (account, request) = login_request_for("correct_password", random_key());

        let login_hash = login(request, "correct_password".to_string()).unwrap();

        parse_phc(&login_hash);
        assert_eq!(login_hash, account.stored_password_hash);
    }

    #[test]
    fn login_wrong_password_returns_different_hash() {
        let (account, request) = login_request_for("correct_password", random_key());

        let login_hash = login(request, "wrong_password".to_string()).unwrap();

        parse_phc(&login_hash);
        assert_ne!(login_hash, account.stored_password_hash);
    }

    #[test]
    fn login_invalid_salts_fail() {
        let (_, good) = login_request_for("password", random_key());

        let bad_auth = LoginRequest {
            salt_auth: "!!!not-base64!!!".to_string(),
            salt_server_auth: good.salt_server_auth.clone(),
        };
        assert!(login(bad_auth, "password".to_string()).is_err());

        let bad_server = LoginRequest {
            salt_auth: good.salt_auth.clone(),
            salt_server_auth: "!!!not-base64!!!".to_string(),
        };
        assert!(login(bad_server, "password".to_string()).is_err());
    }

    #[test]
    fn login_empty_password_succeeds() {
        let (_, request) = login_request_for("password", random_key());
        let result = login(request, String::new());
        assert!(result.is_ok());
        parse_phc(&result.unwrap());
    }

    #[test]
    fn old_salts_unlock_same_hashes() {
        // Pinned vectors generated on main pre-migration.
        let expected_auth = "$argon2id$v=19$m=19456,t=2,p=1$BT0o9jV3EHXq0pDtuKwhnQ$i5JO+R8rpB3Oz6QIxJnaJPJng0gL8cvzsrv2tTfqlmM";
        let expected_data = "$argon2id$v=19$m=19456,t=2,p=1$r0hiAHmXmVpkzg4Ii96Ehw$6Er/i1PRRBD9Sw6iLi6Oj8C5BzAaSvgXOEZdnhQAmg4";

        let salt_auth = SaltString::from_b64("BT0o9jV3EHXq0pDtuKwhnQ")
            .expect("old salt_auth must parse");
        let salt_data = SaltString::from_b64("r0hiAHmXmVpkzg4Ii96Ehw")
            .expect("old salt_data must parse");

        let argon2 = Argon2::default();
        let hash_auth = argon2
            .hash_password_with_salt(b"johndoe", &salt_auth.to_salt())
            .expect("auth re-derive must succeed")
            .to_string();
        let hash_data = argon2
            .hash_password_with_salt(b"johndoe", &salt_data.to_salt())
            .expect("data re-derive must succeed")
            .to_string();

        assert_eq!(hash_auth, expected_auth);
        assert_eq!(hash_data, expected_data);
    }
}
