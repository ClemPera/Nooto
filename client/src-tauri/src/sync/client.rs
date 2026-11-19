use anyhow::Result;
use reqwest::{Client, Method, RequestBuilder};
use std::sync::OnceLock;
use std::time::Duration;

/// Timeout for establishing a connection to the server.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Total timeout for a sync request, including reading the response body.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// Shared HTTP client for all sync requests.
fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| build_http_client(CONNECT_TIMEOUT, REQUEST_TIMEOUT))
}

fn build_http_client(connect_timeout: Duration, request_timeout: Duration) -> Client {
    Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()
        .expect("Failed to build HTTP client")
}

/// Joins a server base URL with an endpoint path, avoiding duplicate slashes.
fn endpoint(instance: &str, path: &str) -> String {
    format!(
        "{}/{}",
        instance.trim().trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// Trims whitespace and trailing slashes from a user-supplied server URL.
pub fn normalize_instance(instance: &str) -> Result<String> {
    let normalized = instance.trim().trim_end_matches('/');
    anyhow::ensure!(!normalized.is_empty(), "Instance url is empty");
    Ok(normalized.to_owned())
}

/// Per-instance HTTP client: normalizes the base URL and applies the shared
/// timeout client and protocol header to every request.
pub struct ApiClient {
    base: String,
    http: Client,
}

impl ApiClient {
    pub fn new(instance: &str) -> Result<Self> {
        Ok(Self {
            base: normalize_instance(instance)?,
            http: http_client().clone(),
        })
    }

    pub fn get(&self, path: &str) -> RequestBuilder {
        self.request(Method::GET, path)
    }

    pub fn post(&self, path: &str) -> RequestBuilder {
        self.request(Method::POST, path)
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, endpoint(&self.base, path))
            .header(
                shared::PROTOCOL_VERSION_HEADER,
                shared::PROTOCOL_VERSION.to_string(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn client_times_out_on_stalled_response() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
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

    #[test]
    fn endpoint_trims_whitespace() {
        assert_eq!(
            endpoint("  https://example.com/ ", "/notes"),
            "https://example.com/notes"
        );
    }

    #[test]
    fn normalize_instance_trims_whitespace_and_trailing_slashes() {
        assert_eq!(
            normalize_instance("  https://example.com/  ").unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn normalize_instance_keeps_base_path() {
        assert_eq!(
            normalize_instance("https://example.com/nooto///").unwrap(),
            "https://example.com/nooto"
        );
    }

    #[test]
    fn normalize_instance_rejects_empty() {
        assert!(normalize_instance("   /  ").is_err());
    }

    #[test]
    fn joins_trailing_slash_instance() {
        let api = ApiClient::new("https://example.com/").unwrap();
        let request = api.get("/notes").build().unwrap();

        assert_eq!(request.url().as_str(), "https://example.com/notes");
    }

    #[test]
    fn preserves_base_path() {
        let api = ApiClient::new("https://example.com/nooto/").unwrap();
        let request = api.post("/login").build().unwrap();

        assert_eq!(request.url().as_str(), "https://example.com/nooto/login");
    }

    #[test]
    fn trims_whitespace() {
        let api = ApiClient::new("  https://example.com/ ").unwrap();
        let request = api.get("/version").build().unwrap();

        assert_eq!(request.url().as_str(), "https://example.com/version");
    }

    #[test]
    fn rejects_empty_instance() {
        assert!(ApiClient::new("   /  ").is_err());
    }

    #[test]
    fn adds_protocol_header() {
        let api = ApiClient::new("https://example.com").unwrap();
        let request = api.get("/notes").build().unwrap();

        assert_eq!(
            request.headers()[shared::PROTOCOL_VERSION_HEADER]
                .to_str()
                .unwrap(),
            shared::PROTOCOL_VERSION.to_string()
        );
    }
}
