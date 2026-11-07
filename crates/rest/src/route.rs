//! Route generalization for rate limit keys, `ARCHITECTURE.md` 10.2.
//!
//! Hand-parsed on purpose: `\d` in the upstream regex must stay ASCII, and `regex` is not a dependency.

use crate::request::Method;
use std::time::{SystemTime, UNIX_EPOCH};

const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;
const FOURTEEN_DAYS_MS: u64 = 14 * 24 * 60 * 60 * 1000;

/// What the limiter needs from a request's path.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct RouteInfo {
    /// The top-level resource id a bucket is scoped to: a channel, guild or webhook id, the webhook
    /// id and token, `burst` for an interaction callback, or `global` when there is none.
    pub major: String,
    /// The key that maps to a bucket hash: `METHOD:route`, with the route's ids replaced.
    pub bucket_route: String,
}

impl RouteInfo {
    /// A route with the given key parts.
    pub fn new(major: impl Into<String>, bucket_route: impl Into<String>) -> Self {
        Self { major: major.into(), bucket_route: bucket_route.into() }
    }

    /// True for the burst lane, used only by interaction callbacks.
    pub fn is_burst(&self) -> bool {
        self.major == "burst"
    }
}

fn is_snowflake(segment: &str) -> bool {
    (17..=19).contains(&segment.len()) && segment.bytes().all(|b| b.is_ascii_digit())
}

fn major_of(segments: &[&str]) -> String {
    let (Some(&first), Some(&id)) = (segments.first(), segments.get(1)) else {
        return String::from("global");
    };
    if !is_snowflake(id) {
        return String::from("global");
    }
    match first {
        "webhooks" => match segments.get(2) {
            Some(token) if !token.is_empty() => format!("{id}/{token}"),
            _ => id.to_owned(),
        },
        "channels" | "guilds" => id.to_owned(),
        _ => String::from("global"),
    }
}

/// True when a snowflake is older than 14 days by the wall clock. An unparsable id counts as recent.
fn older_than_14_days(snowflake: &str, now_ms: u64) -> bool {
    let Ok(id) = snowflake.parse::<u64>() else { return false };
    let created = (id >> 22) + DISCORD_EPOCH_MS;
    now_ms.saturating_sub(created) > FOURTEEN_DAYS_MS
}

/// Generalizes a path, without its query. The vectors live in `tests/route_vectors.rs`.
pub fn route_info(method: Method, path: &str) -> RouteInfo {
    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(0));
    route_info_at(method, path, now_ms)
}

fn route_info_at(method: Method, path: &str, now_ms: u64) -> RouteInfo {
    let verb = method.as_str();
    if path.starts_with("/interactions/") && path.ends_with("/callback") {
        return RouteInfo::new("burst", format!("{verb}:/interactions/:id/:token/callback"));
    }
    let segments: Vec<&str> = path.strip_prefix('/').unwrap_or(path).split('/').collect();
    let major = major_of(&segments);

    let mut out: Vec<&str> = Vec::with_capacity(segments.len());
    for (i, seg) in segments.iter().enumerate() {
        if *seg == "reactions" {
            // The first `/reactions/(.*)` swallows everything after it.
            out.push(seg);
            if i + 1 < segments.len() {
                out.push(":reaction");
            }
            break;
        }
        if i == 2 && segments.first() == Some(&"webhooks") && is_snowflake(segments[1]) {
            out.push(":token");
        } else if is_snowflake(seg) {
            out.push(":id");
        } else {
            out.push(seg);
        }
    }
    let mut route = format!("/{}", out.join("/"));
    if method == Method::Delete
        && route == "/channels/:id/messages/:id"
        && let Some(last) = segments.last()
        && older_than_14_days(last, now_ms)
    {
        route.push_str("/Delete Old Message");
    }
    RouteInfo::new(major, format!("{verb}:{route}"))
}
