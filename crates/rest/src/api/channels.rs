//! Channel and message endpoints.

use crate::encode::encode_path_segment;
use crate::{Empty, Method, Request, Rest};
use dr_core::Timestamp;
use dr_core::{IntoBody, RequestBody};
use dr_model::{
    Channel, ChannelId, GenericId, Message, MessageId, PermissionOverwrite, PollAnswerVoters, ThreadList, ThreadMember,
    UserId, routes,
};
use serde::Serialize;

/// Which messages of a channel to list. With no anchor, the newest ones.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct MessagePage {
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    before: Option<MessageId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<MessageId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    around: Option<MessageId>,
}

impl MessagePage {
    /// The default page: Discord's 50 newest messages.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many messages, 1 to 100.
    #[must_use]
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Messages before this one.
    #[must_use]
    pub fn before(mut self, message: MessageId) -> Self {
        self.before = Some(message);
        self
    }

    /// Messages after this one.
    #[must_use]
    pub fn after(mut self, message: MessageId) -> Self {
        self.after = Some(message);
        self
    }

    /// Messages around this one.
    #[must_use]
    pub fn around(mut self, message: MessageId) -> Self {
        self.around = Some(message);
        self
    }
}

/// Which voters of a poll answer to list. With no anchor, the first ones.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct VoterPage {
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}

impl VoterPage {
    /// The first page: Discord's 25 first voters.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many voters, 1 to 100.
    #[must_use]
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Voters after this user.
    #[must_use]
    pub fn after(mut self, user: UserId) -> Self {
        self.after = Some(user);
        self
    }
}

/// Which archived threads to list. With no anchor, the most recently archived ones.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct ArchivedThreadPage {
    #[serde(skip_serializing_if = "Option::is_none")]
    before: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}

impl ArchivedThreadPage {
    /// The most recently archived threads.
    pub fn new() -> Self {
        Self::default()
    }

    /// At most this many threads.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Threads archived before this time: the `archive_timestamp` of the last thread of a page.
    pub fn before(mut self, time: Timestamp) -> Self {
        self.before = Some(time);
        self
    }
}

/// Which members of a thread to list.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct ThreadMemberPage {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    with_member: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}

impl ThreadMemberPage {
    /// The first members.
    pub fn new() -> Self {
        Self::default()
    }

    /// At most this many, 1 to 100.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Members after this user id.
    pub fn after(mut self, user: UserId) -> Self {
        self.after = Some(user);
        self
    }

    /// Includes each one's guild member, which needs the `GUILD_MEMBERS` intent.
    pub fn with_member(mut self) -> Self {
        self.with_member = true;
        self
    }
}

/// Channel and message endpoints. Get one from [`Rest::channels`].
#[derive(Clone, Copy, Debug)]
pub struct ChannelsApi<'a>(pub(crate) &'a Rest);

