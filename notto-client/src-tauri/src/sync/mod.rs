use rusqlite::Connection;
use tokio::sync::Mutex;
use crate::{crypt, schema::Workspace};
use tauri_plugin_log::log::{trace, debug};

mod operations;
pub mod service;

pub async fn create_account(workspace: Workspace, username: String, account: crypt::AccountEncryptionData, instance: Option<String>){
    let instance = match instance {
        Some(i) => i,
        None => "http://localhost:3000".to_string()
    };

    let send_user = shared::User {
        id: None,
        username,
        stored_password_hash: account.stored_password_hash,
        stored_recovery_hash: account.stored_recovery_hash,
        encrypted_mek_password: account.encrypted_mek_password,
        mek_password_nonce: account.mek_password_nonce,
        encrypted_mek_recovery: workspace.encrypted_mek_recovery,
        mek_recovery_nonce: workspace.mek_recovery_nonce,
        salt_auth: account.salt_auth.to_string(),
        salt_data: account.salt_data.to_string(),
        salt_recovery_auth: account.salt_recovery_auth.to_string(),
        salt_recovery_data: workspace.salt_recovery_data.to_string(),
        salt_server_auth: account.salt_server_auth.to_string(),
        salt_server_recovery: account.salt_server_recovery.to_string(),
    };

    operations::create_account(send_user, instance).await.unwrap();
}

pub async fn login(username: String, password: String, instance: String) -> shared::Login{
    trace!("requesting login...");
    //Request login
    let request_params = shared::LoginRequestParams {
        username: username.clone()
    };
    
    let login_request = operations::login_request(request_params, instance.clone()).await.unwrap();
    
    trace!("hashing login...");
    //Hash
    let login_hash = crypt::login(login_request, password);
    
    //Login
    trace!("loggin in...");
    let login_params = shared::LoginParams {
        username,
        login_hash
    };
    
    operations::login(login_params, instance).await.unwrap()
}