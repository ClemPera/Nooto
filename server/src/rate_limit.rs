//! Rate-limit key extraction that only trusts forwarding headers from known proxies.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use anyhow::Context;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request};
use forwarded_header_value::ForwardedHeaderValue;
use ipnet::IpNet;
use tower_governor::errors::GovernorError;
use tower_governor::key_extractor::KeyExtractor;

/// Env var overriding the default trusted proxy list. `none` disables trust entirely.
pub const TRUSTED_PROXIES_ENV: &str = "NOOTO_TRUSTED_PROXIES";

/// Trusted when the env var is unset: loopback, RFC1918, link-local, and IPv6 ULA.
const DEFAULT_TRUSTED_PROXIES: &[&str] = &[
    "127.0.0.0/8",
    "::1/128",
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "169.254.0.0/16",
    "fc00::/7",
    "fe80::/10",
];

/// Keys requests by real client IP. Forwarding headers are ignored unless the TCP peer is trusted.
#[derive(Clone, Debug)]
pub struct TrustedProxyIpKeyExtractor {
    trusted: Arc<[IpNet]>,
}

impl TrustedProxyIpKeyExtractor {
    pub fn new(trusted: Vec<IpNet>) -> Self {
        Self {
            trusted: trusted.into(),
        }
    }

    /// Loads the trusted list from the environment, falling back to the local defaults.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self::new(trusted_proxies_from_env()?))
    }

    fn is_trusted(&self, ip: IpAddr) -> bool {
        let ip = normalize_ip(ip);
        self.trusted.iter().any(|net| net.contains(&ip))
    }
}

impl KeyExtractor for TrustedProxyIpKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        let peer = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .ok_or(GovernorError::UnableToExtractKey)?
            .0
            .ip();
        let peer = normalize_ip(peer);

        // A client that is not itself a proxy cannot choose its bucket.
        if !self.is_trusted(peer) {
            return Ok(peer);
        }

        // Walk from the nearest hop outwards; the first untrusted address is the client.
        Ok(forwarded_chain(req.headers())
            .into_iter()
            .rev()
            .find(|ip| !self.is_trusted(*ip))
            .unwrap_or(peer))
    }
}

/// Parses the trusted proxy list. Empty input yields an empty list.
pub fn parse_trusted_proxies(raw: &str) -> anyhow::Result<Vec<IpNet>> {
    let mut trusted = Vec::new();

    for entry in raw.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        trusted.push(parse_net(entry)?);
    }

    Ok(trusted)
}

/// Reads `NOOTO_TRUSTED_PROXIES`: unset or empty means the local defaults, `none` means no trust.
pub fn trusted_proxies_from_env() -> anyhow::Result<Vec<IpNet>> {
    let raw = match std::env::var(TRUSTED_PROXIES_ENV) {
        Ok(raw) => raw,
        Err(std::env::VarError::NotPresent) => {
            return default_trusted_proxies();
        }
        Err(err) => {
            return Err(anyhow::Error::new(err).context(format!("Invalid {TRUSTED_PROXIES_ENV}")));
        }
    };

    let raw = raw.trim();
    if raw.is_empty() {
        return default_trusted_proxies();
    }
    if raw.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }

    parse_trusted_proxies(raw)
}

fn default_trusted_proxies() -> anyhow::Result<Vec<IpNet>> {
    parse_trusted_proxies(&DEFAULT_TRUSTED_PROXIES.join(","))
}

fn parse_net(entry: &str) -> anyhow::Result<IpNet> {
    if let Ok(ip) = entry.parse::<IpAddr>() {
        return Ok(IpNet::from(ip));
    }

    entry
        .parse::<IpNet>()
        .with_context(|| format!("Invalid IP or CIDR in NOOTO_TRUSTED_PROXIES: {entry}"))
}

/// Collected forwarded addresses, left to right, from the first header that carries any.
fn forwarded_chain(headers: &HeaderMap) -> Vec<IpAddr> {
    let mut chain = Vec::new();

    for value in headers.get_all("x-forwarded-for") {
        let Ok(value) = value.to_str() else { continue };
        for entry in value.split(',') {
            if let Some(ip) = parse_ip(entry) {
                chain.push(ip);
            }
        }
    }

    if !chain.is_empty() {
        return chain;
    }

    if let Some(ip) = headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_ip)
    {
        chain.push(ip);
        return chain;
    }

    for value in headers.get_all("forwarded") {
        let Ok(value) = value.to_str() else { continue };
        let Ok(stanzas) = ForwardedHeaderValue::from_forwarded(value) else {
            continue;
        };

        for stanza in stanzas.iter() {
            if let Some(ip) = stanza.forwarded_for_ip() {
                chain.push(normalize_ip(ip));
            }
        }
    }

    chain
}

fn parse_ip(value: &str) -> Option<IpAddr> {
    value.trim().parse::<IpAddr>().ok().map(normalize_ip)
}

