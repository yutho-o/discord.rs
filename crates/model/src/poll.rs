//! Polls. Upstream: `payloads/v10/poll.ts`, `rest/v10/poll.ts` and the poll vote events of
//! `gateway/v10.ts` in discord-api-types (MIT, (c) 2020 vladfrangu).
//!
//! The body that creates a poll is `PollBuilder` in the builders crate. These are the forms Discord
//! sends back.

use crate::{ChannelId, Emoji, GuildId, MessageId, User, UserId};
use dr_core::{Timestamp, null_default, open_int};
use serde::{Deserialize, Serialize};

open_int! {
    /// How a poll is laid out.
    PollLayoutType: u16 {
        /// The only layout so far.
        DEFAULT = 1,
    }
}

impl Default for PollLayoutType {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A poll on a message.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Poll {
    /// The question. Polls only support text there.
    pub question: PollMedia,
    /// The answers.
    #[serde(default, deserialize_with = "null_default")]
    pub answers: Vec<PollAnswer>,
    /// When it ends. `None` for a poll that never does, which Discord keeps for future use.
    #[serde(default)]
    pub expiry: Option<Timestamp>,
    /// Whether a user may pick several answers.
    #[serde(default)]
    pub allow_multiselect: bool,
    /// The layout.
    #[serde(default)]
    pub layout_type: PollLayoutType,
    /// The vote counts. Absent means unknown, not that nobody voted. Discord does not always send
    /// them, and they are exact only once `is_finalized` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub results: Option<PollResults>,
}

impl Poll {
    /// The answer with this id.
    pub fn answer(&self, id: u32) -> Option<&PollAnswer> {
        self.answers.iter().find(|a| a.answer_id == id)
    }

    /// How many votes an answer has, from `results`. `None` when the results are absent.
    pub fn votes(&self, answer: u32) -> Option<u32> {
        let results = self.results.as_ref()?;
        Some(results.answer_counts.iter().find(|c| c.id == answer).map_or(0, |c| c.count))
    }
}

/// The text and emoji of a question or an answer.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PollMedia {
    /// The text, up to 300 characters for a question and 55 for an answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// An emoji, on answers only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<Emoji>,
}

/// One answer of a poll.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PollAnswer {
    /// Its id, which numbers answers from 1 in the order they were given.
    pub answer_id: u32,
    /// Its text and emoji.
    pub poll_media: PollMedia,
}

/// The votes of a poll.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PollResults {
    /// Whether the votes are counted exactly, which happens once the poll ended.
    #[serde(default)]
    pub is_finalized: bool,
    /// The count of each answer. An answer nobody chose may be missing.
    #[serde(default, deserialize_with = "null_default")]
    pub answer_counts: Vec<PollAnswerCount>,
}

/// The votes of one answer.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PollAnswerCount {
    /// The answer's id.
    pub id: u32,
    /// How many votes.
    pub count: u32,
    /// Whether the bot voted for it.
    #[serde(default)]
    pub me_voted: bool,
}

/// The users who chose one answer, from `GET .../polls/{message}/answers/{answer}`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PollAnswerVoters {
    /// The voters, one page of them.
    #[serde(default, deserialize_with = "null_default")]
    pub users: Vec<User>,
}

/// `MESSAGE_POLL_VOTE_ADD` and `MESSAGE_POLL_VOTE_REMOVE`: someone picked or dropped an answer.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessagePollVoteEvent {
    /// Who voted.
    pub user_id: UserId,
    /// The channel of the poll.
    pub channel_id: ChannelId,
    /// The poll's message.
    pub message_id: MessageId,
    /// The guild, `None` in a DM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The answer.
    pub answer_id: u32,
}
