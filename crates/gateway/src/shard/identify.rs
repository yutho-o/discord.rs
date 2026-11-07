//! Building identify and resume frames.

use super::ShardOptions;
use dr_core::Secret;
use dr_model::{GatewayFrame, Identify, IdentifyProperties, Resume};

/// Client capability sent at identify, `[gateway 11.4]`.
const CAPABILITIES: u64 = 1 << 15;

/// Strips `Bot `, in any case and with any space, as `/^bot\s*/i` does upstream.
pub(crate) fn bare_token(token: &Secret) -> Secret {
    let raw = token.expose();
    let bare = match raw.get(..3) {
        Some(prefix) if prefix.eq_ignore_ascii_case("bot") => {
            let tail = raw.get(3..).unwrap_or_default();
            // A token that merely starts with "bot" has no space after it, and is left alone.
            if tail.starts_with(char::is_whitespace) { tail.trim_start() } else { raw }
        }
        _ => raw,
    };
    Secret::new(bare)
}

pub(crate) fn identify_json(options: &ShardOptions) -> Result<String, serde_json::Error> {
    let lib = concat!("discord.rs/", env!("CARGO_PKG_VERSION"));
    serde_json::to_string(&GatewayFrame::identify(Identify {
        token: bare_token(&options.token),
        properties: IdentifyProperties {
            os: std::env::consts::OS.to_owned(),
            browser: lib.to_owned(),
            device: lib.to_owned(),
        },
        compress: false,
        large_threshold: options.large_threshold,
        shard: [options.shard_id, options.shard_count],
        presence: options.presence.clone(),
        intents: options.intents,
        capabilities: CAPABILITIES,
    }))
}

pub(crate) fn resume_json(options: &ShardOptions, session_id: &Secret, seq: u64) -> Result<String, serde_json::Error> {
    serde_json::to_string(&GatewayFrame::resume(Resume {
        token: bare_token(&options.token),
        session_id: session_id.clone(),
        seq,
    }))
}
