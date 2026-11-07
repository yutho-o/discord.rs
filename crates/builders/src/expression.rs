//! Guild emojis and stickers: `CreateEmoji`, `EditEmoji`, `CreateSticker`, `EditSticker`. Limits from
//! Discord's emoji and sticker documentation, and from discord.js's `GuildEmojiManager` and
//! `GuildStickerManager` (Apache-2.0).

use crate::{CreateAttachment, ImageData, check_len};
use dr_core::{BodyError, FileUpload, IntoBody, RequestBody, ValidationError};
use dr_model::RoleId;
use serde::Serialize;

/// The largest emoji image Discord takes, 256 KiB.
pub const MAX_EMOJI_BYTES: usize = 256 * 1024;
/// The largest sticker file Discord takes, 512 KiB.
pub const MAX_STICKER_BYTES: usize = 512 * 1024;

/// Fails unless `name` is 2 to 32 characters of letters, digits and underscores, as Discord asks of
/// an emoji name.
fn check_emoji_name(name: &str) -> Result<(), ValidationError> {
    check_len("name", name, 2, 32)?;
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(ValidationError::new("name", "may hold only letters, digits and underscores"));
    }
    Ok(())
}

/// A new custom emoji of a guild. Needs `CREATE_GUILD_EXPRESSIONS`.
#[derive(Clone, Debug, Serialize)]
pub struct CreateEmoji {
    name: String,
    image: ImageData,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    roles: Vec<RoleId>,
    #[serde(skip)]
    image_len: usize,
}

impl CreateEmoji {
    /// An emoji named `name` from an image: PNG, JPEG, GIF or WebP, at most 256 KiB.
    pub fn new(name: impl Into<String>, image: &[u8], media_type: &str) -> Self {
        Self { name: name.into(), image: ImageData::new(media_type, image), roles: Vec::new(), image_len: image.len() }
    }

    /// Limits the emoji to members with one of these roles.
    pub fn roles(mut self, roles: impl IntoIterator<Item = RoleId>) -> Self {
        self.roles = roles.into_iter().collect();
        self
    }

    /// Checks the name and the image size.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_emoji_name(&self.name)?;
        if self.image_len > MAX_EMOJI_BYTES {
            return Err(ValidationError::new("image", format!("must be at most {MAX_EMOJI_BYTES} bytes")));
        }
        Ok(())
    }
}

impl IntoBody for CreateEmoji {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

/// An edit of a guild emoji. What is not set stays as it is.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditEmoji {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    roles: Option<Vec<RoleId>>,
}

impl EditEmoji {
    /// An edit that changes nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Renames it.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Limits it to these roles. An empty list opens it to everyone.
    pub fn roles(mut self, roles: impl IntoIterator<Item = RoleId>) -> Self {
        self.roles = Some(roles.into_iter().collect());
        self
    }

    /// Checks the name.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.name.as_deref().map_or(Ok(()), check_emoji_name)
    }
}

impl IntoBody for EditEmoji {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}

/// A new sticker of a guild, sent as a form with its file. Needs `CREATE_GUILD_EXPRESSIONS`.
#[derive(Clone, Debug)]
pub struct CreateSticker {
    name: String,
    description: String,
    tags: String,
    file: FileUpload,
}

impl CreateSticker {
    /// A sticker named `name`, 2 to 30 characters. `tags` is what suggests it, usually the name of a
    /// unicode emoji such as `pizza`, up to 200 characters. `file` is a PNG, APNG or GIF of at most
    /// 512 KiB, or a Lottie JSON file. Set the file's content type, such as `image/png`, so that
    /// Discord knows its format.
    pub fn new(name: impl Into<String>, tags: impl Into<String>, file: CreateAttachment) -> Self {
        Self { name: name.into(), description: String::new(), tags: tags.into(), file: file.upload().field("file") }
    }

    /// Sets the description, empty or 2 to 100 characters.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Checks the name, description, tags and file size.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_len("name", &self.name, 2, 30)?;
        if !self.description.is_empty() {
            check_len("description", &self.description, 2, 100)?;
        }
        check_len("tags", &self.tags, 1, 200)?;
        if self.file.data.len() > MAX_STICKER_BYTES {
            return Err(ValidationError::new("file", format!("must be at most {MAX_STICKER_BYTES} bytes")));
        }
        Ok(())
    }
}

impl IntoBody for CreateSticker {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        let fields = [("name", self.name), ("description", self.description), ("tags", self.tags)];
        Ok(RequestBody::form(fields.map(|(k, v)| (k.to_owned(), v))).with_files([self.file]))
    }
}

/// An edit of a guild sticker. What is not set stays as it is.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EditSticker {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<String>,
}

impl EditSticker {
    /// An edit that changes nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Renames it, 2 to 30 characters.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the description, 2 to 100 characters.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(Some(description.into()));
        self
    }

    /// Removes the description.
    pub fn clear_description(mut self) -> Self {
        self.description = Some(None);
        self
    }

    /// Sets the tags, up to 200 characters.
    pub fn tags(mut self, tags: impl Into<String>) -> Self {
        self.tags = Some(tags.into());
        self
    }

    /// Checks the fields that are set.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(name) = &self.name {
            check_len("name", name, 2, 30)?;
        }
        if let Some(Some(description)) = &self.description {
            check_len("description", description, 2, 100)?;
        }
        if let Some(tags) = &self.tags {
            check_len("tags", tags, 1, 200)?;
        }
        Ok(())
    }
}

impl IntoBody for EditSticker {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        self.validate()?;
        RequestBody::json(&self)
    }
}
