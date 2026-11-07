//! Where to connect. `MockServer::endpoints()` returns one, so nothing here depends on the mock.

/// Base addresses of the Discord services a client talks to.
///
/// Marked `non_exhaustive`, since the later full port adds the CDN and voice. Start from
/// [`Endpoints::default`] or [`Endpoints::custom`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Endpoints {
    /// REST base with no version. Default `https://discord.com/api`; `dr_rest` appends `/v10`.
    pub api: String,
    /// Gateway URL to use instead of the one from `GET /gateway/bot`. `None` by default.
    pub gateway_override: Option<String>,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self { api: "https://discord.com/api".to_owned(), gateway_override: None }
    }
}

impl Endpoints {
    /// Endpoints at custom addresses, such as a local mock server.
    pub fn custom(api: impl Into<String>, gateway_override: Option<String>) -> Self {
        Self { api: api.into(), gateway_override }
    }
}
