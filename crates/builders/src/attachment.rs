//! `CreateAttachment`, a file to upload with a message, and the `attachments` list that goes with it.

use crate::check_len;
use bytes::Bytes;
use dr_core::{FileUpload, ValidationError};
use dr_model::AttachmentId;
use serde::Serialize;

/// Discord takes at most this many files on one message.
pub const MAX_ATTACHMENTS: usize = 10;

/// A file to upload, the upstream `AttachmentBuilder`. An embed shows it with
/// `attachment://<name>` as its image URL.
///
/// ```
/// # use dr_builders::{CreateAttachment, CreateMessage, EmbedBuilder};
/// let chart = CreateAttachment::bytes(vec![0x89, b'P', b'N', b'G'], "chart.png").description("This week");
/// let message = CreateMessage::new()
///     .embed(EmbedBuilder::new().title("Stats").image("attachment://chart.png"))
///     .attach(chart);
/// ```
///
/// Reading a file from disk is the caller's choice, for example
/// `CreateAttachment::bytes(tokio::fs::read(path).await?, "report.txt")`.
#[derive(Clone, Debug)]
pub struct CreateAttachment {
    upload: FileUpload,
    description: Option<String>,
}

impl CreateAttachment {
    /// A file named `name` holding `data`.
    pub fn bytes(data: impl Into<Bytes>, name: impl Into<String>) -> Self {
        Self { upload: FileUpload::new(name, data), description: None }
    }

    /// Sets the alt text, at most 1024 characters.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Hides the file behind a spoiler: Discord does that for names that start with `SPOILER_`.
    pub fn spoiler(mut self) -> Self {
        if !self.is_spoiler() {
            self.upload.name = format!("SPOILER_{}", self.upload.name);
        }
        self
    }

    /// True when the file is hidden behind a spoiler.
    pub fn is_spoiler(&self) -> bool {
        self.upload.name.starts_with("SPOILER_")
    }

    /// Sets the media type, such as `image/png`. Without one, Discord guesses from the name.
    pub fn content_type(mut self, value: impl Into<String>) -> Self {
        self.upload = self.upload.content_type(value);
        self
    }

    /// The name Discord shows, with the `SPOILER_` prefix if any.
    pub fn name(&self) -> &str {
        &self.upload.name
    }

    /// The content.
    pub fn data(&self) -> &Bytes {
        &self.upload.data
    }

    /// Checks the name and the alt text.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_len("filename", &self.upload.name, 1, 1024)?;
        if let Some(d) = &self.description {
            check_len("description", d, 0, 1024)?;
        }
        Ok(())
    }

    pub(crate) fn upload(&self) -> FileUpload {
        self.upload.clone()
    }

    pub(crate) fn reference(&self, index: usize) -> AttachmentRef {
        AttachmentRef::New { id: index, filename: self.upload.name.clone(), description: self.description.clone() }
    }
}

/// One entry of the `attachments` list of a message body: a new file, by its index among the
/// uploaded files, or an attachment the message already has and keeps.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum AttachmentRef {
    /// A file of this request, `files[id]`.
    New {
        id: usize,
        filename: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    /// An attachment already on the message.
    Existing { id: AttachmentId },
}

/// Checks a list of files: how many, and each one.
pub(crate) fn validate_files(files: &[CreateAttachment]) -> Result<(), ValidationError> {
    if files.len() > MAX_ATTACHMENTS {
        return Err(ValidationError::new("files", format!("must hold at most {MAX_ATTACHMENTS} files")));
    }
    for (i, f) in files.iter().enumerate() {
        f.validate().map_err(|err| crate::nested(format!("files[{i}]"), err))?;
    }
    Ok(())
}
