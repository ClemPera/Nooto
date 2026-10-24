use anyhow::{Result, Context};
use std::sync::OnceLock;
use std::time::Duration;
use tauri_plugin_log::log::trace;

use crate::{crypt, db::schema::Workspace};

pub mod operations;
pub mod service;

/// Timeout for establishing a connection to the server.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Total timeout for a sync request, including reading the response body.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Shared HTTP client for all sync requests.
pub fn http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| build_http_client(CONNECT_TIMEOUT, REQUEST_TIMEOUT))
}

fn build_http_client(connect_timeout: Duration, request_timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()
        .expect("Failed to build HTTP client")
}

/// Joins a server base URL with an endpoint path, avoiding duplicate slashes.
pub fn endpoint(instance: &str, path: &str) -> String {
    format!(
        "{}/{}",
        instance.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn client_times_out_on_stalled_response() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
        });

        let client = build_http_client(Duration::from_millis(100), Duration::from_millis(500));
        let error = client
            .get(format!("http://{addr}/"))
            .send()
            .await
            .expect_err("request should time out");

        assert!(error.is_timeout());
    }

    #[test]
    fn endpoint_adds_missing_slash() {
        assert_eq!(
            endpoint("https://example.com", "/notes"),
            "https://example.com/notes"
        );
    }

    #[test]
    fn endpoint_ignores_trailing_slashes() {
        assert_eq!(
            endpoint("https://example.com/", "/notes"),
            "https://example.com/notes"
        );
        assert_eq!(
            endpoint("https://example.com///", "notes"),
            "https://example.com/notes"
        );
    }

    #[test]
    fn endpoint_preserves_base_path() {
        assert_eq!(
            endpoint("https://example.com/nooto/", "/notes"),
            "https://example.com/nooto/notes"
        );
    }
}
