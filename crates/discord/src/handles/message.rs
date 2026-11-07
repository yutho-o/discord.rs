//! `Bound<Message>`, and the request behind `reply` and `send`.

use crate::handles::{ChannelHandle, GuildHandle, UserHandle};
use crate::{Bound, Call, Client, Error};
use dr_builders::{CreateMessage, CreateThread, EditMessage};
use dr_model::{Channel, ChannelId, Message, Nonce, UserId};
use dr_rest::RetryClass;
use futures::future::BoxFuture;
use std::future::IntoFuture;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// A nonce no other send of this process has used, at most 25 characters: 16 of time, 8 of counter.
fn new_nonce() -> Nonce {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let time = u64::try_from(millis).unwrap_or(u64::MAX);
    Nonce::Str(format!("{time:x}.{:x}", COUNTER.fetch_add(1, Ordering::Relaxed)))
}

/// A message about to be sent, with options that change its body before any I/O. It sets a nonce when
/// the body has none, so that a retry is `RetryClass::Protected`. Validation errors surface when it
/// is awaited, with nothing sent.
#[must_use = "a request does nothing until it is awaited"]
#[derive(Debug)]
pub struct SendMessageRequest {
    client: Client,
    channel: ChannelId,
    body: CreateMessage,
}

impl SendMessageRequest {
    /// A message to send to `channel`. `reply` sets the body's reference first.
    pub(crate) fn new(client: Client, channel: ChannelId, body: CreateMessage) -> Self {
        Self { client, channel, body }
    }

    /// Pings nobody.
    pub fn no_mentions(mut self) -> Self {
        self.body = self.body.no_mentions();
        self
    }
}

impl IntoFuture for SendMessageRequest {
    type Output = Result<Bound<Message>, Error>;
    type IntoFuture = BoxFuture<'static, Self::Output>;
    fn into_future(self) -> Self::IntoFuture {
        let Self { client, channel, body } = self;
        let body = if body.has_nonce() { body } else { body.nonce(new_nonce()) };
        Box::pin(async move {
            let request = client.rest().channels().create_message(channel, body).retry_class(RetryClass::Protected);
            let message = request.await?;
            Ok(client.bind(message))
        })
    }
}

impl Bound<Message> {
    /// Replies to this message: a message in the same channel with a reply reference. It does not
    /// fail if the message was deleted meanwhile.
    pub fn reply(&self, body: impl Into<CreateMessage>) -> SendMessageRequest {
        SendMessageRequest::new(self.client().clone(), self.channel_id, body.into().reply_to(self.id))
    }

    /// Edits this message. A separate body type from `reply`, so that an edit writes only what was
    /// asked for. The result is fresh from Discord, and the cache catches up on the gateway event.
    pub fn edit(&self, body: EditMessage) -> Call<Bound<Message>> {
        let request = self.client().rest().channels().edit_message(self.channel_id, self.id, body);
        Call::new(self.client().clone(), request)
    }

    /// Starts a thread from this message. The thread's id is the message's id.
    pub fn start_thread(&self, body: CreateThread) -> Call<Bound<Channel>> {
        let request = self.client().rest().channels().start_thread_from_message(self.channel_id, self.id, body);
        Call::new(self.client().clone(), request)
    }

    /// Deletes this message.
    pub fn delete(&self) -> Call<()> {
        let request = self.client().rest().channels().delete_message(self.channel_id, self.id);
        Call::new(self.client().clone(), request)
    }

    /// Adds the bot's reaction. `emoji` is a unicode emoji, or `name:id` for a custom one.
    pub fn react(&self, emoji: &str) -> Call<()> {
        let request = self.client().rest().channels().add_message_reaction(self.channel_id, self.id, emoji);
        Call::new(self.client().clone(), request)
    }

    /// Removes the bot's own reaction.
    pub fn unreact(&self, emoji: &str) -> Call<()> {
        let request = self.client().rest().channels().delete_own_message_reaction(self.channel_id, self.id, emoji);
        Call::new(self.client().clone(), request)
    }

    /// Removes someone else's reaction. Needs `MANAGE_MESSAGES`.
    pub fn remove_reaction(&self, emoji: &str, user: UserId) -> Call<()> {
        let request =
            self.client().rest().channels().delete_user_message_reaction(self.channel_id, self.id, emoji, user);
        Call::new(self.client().clone(), request)
    }

    /// Removes every reaction. Needs `MANAGE_MESSAGES`.
    pub fn clear_reactions(&self) -> Call<()> {
        let request = self.client().rest().channels().delete_all_message_reactions(self.channel_id, self.id);
        Call::new(self.client().clone(), request)
    }

    /// Pins this message. Use `.reason(...)` for the audit log.
    pub fn pin(&self) -> Call<()> {
        let request = self.client().rest().channels().pin_message(self.channel_id, self.id);
        Call::new(self.client().clone(), request)
    }

    /// Unpins this message. Use `.reason(...)` for the audit log.
    pub fn unpin(&self) -> Call<()> {
        let request = self.client().rest().channels().unpin_message(self.channel_id, self.id);
        Call::new(self.client().clone(), request)
    }

    /// The newest version the cache has, or this one when the cache has none. No I/O.
    pub fn latest(&self) -> Bound<Message> {
        match self.client().cache().message(self.channel_id, self.id) {
            Some(newest) => self.client().bind_arc(newest),
            None => self.clone(),
        }
    }

    /// The author, as a handle.
    pub fn author(&self) -> UserHandle {
        self.client().user(self.author.id)
    }

    /// The channel, as a handle.
    pub fn channel(&self) -> ChannelHandle {
        self.client().channel(self.channel_id)
    }

    /// The guild, as a handle. `None` in a DM.
    pub fn guild(&self) -> Option<GuildHandle> {
        self.guild_id.map(|g| self.client().guild(g))
    }
}
