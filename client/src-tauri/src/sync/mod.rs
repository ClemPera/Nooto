use anyhow::{Result, Context};
use tauri_plugin_log::log::trace;

use crate::{crypt, db::schema::Workspace};

pub mod operations;
pub mod service;

/// Assembles the `shared::User` payload from workspace and account encryption data,
/// then posts it to the server's `/create_account` endpoint.
pub async fn create_account(
    workspace: Workspace,
    username: String,
    account: crypt::AccountEncryptionData,
    instance: Option<String>,
) -> Result<()> {

    let instance: String = instance.context("Instance url is empty")?;

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

    operations::create_account(send_user, instance).await
}

/// Fetches the server salts, derives the login hash locally, and submits credentials.
/// Returns the server's `Login` response on success.
pub async fn login(username: String, password: String, instance: String) -> Result<shared::Login> {
    trace!("requesting login...");

    let request_params = shared::LoginRequestParams {
        username: username.clone(),
    };

    let login_request = operations::login_request(request_params, instance.clone()).await?;

    trace!("hashing login...");

    let login_hash = crypt::login(login_request, password)?;

    trace!("loggin in...");

    let login_params = shared::LoginParams {
        username,
        login_hash,
    };

    operations::login(login_params, instance)
        .await
}
