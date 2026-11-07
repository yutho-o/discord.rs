//! `EditMessage`. A separate type from `CreateMessage`, so a PATCH writes only what was asked for:
//! the trap of `[types 9.5]`.

use crate::attachment::{AttachmentRef, validate_files};
use crate::{ActionRowBuilder, AllowedMentions, CreateAttachment, EmbedBuilder, check_len, nested};
use dr_core::{BodyError, FileUpload, IntoBody, Patch, RequestBody, ValidationError};
use dr_model::{AttachmentId, MessageFlags};
use serde::Serialize;

/// The body of a message edit. A field that was not set stays out of the JSON and keeps its value
/// on Discord's side. `Serialize` writes the wire form, which `update` on a component interaction
/// uses as the `data` of an `InteractionResponse`.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditMessage {
    #[serde(skip_serializing_if = "Patch::is_absent")]
    content: Patch<String>,
    #[serde(skip_serializing_if = "Patch::is_absent")]
    embeds: Patch<Vec<EmbedBuilder>>,
    #[serde(skip_serializing_if = "Patch::is_absent")]
    components: Patch<Vec<ActionRowBuilder>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<MessageFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_mentions: Option<AllowedMentions>,
    #[serde(skip_serializing_if = "Patch::is_absent")]
    attachments: Patch<Vec<AttachmentRef>>,
    #[serde(skip)]
    files: Vec<CreateAttachment>,
    #[serde(skip)]
    skip_validation: bool,
}

impl EditMessage {
    /// An edit that changes nothing yet.
    pub fn new() -> Self {
        Self::default()
    }
    /// Replaces the text.
    pub fn content(mut self, s: impl Into<String>) -> Self {
        self.content = Patch::Value(s.into());
        self
    }
    /// Removes the text: writes `"content": null`.
    pub fn clear_content(mut self) -> Self {
        self.content = Patch::Null;
        self
    }
    /// Adds an embed to the new embed list. The first call replaces the old list.
    pub fn embed(mut self, e: EmbedBuilder) -> Self {
        match &mut self.embeds {
            Patch::Value(list) => list.push(e),
            other => *other = Patch::Value(vec![e]),
        }
        self
    }
    /// Removes every embed: writes `"embeds": []`.
    pub fn clear_embeds(mut self) -> Self {
        self.embeds = Patch::Value(Vec::new());
        self
    }
    /// Adds an action row to the new component list. The first call replaces the old list.
    pub fn row(mut self, r: ActionRowBuilder) -> Self {
        match &mut self.components {
            Patch::Value(list) => list.push(r),
            other => *other = Patch::Value(vec![r]),
        }
        self
    }
    /// Removes every component: writes `"components": []`.
    pub fn clear_components(mut self) -> Self {
        self.components = Patch::Value(Vec::new());
        self
    }
    /// Uploads a file. The message keeps only the attachments this edit lists, so the old ones go
    /// away unless [`Self::keep_attachment`] names them.
    pub fn attach(mut self, file: CreateAttachment) -> Self {
        let reference = file.reference(self.files.len());
        self.push_attachment(reference);
        self.files.push(file);
        self
    }
    /// Keeps an attachment the message already has. Once an edit lists attachments, the others go.
    pub fn keep_attachment(mut self, id: AttachmentId) -> Self {
        self.push_attachment(AttachmentRef::Existing { id });
        self
    }
    /// Removes every attachment: writes `"attachments": []`.
    pub fn clear_attachments(mut self) -> Self {
        self.attachments = Patch::Value(Vec::new());
        self.files.clear();
        self
    }
    fn push_attachment(&mut self, reference: AttachmentRef) {
        match &mut self.attachments {
            Patch::Value(list) => list.push(reference),
            other => *other = Patch::Value(vec![reference]),
        }
    }
    /// The files to upload, in the order of `files[i]`. Code that wraps this body in another one,
    /// such as an interaction callback, sends these with it.
    pub fn uploads(&self) -> Vec<FileUpload> {
        self.files.iter().map(CreateAttachment::upload).collect()
    }
    /// Sets the flags.
    pub fn flags(mut self, f: MessageFlags) -> Self {
        self.flags = Some(f);
        self
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
    /// Turns off validation for this request only.
    pub fn skip_validation(mut self) -> Self {
        self.skip_validation = true;
        self
    }
    /// Checks the limits of `[types 4.3]` on whatever was set. It does nothing after
    /// [`Self::skip_validation`].
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.skip_validation {
            return Ok(());
        }
        if let Patch::Value(content) = &self.content {
            check_len("content", content, 0, 2000)?;
        }
        let no_embeds = Vec::new();
        let no_rows = Vec::new();
        let embeds = if let Patch::Value(list) = &self.embeds { list } else { &no_embeds };
        let rows = if let Patch::Value(list) = &self.components { list } else { &no_rows };
        if embeds.len() > 10 {
            return Err(ValidationError::new("embeds", "must hold at most 10 embeds"));
        }
        if rows.len() > 5 {
            return Err(ValidationError::new("components", "must hold at most 5 rows"));
        }
        for (i, e) in embeds.iter().enumerate() {
            e.validate().map_err(|err| nested(format!("embeds[{i}]"), err))?;
        }
        for (i, r) in rows.iter().enumerate() {
            r.validate().map_err(|err| nested(format!("components[{i}]"), err))?;
        }
        validate_files(&self.files)?;
        Ok(())
    }
}

impl From<&str> for EditMessage {
    fn from(s: &str) -> Self {
        Self::new().content(s)
    }
}
impl From<String> for EditMessage {
    fn from(s: String) -> Self {
        Self::new().content(s)
    }
}

impl IntoBody for EditMessage {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        Ok(RequestBody::json(&self)?.with_files(self.uploads()))
    }
}
