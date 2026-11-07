//! Channel and message endpoints.

use crate::encode::encode_path_segment;
use crate::{Empty, Method, Request, Rest};
use dr_core::{IntoBody, RequestBody};
use dr_model::{Channel, ChannelId, Message, MessageId, routes};

/// Channel and message endpoints. Get one from [`Rest::channels`].
#[derive(Clone, Copy, Debug)]
pub struct ChannelsApi<'a>(pub(crate) &'a Rest);

impl ChannelsApi<'_> {
    /// `GET /channels/{channel}`.
    pub fn get(&self, channel: ChannelId) -> Request<Channel> {
        Request::new(self.0.clone(), Method::Get, routes::channel(channel), Ok(RequestBody::empty()))
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

    /// `POST /channels/{channel}/typing`.
    pub fn show_typing(&self, channel: ChannelId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Post, routes::channel_typing(channel), Ok(RequestBody::empty()))
    }
}
