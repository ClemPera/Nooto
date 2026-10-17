use anyhow::{Context, Result};
use shared::{ProtocolCompatibility, ProtocolMismatch, ServerInfo};
use std::fmt;

/// Error returned when the client and server protocol versions are incompatible.
#[derive(Debug)]
pub struct ProtocolMismatchError(pub ProtocolMismatch);

impl fmt::Display for ProtocolMismatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.client_outdated {
            write!(
                f,
                "This app is too old for this server. Update Nooto to continue syncing."
            )
        } else if let Some(server) = &self.0.server {
            write!(
                f,
                "This server (v{}) is too old for this version of Nooto. Update the server to continue syncing.",
                server.server_version
            )
        } else {
            write!(
                f,
                "This server does not support version checks. Update it to continue syncing."
            )
        }
    }
}

impl std::error::Error for ProtocolMismatchError {}

/// `GET /version` — retrieves the server's protocol information.
pub async fn fetch_server_info(instance: &str) -> Result<ServerInfo> {
    let client = reqwest::Client::new();

    let response = client
        .get(format!("{instance}/version"))
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Server rejected the version request")?;

    response
        .json()
        .await
        .context("Failed to parse server version response")
}

/// Fetches the server's protocol information and rejects incompatible servers.
/// A server without a `/version` endpoint predates protocol checks and is rejected too.
pub async fn ensure_server_compatible(instance: &str) -> Result<ServerInfo> {
    let info = match fetch_server_info(instance).await {
        Ok(info) => info,
        Err(e) if is_not_found(&e) => {
            return Err(ProtocolMismatchError(ProtocolMismatch::new(
                Some(shared::PROTOCOL_VERSION),
                None,
            ))
            .into());
        }
        Err(e) => return Err(e),
    };

    match info.check_client(shared::PROTOCOL_VERSION) {
        ProtocolCompatibility::Compatible => Ok(info),
        _ => Err(ProtocolMismatchError(ProtocolMismatch::new(
            Some(shared::PROTOCOL_VERSION),
            Some(info),
        ))
        .into()),
    }
}

/// Converts a `426 Upgrade Required` response into a `ProtocolMismatchError`.
/// Non-426 responses are returned unchanged.
pub async fn reject_protocol_mismatch(response: reqwest::Response) -> Result<reqwest::Response> {
    if response.status() != reqwest::StatusCode::UPGRADE_REQUIRED {
        return Ok(response);
    }

    let mismatch = response
        .json::<ProtocolMismatch>()
        .await
        .unwrap_or_else(|_| ProtocolMismatch::new(Some(shared::PROTOCOL_VERSION), None));

    Err(ProtocolMismatchError(mismatch).into())
}

fn is_not_found(e: &anyhow::Error) -> bool {
    e.downcast_ref::<reqwest::Error>()
        .and_then(reqwest::Error::status)
        == Some(reqwest::StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server_info(server_version: &str) -> ServerInfo {
        ServerInfo {
            server_version: server_version.to_string(),
            protocol_version: 1,
            min_client_protocol_version: 1,
        }
    }

    fn mismatch(client_outdated: bool, server: Option<ServerInfo>) -> ProtocolMismatch {
        ProtocolMismatch {
            code: "protocol_mismatch".to_string(),
            client_outdated,
            client_protocol_version: Some(shared::PROTOCOL_VERSION),
            server,
        }
    }

    #[test]
    fn display_client_outdated() {
        let error = ProtocolMismatchError(mismatch(true, Some(server_info("0.4.0"))));
        assert_eq!(
            error.to_string(),
            "This app is too old for this server. Update Nooto to continue syncing."
        );
    }

    #[test]
    fn display_server_outdated() {
        let error = ProtocolMismatchError(mismatch(false, Some(server_info("0.3.0"))));
        assert_eq!(
            error.to_string(),
            "This server (v0.3.0) is too old for this version of Nooto. Update the server to continue syncing."
        );
    }

    #[test]
    fn display_missing_server_info() {
        let error = ProtocolMismatchError(mismatch(false, None));
        assert_eq!(
            error.to_string(),
            "This server does not support version checks. Update it to continue syncing."
        );
    }
}
