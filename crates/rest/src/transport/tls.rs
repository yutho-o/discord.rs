//! TLS settings. Traps from `ARCHITECTURE.md` 5.3:
//!
//! - Use an explicit ring provider. Never call `install_default`: a process-wide provider collides
//!   with whatever else the bot links.
//! - Hand reqwest the config through `tls_backend_preconfigured`. It checks the type at runtime, and
//!   a config of the wrong type only fails at `build()`, so `tests/tls.rs` builds a client.
//! - The WebSocket connector must not advertise `h2`, see the gateway's `connector.rs`.

use rustls::ClientConfig;
use std::sync::Arc;

/// The ring-backed config for HTTP, with `h2` and `http/1.1` in ALPN and the webpki roots.
pub fn tls_http() -> Result<Arc<ClientConfig>, rustls::Error> {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}
