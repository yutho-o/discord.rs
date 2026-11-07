//! Opening the WebSocket.
//!
//! Trap from `ARCHITECTURE.md` 5.3: the WebSocket connector needs **no ALPN**. Advertising `h2` makes
//! the upgrade fail. Use the ring provider explicitly, and never call `install_default`.

use crate::GatewayError;
use crate::compress::Compression;
use std::sync::Arc;

/// The connector, with no ALPN.
pub fn ws_connector() -> Result<tokio_tungstenite::Connector, GatewayError> {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| GatewayError::Connect(e.to_string()))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(tokio_tungstenite::Connector::Rustls(Arc::new(config)))
}

/// The URL to dial: `{base}/?v=10&encoding=json[&compress=zlib-stream]`. The query goes on a resume
/// URL too. Any trailing `/` or query already on `base` is replaced.
pub fn connect_url(base: &str, compression: Compression) -> String {
    let base = base.split('?').next().unwrap_or(base).trim_end_matches('/');
    match compression.query_value() {
        Some(c) => format!("{base}/?v=10&encoding=json&compress={c}"),
        None => format!("{base}/?v=10&encoding=json"),
    }
}
