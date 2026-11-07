//! `PollBuilder`, the `poll` of a new message. Limits from Discord's poll documentation, and from
//! discord.js's `PollBuilder` in `packages/builders` (Apache-2.0).

use crate::{check_len, nested};
use dr_core::ValidationError;
use dr_model::{Emoji, EmojiId, PollLayoutType};
use serde::Serialize;

/// At most this many answers.
pub const MAX_POLL_ANSWERS: usize = 10;
/// The longest a poll can last, in hours: 32 days.
pub const MAX_POLL_HOURS: u32 = 768;

/// A poll to send with a message: a question, up to 10 answers, and how long it stays open. Polls
/// cannot be edited once sent.
///
/// ```
/// # use dr_builders::{CreateMessage, PollBuilder};
/// let body = CreateMessage::new()
///     .poll(PollBuilder::new("Pizza or pasta?").answer("Pizza").answer_with_emoji("Pasta", "🍝").hours(4));
/// ```
#[derive(Clone, Debug, Serialize)]
pub struct PollBuilder {
    question: Media,
    answers: Vec<Answer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<u32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    allow_multiselect: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    layout_type: Option<PollLayoutType>,
}

#[derive(Clone, Debug, Serialize)]
struct Media {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    emoji: Option<MediaEmoji>,
}

/// Discord takes a custom emoji by id and a unicode one by name.
#[derive(Clone, Debug, Serialize)]
struct MediaEmoji {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<EmojiId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct Answer {
    poll_media: Media,
}

impl PollBuilder {
    /// A poll with this question, up to 300 characters, and no answers yet. Discord's default
    /// duration is 24 hours.
    pub fn new(question: impl Into<String>) -> Self {
        Self {
            question: Media { text: Some(question.into()), emoji: None },
            answers: Vec::new(),
            duration: None,
            allow_multiselect: false,
            layout_type: None,
        }
    }

    /// Adds an answer, up to 55 characters.
    pub fn answer(mut self, text: impl Into<String>) -> Self {
        self.answers.push(Answer { poll_media: Media { text: Some(text.into()), emoji: None } });
        self
    }

    /// Adds an answer with an emoji: a unicode emoji such as `"🍕"`, or a custom one as `name:id`,
    /// `<:name:id>` or a bare id.
    pub fn answer_with_emoji(mut self, text: impl Into<String>, emoji: &str) -> Self {
        // Text that is no emoji keeps neither, which `validate` rejects.
        let emoji = Some(Emoji::parse(emoji).map_or(MediaEmoji { id: None, name: None }, |e| match e.id {
            Some(id) => MediaEmoji { id: Some(id), name: None },
            None => MediaEmoji { id: None, name: e.name },
        }));
        self.answers.push(Answer { poll_media: Media { text: Some(text.into()), emoji } });
        self
    }

    /// How long it stays open, in hours, from 1 to 768 (32 days).
    pub fn hours(mut self, hours: u32) -> Self {
        self.duration = Some(hours);
        self
    }

    /// Lets each user pick several answers.
    pub fn multiselect(mut self, on: bool) -> Self {
        self.allow_multiselect = on;
        self
    }

    /// The layout. There is only `PollLayoutType::DEFAULT` so far.
    pub fn layout(mut self, layout: PollLayoutType) -> Self {
        self.layout_type = Some(layout);
        self
    }

    /// Checks a question of 1 to 300 characters, 1 to 10 answers of 1 to 55 characters, each with a
    /// usable emoji when it has one, and a duration of 1 to 768 hours.
    pub fn validate(&self) -> Result<(), ValidationError> {
        check_len("question.text", self.question.text.as_deref().unwrap_or_default(), 1, 300)?;
        if self.answers.is_empty() || self.answers.len() > MAX_POLL_ANSWERS {
            return Err(ValidationError::new("answers", format!("must hold 1 to {MAX_POLL_ANSWERS} answers")));
        }
        for (i, answer) in self.answers.iter().enumerate() {
            let media = &answer.poll_media;
            check_len("poll_media.text", media.text.as_deref().unwrap_or_default(), 1, 55)
                .map_err(|e| nested(format!("answers[{i}]"), e))?;
            if let Some(emoji) = &media.emoji
                && emoji.id.is_none()
                && emoji.name.as_deref().is_none_or(str::is_empty)
            {
                return Err(ValidationError::new(format!("answers[{i}].poll_media.emoji"), "is not an emoji"));
            }
        }
        if let Some(hours) = self.duration
            && !(1..=MAX_POLL_HOURS).contains(&hours)
        {
            return Err(ValidationError::new("duration", format!("must be 1 to {MAX_POLL_HOURS} hours, got {hours}")));
        }
        Ok(())
    }
}
