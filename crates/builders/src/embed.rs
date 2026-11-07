//! `EmbedBuilder`.

use crate::check_len;
use dr_core::{Timestamp, ValidationError};
use serde::Serialize;

/// Builds one embed. `Serialize` writes the wire form.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EmbedBuilder {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    footer: Option<EmbedText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<EmbedName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<EmbedUrl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail: Option<EmbedUrl>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<EmbedFieldData>,
}

#[derive(Clone, Debug, Serialize)]
struct EmbedText {
    text: String,
}
#[derive(Clone, Debug, Serialize)]
struct EmbedName {
    name: String,
}
#[derive(Clone, Debug, Serialize)]
struct EmbedUrl {
    url: String,
}
#[derive(Clone, Debug, Serialize)]
struct EmbedFieldData {
    name: String,
    value: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    inline: bool,
}

impl EmbedBuilder {
    /// An empty embed.
    pub fn new() -> Self {
        Self::default()
    }
    /// Sets the title.
    pub fn title(mut self, s: impl Into<String>) -> Self {
        self.title = Some(s.into());
        self
    }
    /// Sets the description.
    pub fn description(mut self, s: impl Into<String>) -> Self {
        self.description = Some(s.into());
        self
    }
    /// Sets the title link.
    pub fn url(mut self, s: impl Into<String>) -> Self {
        self.url = Some(s.into());
        self
    }
    /// Sets the display time.
    pub fn timestamp(mut self, t: Timestamp) -> Self {
        self.timestamp = Some(t);
        self
    }
    /// Sets the colour, as `0xRRGGBB`.
    pub fn color(mut self, c: u32) -> Self {
        self.color = Some(c);
        self
    }
    /// Sets the footer text.
    pub fn footer(mut self, s: impl Into<String>) -> Self {
        self.footer = Some(EmbedText { text: s.into() });
        self
    }
    /// Sets the author name.
    pub fn author(mut self, s: impl Into<String>) -> Self {
        self.author = Some(EmbedName { name: s.into() });
        self
    }
    /// Sets the image URL.
    pub fn image(mut self, s: impl Into<String>) -> Self {
        self.image = Some(EmbedUrl { url: s.into() });
        self
    }
    /// Sets the thumbnail URL.
    pub fn thumbnail(mut self, s: impl Into<String>) -> Self {
        self.thumbnail = Some(EmbedUrl { url: s.into() });
        self
    }
    /// Adds a field.
    pub fn field(mut self, name: impl Into<String>, value: impl Into<String>, inline: bool) -> Self {
        self.fields.push(EmbedFieldData { name: name.into(), value: value.into(), inline });
        self
    }
    /// Checks the embed limits of `[types 4.3]`, including the total of 6000 UTF-16 units. The path
    /// in an error is relative to this embed, and `CreateMessage` prefixes it.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let field_len = |s: &Option<String>| s.as_deref().map_or(0, |s| s.encode_utf16().count());
        if let Some(t) = &self.title {
            check_len("title", t, 1, 256)?;
        }
        if let Some(d) = &self.description {
            check_len("description", d, 1, 4096)?;
        }
        if let Some(f) = &self.footer {
            check_len("footer.text", &f.text, 1, 2048)?;
        }
        if let Some(a) = &self.author {
            check_len("author.name", &a.name, 1, 256)?;
        }
        if self.color.is_some_and(|c| c > 0xFF_FFFF) {
            return Err(ValidationError::new("color", "must be at most 0xFFFFFF"));
        }
        if self.fields.len() > 25 {
            return Err(ValidationError::new("fields", "must hold at most 25 fields"));
        }
        let mut total = field_len(&self.title) + field_len(&self.description);
        total += self.footer.as_ref().map_or(0, |f| f.text.encode_utf16().count());
        total += self.author.as_ref().map_or(0, |a| a.name.encode_utf16().count());
        for (i, f) in self.fields.iter().enumerate() {
            check_len(&format!("fields[{i}].name"), &f.name, 1, 256)?;
            check_len(&format!("fields[{i}].value"), &f.value, 1, 1024)?;
            total += f.name.encode_utf16().count() + f.value.encode_utf16().count();
        }
        if total > 6000 {
            return Err(ValidationError::new("", format!("an embed may hold 6000 characters in total, got {total}")));
        }
        Ok(())
    }
}
