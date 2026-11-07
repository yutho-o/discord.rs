//! `CreateWebhook`, `EditWebhook` and `ExecuteWebhook`, and `ImageData` for avatars.

use crate::{CreateMessage, check_len, nested};
use dr_core::{BodyError, IntoBody, Patch, RequestBody, ValidationError};
use dr_model::{ChannelId, GenericId};
use serde::{Serialize, Serializer};

/// An image sent inline, as a `data:` URI: avatars, icons and banners.
#[derive(Clone, PartialEq, Eq)]
pub struct ImageData(String);

impl ImageData {
    /// A PNG image.
    pub fn png(bytes: &[u8]) -> Self {
        Self::new("image/png", bytes)
    }

    /// A JPEG image.
    pub fn jpeg(bytes: &[u8]) -> Self {
        Self::new("image/jpeg", bytes)
    }

    /// A GIF image.
    pub fn gif(bytes: &[u8]) -> Self {
        Self::new("image/gif", bytes)
    }

    /// An image of this media type.
    pub fn new(media_type: &str, bytes: &[u8]) -> Self {
        Self(format!("data:{media_type};base64,{}", base64(bytes)))
    }

    /// The `data:` URI.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Shows the media type and the size, not the content.
impl std::fmt::Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let head = self.0.split(';').next().unwrap_or_default();
        write!(f, "ImageData({head}, {} chars)", self.0.len())
    }
}

impl Serialize for ImageData {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

/// Standard base64 with padding, RFC 4648 section 4.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Names Discord refuses for a webhook.
fn check_webhook_name(name: &str) -> Result<(), ValidationError> {
    check_len("name", name, 1, 80)?;
    let lower = name.to_lowercase();
    if lower.contains("clyde") || lower.contains("discord") {
        return Err(ValidationError::new("name", "cannot contain \"clyde\" or \"discord\""));
    }
    Ok(())
}

/// The body of a new webhook.
#[derive(Clone, Debug, Serialize)]
pub struct CreateWebhook {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar: Option<ImageData>,
}

impl CreateWebhook {
    /// A webhook named `name`, 1 to 80 characters, without "clyde" or "discord".
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), avatar: None }
    }

    /// Sets its default avatar.
    pub fn avatar(mut self, image: ImageData) -> Self {
        self.avatar = Some(image);
        self
    }

    /// Checks the name.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_webhook_name(&self.name)
    }
}

impl IntoBody for CreateWebhook {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

/// The body of a webhook edit. A field that was not set keeps its value. Moving the webhook to
/// another channel needs the bot's token, not the webhook's.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditWebhook {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Patch::is_absent")]
    avatar: Patch<ImageData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<ChannelId>,
}

impl EditWebhook {
    /// An edit that changes nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Renames it.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets its default avatar.
    pub fn avatar(mut self, image: ImageData) -> Self {
        self.avatar = Patch::Value(image);
        self
    }

    /// Goes back to the default avatar: writes `"avatar": null`.
    pub fn clear_avatar(mut self) -> Self {
        self.avatar = Patch::Null;
        self
    }

    /// Moves it to another channel of the same guild.
    pub fn channel(mut self, channel: ChannelId) -> Self {
        self.channel_id = Some(channel);
        self
    }

    /// True when it moves the webhook, which needs the bot's token.
    pub fn moves_channel(&self) -> bool {
        self.channel_id.is_some()
    }

    /// Checks the name.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.name.as_deref().map_or(Ok(()), check_webhook_name)
    }
}

impl IntoBody for EditWebhook {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

/// A message sent through a webhook: a [`CreateMessage`] plus the name and avatar to show, and, in a
/// forum channel, the post it starts. Files of the message are uploaded with it.
#[derive(Clone, Debug, Serialize)]
pub struct ExecuteWebhook {
    #[serde(flatten)]
    message: CreateMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    applied_tags: Vec<GenericId>,
}

impl ExecuteWebhook {
    /// Sends `message` with the webhook's name and avatar.
    pub fn new(message: impl Into<CreateMessage>) -> Self {
        Self { message: message.into(), username: None, avatar_url: None, thread_name: None, applied_tags: Vec::new() }
    }

    /// Shows this name instead of the webhook's, 1 to 80 characters.
    pub fn username(mut self, name: impl Into<String>) -> Self {
        self.username = Some(name.into());
        self
    }

    /// Shows the image at this URL as the avatar.
    pub fn avatar_url(mut self, url: impl Into<String>) -> Self {
        self.avatar_url = Some(url.into());
        self
    }

    /// In a forum channel, starts a post with this name.
    pub fn thread_name(mut self, name: impl Into<String>) -> Self {
        self.thread_name = Some(name.into());
        self
    }

    /// In a forum channel, tags the new post, at most 5.
    pub fn tag(mut self, tag: GenericId) -> Self {
        self.applied_tags.push(tag);
        self
    }

    /// Checks the message and the name.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.message.validate()?;
        if let Some(name) = &self.username {
            check_webhook_name(name).map_err(|e| nested(String::from("username"), e))?;
        }
        if let Some(name) = &self.thread_name {
            check_len("thread_name", name, 1, 100)?;
        }
        if self.applied_tags.len() > 5 {
            return Err(ValidationError::new("applied_tags", "must hold at most 5 tags"));
        }
        Ok(())
    }
}

impl From<&str> for ExecuteWebhook {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}
impl From<String> for ExecuteWebhook {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}
impl From<CreateMessage> for ExecuteWebhook {
    fn from(m: CreateMessage) -> Self {
        Self::new(m)
    }
}

impl IntoBody for ExecuteWebhook {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        let files = self.message.uploads();
        Ok(RequestBody::json(&self)?.with_files(files))
    }
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn base64_matches_the_rfc_vectors() {
        let vectors = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in vectors {
            assert_eq!(base64(input.as_bytes()), expected);
        }
        assert_eq!(base64(&[0xfb, 0xff]), "+/8=");
    }
}