/// Maps IPv4-mapped IPv6 addresses to IPv4 so CIDR checks match the expected family.
fn normalize_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
        ip => ip,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::ConnectInfo;
    use axum::http::HeaderValue;

    fn extractor(trusted: &str) -> TrustedProxyIpKeyExtractor {
        TrustedProxyIpKeyExtractor::new(parse_trusted_proxies(trusted).unwrap())
    }

    fn ip(value: &str) -> IpAddr {
        value.parse().unwrap()
    }

    fn request(peer: &str, headers: &[(&str, &str)]) -> Request<()> {
        let mut builder = Request::builder();
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }

        let mut request = builder.body(()).unwrap();
        let peer: SocketAddr = peer.parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(peer));
        request
    }

    // --- extractor ---

    #[test]
    fn untrusted_peer_cannot_spoof_x_forwarded_for() {
        let extractor = extractor("10.0.0.0/8");

        for spoofed in ["203.0.113.1", "203.0.113.2", "203.0.113.3"] {
            let request = request("198.51.100.7:1234", &[("x-forwarded-for", spoofed)]);
            assert_eq!(extractor.extract(&request).unwrap(), ip("198.51.100.7"));
        }
    }

    #[test]
    fn untrusted_peer_ignores_x_real_ip_and_forwarded() {
        let extractor = extractor("10.0.0.0/8");
        let request = request(
            "198.51.100.7:1234",
            &[
                ("x-real-ip", "203.0.113.1"),
                ("forwarded", "for=203.0.113.2"),
            ],
        );

        assert_eq!(extractor.extract(&request).unwrap(), ip("198.51.100.7"));
    }

    #[test]
    fn missing_connect_info_is_an_extraction_error() {
        let extractor = extractor("10.0.0.0/8");
        let request = Request::builder().body(()).unwrap();

        assert!(matches!(
            extractor.extract(&request),
            Err(GovernorError::UnableToExtractKey)
        ));
    }

    #[test]
    fn trusted_peer_uses_the_forwarded_client() {
        let extractor = extractor("127.0.0.0/8");
        let request = request("127.0.0.1:1234", &[("x-forwarded-for", "203.0.113.5")]);

        assert_eq!(extractor.extract(&request).unwrap(), ip("203.0.113.5"));
    }

    #[test]
    fn trusted_peer_walks_past_inner_proxies() {
        let extractor = extractor("127.0.0.0/8, 10.0.0.0/8");
        let request = request(
            "127.0.0.1:1234",
            &[("x-forwarded-for", "198.51.100.7, 10.0.0.2, 127.0.0.1")],
        );

        assert_eq!(extractor.extract(&request).unwrap(), ip("198.51.100.7"));
    }

    #[test]
    fn trusted_peer_without_headers_uses_the_peer() {
        let extractor = extractor("127.0.0.0/8");
        let request = request("127.0.0.1:1234", &[]);

        assert_eq!(extractor.extract(&request).unwrap(), ip("127.0.0.1"));
    }

    #[test]
    fn trusted_peer_with_only_trusted_hops_uses_the_peer() {
        let extractor = extractor("127.0.0.0/8, 10.0.0.0/8");
        let request = request(
            "127.0.0.1:1234",
            &[("x-forwarded-for", "10.0.0.2, 127.0.0.1")],
        );

        assert_eq!(extractor.extract(&request).unwrap(), ip("127.0.0.1"));
    }

    #[test]
    fn x_forwarded_for_wins_over_other_headers() {
        let extractor = extractor("127.0.0.0/8");
        let request = request(
            "127.0.0.1:1234",
            &[
                ("x-forwarded-for", "203.0.113.5"),
                ("x-real-ip", "203.0.113.6"),
                ("forwarded", "for=203.0.113.7"),
            ],
        );

        assert_eq!(extractor.extract(&request).unwrap(), ip("203.0.113.5"));
    }

    #[test]
    fn x_real_ip_is_used_when_x_forwarded_for_is_absent() {
        let extractor = extractor("127.0.0.0/8");
        let request = request(
            "127.0.0.1:1234",
            &[
                ("x-real-ip", "203.0.113.6"),
                ("forwarded", "for=203.0.113.7"),
            ],
        );

        assert_eq!(extractor.extract(&request).unwrap(), ip("203.0.113.6"));
    }

    #[test]
    fn forwarded_header_is_used_as_a_last_resort() {
        let extractor = extractor("127.0.0.0/8");
        let request = request("127.0.0.1:1234", &[("forwarded", "for=203.0.113.7")]);

        assert_eq!(extractor.extract(&request).unwrap(), ip("203.0.113.7"));
    }

    #[test]
    fn malformed_entries_are_skipped() {
        let extractor = extractor("127.0.0.0/8");
        let request = request(
            "127.0.0.1:1234",
            &[("x-forwarded-for", "unknown, 203.0.113.5")],
        );

        assert_eq!(extractor.extract(&request).unwrap(), ip("203.0.113.5"));
    }

    #[test]
    fn ipv4_mapped_ipv6_peer_is_treated_as_ipv4() {
        let extractor = extractor("127.0.0.0/8");
        let request = request(
            "[::ffff:127.0.0.1]:1234",
            &[("x-forwarded-for", "203.0.113.5")],
        );

        assert_eq!(extractor.extract(&request).unwrap(), ip("203.0.113.5"));
    }

    #[test]
    fn ipv6_clients_are_supported() {
        let extractor = extractor("::1/128");
        let request = request("[::1]:1234", &[("x-forwarded-for", "2001:db8::1")]);

        assert_eq!(extractor.extract(&request).unwrap(), ip("2001:db8::1"));
    }

    #[test]
    fn multiple_header_values_are_concatenated() {
        let extractor = extractor("127.0.0.0/8");
        let mut request = request("127.0.0.1:1234", &[]);
        request
            .headers_mut()
            .append("x-forwarded-for", HeaderValue::from_static("198.51.100.7"));
        request
            .headers_mut()
            .append("x-forwarded-for", HeaderValue::from_static("127.0.0.1"));

        assert_eq!(extractor.extract(&request).unwrap(), ip("198.51.100.7"));
    }

    // --- parsing ---

    #[test]
    fn parse_accepts_ips_and_cidrs() {
        let nets = parse_trusted_proxies("10.0.0.0/8, 192.168.1.5, fd00::/8").unwrap();

        assert_eq!(nets.len(), 3);
        assert_eq!(nets[0].prefix_len(), 8);
        assert_eq!(nets[1].prefix_len(), 32);
        assert_eq!(nets[2].prefix_len(), 8);
    }

    #[test]
    fn parse_ignores_empty_entries() {
        assert!(parse_trusted_proxies(" , ").unwrap().is_empty());
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse_trusted_proxies("not-an-ip").is_err());
    }

    #[test]
    fn parse_accepts_default_routes() {
        let nets = parse_trusted_proxies("0.0.0.0/0, ::/0").unwrap();

        assert_eq!(nets.len(), 2);
        assert_eq!(nets[0].prefix_len(), 0);
        assert_eq!(nets[1].prefix_len(), 0);
    }

    #[test]
    fn defaults_cover_loopback_private_and_link_local_ranges() {
        let nets = default_trusted_proxies().unwrap();

        assert!(
            nets.iter()
                .any(|net| net.contains(&IpAddr::from([127, 0, 0, 1])))
        );
        assert!(
            nets.iter()
                .any(|net| net.contains(&IpAddr::from([172, 18, 0, 1])))
        );
        assert!(
            nets.iter()
                .any(|net| net.contains(&IpAddr::from([192, 168, 1, 1])))
        );
        assert!(
            nets.iter()
                .any(|net| net.contains(&IpAddr::from([10, 0, 0, 1])))
        );
        assert!(
            !nets
                .iter()
                .any(|net| net.contains(&IpAddr::from([203, 0, 113, 1])))
        );
    }

    // --- governor layer regression ---

    fn governor_app(extractor: TrustedProxyIpKeyExtractor) -> axum::Router {
        use axum::routing::post;
        use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};

        let config = Arc::new(
            GovernorConfigBuilder::default()
                .key_extractor(extractor)
                .per_second(3600)
                .burst_size(1)
                .finish()
                .unwrap(),
        );

        axum::Router::new()
            .route("/login", post(|| async { axum::http::StatusCode::OK }))
            .route_layer(GovernorLayer::new(config))
    }

    async fn request_status(
        app: &axum::Router,
        peer: &str,
        forwarded_for: &str,
    ) -> axum::http::StatusCode {
        use tower::ServiceExt;

        let mut request = Request::builder()
            .method("POST")
            .uri("/login")
            .header("x-forwarded-for", forwarded_for)
            .body(axum::body::Body::empty())
            .unwrap();
        let peer: SocketAddr = peer.parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(peer));

        app.clone().oneshot(request).await.unwrap().status()
    }

    #[tokio::test]
    async fn spoofed_forwarded_for_does_not_create_new_buckets() {
        let app = governor_app(extractor("10.0.0.0/8"));

        let first = request_status(&app, "198.51.100.7:1234", "203.0.113.1").await;
        let second = request_status(&app, "198.51.100.7:1234", "203.0.113.2").await;

        assert_eq!(first, axum::http::StatusCode::OK);
        assert_eq!(second, axum::http::StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn proxied_clients_get_independent_buckets() {
        let app = governor_app(extractor("127.0.0.0/8"));

        let first = request_status(&app, "127.0.0.1:1234", "203.0.113.1").await;
        let other = request_status(&app, "127.0.0.1:1234", "203.0.113.2").await;
        let repeated = request_status(&app, "127.0.0.1:1234", "203.0.113.1").await;

        assert_eq!(first, axum::http::StatusCode::OK);
        assert_eq!(other, axum::http::StatusCode::OK);
        assert_eq!(repeated, axum::http::StatusCode::TOO_MANY_REQUESTS);
    }
}
