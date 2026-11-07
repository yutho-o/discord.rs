//! `CreateMessage`, the body of a new message or an interaction reply.

use crate::attachment::{AttachmentRef, validate_files};
use crate::{ActionRowBuilder, AllowedMentions, CreateAttachment, EmbedBuilder, check_len, nested};
use dr_core::{BodyError, FileUpload, IntoBody, RequestBody, ValidationError};
use dr_model::{MessageFlags, MessageId, MessageReference, Nonce};
use serde::Serialize;

/// The body of a new message. `&str`, `String` and `EmbedBuilder` convert into it, so that
/// `msg.reply("hi")` works. `Serialize` writes the wire form, which interaction replies use as the
/// `data` of an `InteractionResponse`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CreateMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    embeds: Vec<EmbedBuilder>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    components: Vec<ActionRowBuilder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<MessageFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_mentions: Option<AllowedMentions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_reference: Option<MessageReference>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    tts: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<Nonce>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    enforce_nonce: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attachments: Vec<AttachmentRef>,
    #[serde(skip)]
    files: Vec<CreateAttachment>,
    #[serde(skip)]
    skip_validation: bool,
}

impl CreateMessage {
    /// An empty message. Sending it fails validation unless something is added.
    pub fn new() -> Self {
        Self::default()
    }
    /// Sets the text.
    pub fn content(mut self, s: impl Into<String>) -> Self {
        self.content = Some(s.into());
        self
    }
    /// Adds an embed.
    pub fn embed(mut self, e: EmbedBuilder) -> Self {
        self.embeds.push(e);
        self
    }
    /// Adds an action row.
    pub fn row(mut self, r: ActionRowBuilder) -> Self {
        self.components.push(r);
        self
    }
    /// Uploads a file with the message, at most 10.
    pub fn attach(mut self, file: CreateAttachment) -> Self {
        self.attachments.push(file.reference(self.files.len()));
        self.files.push(file);
        self
    }
    /// The files to upload, in the order of `files[i]`. Code that wraps this body in another one,
    /// such as an interaction callback, sends these with it.
    pub fn uploads(&self) -> Vec<FileUpload> {
        self.files.iter().map(CreateAttachment::upload).collect()
    }
    /// Sets who may be pinged.
    pub fn allowed_mentions(mut self, m: AllowedMentions) -> Self {
        self.allowed_mentions = Some(m);
        self
    }
    /// Pings nobody.
    pub fn no_mentions(self) -> Self {
        self.allowed_mentions(AllowedMentions::none())
    }
    /// Makes this a reply to `message`, using [`crate::reply_reference`].
    pub fn reply_to(self, message: MessageId) -> Self {
        self.reference(crate::reply_reference(message))
    }
    /// Sets the message reference directly.
    pub fn reference(mut self, r: MessageReference) -> Self {
        self.message_reference = Some(r);
        self
    }
    /// Adds flags to those already set.
    pub fn flags(mut self, f: MessageFlags) -> Self {
        self.flags = Some(self.flags.unwrap_or(MessageFlags::empty()) | f);
        self
    }
    /// Sets the ephemeral flag. Meaningful on interaction replies only.
    pub fn ephemeral(self) -> Self {
        self.flags(MessageFlags::EPHEMERAL)
    }
    /// Sets text to speech.
    pub fn tts(mut self, on: bool) -> Self {
        self.tts = on;
        self
    }
    /// Sets a nonce, with `enforce_nonce` so that Discord deduplicates a retried send. This is what
    /// makes `RetryClass::Protected` safe.
    pub fn nonce(mut self, n: Nonce) -> Self {
        self.nonce = Some(n);
        self.enforce_nonce = true;
        self
    }
    /// True when a nonce is set. The facade generates one when none is.
    pub fn has_nonce(&self) -> bool {
        self.nonce.is_some()
    }
    /// Turns off validation for this request only.
    pub fn skip_validation(mut self) -> Self {
        self.skip_validation = true;
        self
    }
    /// Checks the limits of `[types 4.3]`: at least one of content, embeds, components or files, content
    /// at most 2000 code points, at most 10 embeds, at most 5 rows, at most 10 files, and each embed, row and file itself.
    /// Interaction replies call this directly, since they wrap the body themselves. It does nothing
    /// after [`Self::skip_validation`].
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.skip_validation {
            return Ok(());
        }
        let content = self.content.as_deref().unwrap_or_default();
        if content.is_empty() && self.embeds.is_empty() && self.components.is_empty() && self.files.is_empty() {
            return Err(ValidationError::new("", "a message needs content, an embed, a component or a file"));
        }
        validate_files(&self.files)?;
        check_len("content", content, 0, 2000)?;
        if self.embeds.len() > 10 {
            return Err(ValidationError::new("embeds", "must hold at most 10 embeds"));
        }
        if self.components.len() > 5 {
            return Err(ValidationError::new("components", "must hold at most 5 rows"));
        }
        let (embeds, rows) = (&self.embeds, &self.components);
        for (i, e) in embeds.iter().enumerate() {
            e.validate().map_err(|err| nested(format!("embeds[{i}]"), err))?;
        }
        for (i, r) in rows.iter().enumerate() {
            r.validate().map_err(|err| nested(format!("components[{i}]"), err))?;
        }
        if let Some(Nonce::Str(n)) = &self.nonce {
            check_len("nonce", n, 0, 25)?;
        }
        Ok(())
    }
}

impl From<&str> for CreateMessage {
    fn from(s: &str) -> Self {
        Self::new().content(s)
    }
}
impl From<String> for CreateMessage {
    fn from(s: String) -> Self {
        Self::new().content(s)
    }
}
impl From<&String> for CreateMessage {
    fn from(s: &String) -> Self {
        Self::new().content(s)
    }
}
impl From<CreateAttachment> for CreateMessage {
    fn from(file: CreateAttachment) -> Self {
        Self::new().attach(file)
    }
}
impl From<EmbedBuilder> for CreateMessage {
    fn from(e: EmbedBuilder) -> Self {
        Self::new().embed(e)
    }
}

impl IntoBody for CreateMessage {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        Ok(RequestBody::json(&self)?.with_files(self.uploads()))
    }
}