impl ChannelsApi<'_> {
    /// `GET /channels/{channel}`.
    pub fn get(&self, channel: ChannelId) -> Request<Channel> {
        Request::new(self.0.clone(), Method::Get, routes::channel(channel), Ok(RequestBody::empty()))
    }

    /// `PATCH /channels/{channel}`. Use `.reason(...)` for the audit log.
    pub fn modify(&self, channel: ChannelId, body: impl IntoBody) -> Request<Channel> {
        Request::new(self.0.clone(), Method::Patch, routes::channel(channel), body.into_body())
    }

    /// `DELETE /channels/{channel}`. Returns the channel as it was. Use `.reason(...)` for the audit log.
    pub fn delete(&self, channel: ChannelId) -> Request<Channel> {
        Request::new(self.0.clone(), Method::Delete, routes::channel(channel), Ok(RequestBody::empty()))
    }

    /// `POST /channels/{channel}/messages/{message}/threads`: a thread started from a message.
    pub fn start_thread_from_message(
        &self,
        channel: ChannelId,
        message: MessageId,
        body: impl IntoBody,
    ) -> Request<Channel> {
        let path = routes::channel_message_threads(channel, message);
        Request::new(self.0.clone(), Method::Post, path, body.into_body())
    }

    /// `POST /channels/{channel}/threads`: a thread with no message, or a forum post.
    pub fn start_thread(&self, channel: ChannelId, body: impl IntoBody) -> Request<Channel> {
        Request::new(self.0.clone(), Method::Post, routes::channel_threads(channel), body.into_body())
    }

    /// `GET /channels/{channel}/threads/archived/public` or `.../private`.
    pub fn archived_threads(&self, channel: ChannelId, private: bool, page: ArchivedThreadPage) -> Request<ThreadList> {
        let path = routes::channel_archived_threads(channel, private);
        Request::new(self.0.clone(), Method::Get, path, Ok(RequestBody::empty())).query(page)
    }

    /// `GET /channels/{channel}/users/@me/threads/archived/private`: the archived private threads the
    /// bot joined.
    pub fn joined_archived_threads(&self, channel: ChannelId, page: ArchivedThreadPage) -> Request<ThreadList> {
        let path = routes::channel_joined_archived_threads(channel);
        Request::new(self.0.clone(), Method::Get, path, Ok(RequestBody::empty())).query(page)
    }

    /// `PUT /channels/{thread}/thread-members/@me`.
    pub fn join_thread(&self, thread: ChannelId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Put, routes::thread_member_me(thread), Ok(RequestBody::empty()))
    }

    /// `DELETE /channels/{thread}/thread-members/@me`.
    pub fn leave_thread(&self, thread: ChannelId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Delete, routes::thread_member_me(thread), Ok(RequestBody::empty()))
    }

    /// `PUT /channels/{thread}/thread-members/{user}`.
    pub fn add_thread_member(&self, thread: ChannelId, user: UserId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Put, routes::thread_member(thread, user), Ok(RequestBody::empty()))
    }

    /// `DELETE /channels/{thread}/thread-members/{user}`.
    pub fn remove_thread_member(&self, thread: ChannelId, user: UserId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Delete, routes::thread_member(thread, user), Ok(RequestBody::empty()))
    }

    /// `GET /channels/{thread}/thread-members/{user}`.
    pub fn get_thread_member(&self, thread: ChannelId, user: UserId) -> Request<ThreadMember> {
        Request::new(self.0.clone(), Method::Get, routes::thread_member(thread, user), Ok(RequestBody::empty()))
    }

    /// `GET /channels/{thread}/thread-members`.
    pub fn get_thread_members(&self, thread: ChannelId, page: ThreadMemberPage) -> Request<Vec<ThreadMember>> {
        let path = routes::thread_members(thread);
        Request::new(self.0.clone(), Method::Get, path, Ok(RequestBody::empty())).query(page)
    }

    /// `PUT /channels/{channel}/permissions/{overwrite}`: creates or replaces one overwrite.
    pub fn edit_permission(&self, channel: ChannelId, overwrite: &PermissionOverwrite) -> Request<Empty> {
        let body = serde_json::json!({"type": overwrite.kind, "allow": overwrite.allow, "deny": overwrite.deny});
        Request::new(
            self.0.clone(),
            Method::Put,
            routes::channel_permission(channel, overwrite.id),
            RequestBody::json(&body),
        )
    }

    /// `DELETE /channels/{channel}/permissions/{overwrite}`.
    pub fn delete_permission(&self, channel: ChannelId, overwrite: GenericId) -> Request<Empty> {
        let path = routes::channel_permission(channel, overwrite);
        Request::new(self.0.clone(), Method::Delete, path, Ok(RequestBody::empty()))
    }

    /// `POST /channels/{channel}/messages`. The retry class is `Protected` only when the body holds
    /// a nonce, which the facade guarantees through `RetryClass` on the returned request.
    pub fn create_message(&self, channel: ChannelId, body: impl IntoBody) -> Request<Message> {
        Request::new(self.0.clone(), Method::Post, routes::channel_messages(channel), body.into_body())
    }

    /// `GET /channels/{channel}/messages/{message}`.
    pub fn get_message(&self, channel: ChannelId, message: MessageId) -> Request<Message> {
        Request::new(self.0.clone(), Method::Get, routes::channel_message(channel, message), Ok(RequestBody::empty()))
    }

    /// `PATCH /channels/{channel}/messages/{message}`.
    pub fn edit_message(&self, channel: ChannelId, message: MessageId, body: impl IntoBody) -> Request<Message> {
        Request::new(self.0.clone(), Method::Patch, routes::channel_message(channel, message), body.into_body())
    }

    /// `DELETE /channels/{channel}/messages/{message}`.
    pub fn delete_message(&self, channel: ChannelId, message: MessageId) -> Request<Empty> {
        Request::new(
            self.0.clone(),
            Method::Delete,
            routes::channel_message(channel, message),
            Ok(RequestBody::empty()),
        )
    }

    /// `PUT /channels/{channel}/messages/{message}/reactions/{emoji}/@me`. `emoji` is the plain
    /// form, a unicode emoji or `name:id`, and this encodes it.
    pub fn add_message_reaction(&self, channel: ChannelId, message: MessageId, emoji: &str) -> Request<Empty> {
        let path = routes::channel_message_own_reaction(channel, message, &encode_path_segment(emoji));
        Request::new(self.0.clone(), Method::Put, path, Ok(RequestBody::empty()))
    }

    /// `DELETE /channels/{channel}/messages/{message}/reactions/{emoji}/@me`.
    pub fn delete_own_message_reaction(&self, channel: ChannelId, message: MessageId, emoji: &str) -> Request<Empty> {
        let path = routes::channel_message_own_reaction(channel, message, &encode_path_segment(emoji));
        Request::new(self.0.clone(), Method::Delete, path, Ok(RequestBody::empty()))
    }

    /// `DELETE /channels/{channel}/messages/{message}/reactions/{emoji}/{user}`.
    pub fn delete_user_message_reaction(
        &self,
        channel: ChannelId,
        message: MessageId,
        emoji: &str,
        user: UserId,
    ) -> Request<Empty> {
        let path = routes::channel_message_user_reaction(channel, message, &encode_path_segment(emoji), user);
        Request::new(self.0.clone(), Method::Delete, path, Ok(RequestBody::empty()))
    }

    /// `GET /channels/{channel}/polls/{message}/answers/{answer}`.
    pub fn get_poll_answer_voters(
        &self,
        channel: ChannelId,
        message: MessageId,
        answer: u32,
        page: VoterPage,
    ) -> Request<PollAnswerVoters> {
        let path = routes::channel_poll_answer(channel, message, answer);
        Request::new(self.0.clone(), Method::Get, path, Ok(RequestBody::empty())).query(page)
    }

    /// `POST /channels/{channel}/polls/{message}/expire`. Ends a poll the bot sent, at once.
    pub fn expire_poll(&self, channel: ChannelId, message: MessageId) -> Request<Message> {
        let path = routes::channel_poll_expire(channel, message);
        Request::new(self.0.clone(), Method::Post, path, Ok(RequestBody::empty()))
    }

    /// `DELETE /channels/{channel}/messages/{message}/reactions`.
    pub fn delete_all_message_reactions(&self, channel: ChannelId, message: MessageId) -> Request<Empty> {
        let path = routes::channel_message_all_reactions(channel, message);
        Request::new(self.0.clone(), Method::Delete, path, Ok(RequestBody::empty()))
    }

    /// `GET /channels/{channel}/messages`.
    pub fn get_messages(&self, channel: ChannelId, page: MessagePage) -> Request<Vec<Message>> {
        Request::new(self.0.clone(), Method::Get, routes::channel_messages(channel), Ok(RequestBody::empty()))
            .query(page)
    }

    /// `POST /channels/{channel}/messages/bulk-delete`.
    pub fn bulk_delete_messages(&self, channel: ChannelId, body: impl IntoBody) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Post, routes::channel_bulk_delete(channel), body.into_body())
    }

    /// `PUT /channels/{channel}/messages/pins/{message}`.
    pub fn pin_message(&self, channel: ChannelId, message: MessageId) -> Request<Empty> {
        Request::new(
            self.0.clone(),
            Method::Put,
            routes::channel_message_pin(channel, message),
            Ok(RequestBody::empty()),
        )
    }

    /// `DELETE /channels/{channel}/messages/pins/{message}`.
    pub fn unpin_message(&self, channel: ChannelId, message: MessageId) -> Request<Empty> {
        let path = routes::channel_message_pin(channel, message);
        Request::new(self.0.clone(), Method::Delete, path, Ok(RequestBody::empty()))
    }

    /// `POST /channels/{channel}/typing`.
    pub fn show_typing(&self, channel: ChannelId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Post, routes::channel_typing(channel), Ok(RequestBody::empty()))
    }
}
