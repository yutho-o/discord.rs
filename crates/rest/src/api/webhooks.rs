//! Webhook endpoints. Those that take the webhook's token use `Auth::None`, so they work without a bot
//! token and a 401 on them does not invalidate it. The token in the path is encoded and masked.

use crate::encode::encode_path_segment;
use crate::{Auth, Empty, Method, Request, Rest};
use dr_core::{IntoBody, RequestBody, Secret};
use dr_model::{ChannelId, GuildId, Message, MessageId, Webhook, WebhookId, routes};
use serde::Serialize;

/// Where a webhook message goes, beside the webhook's own channel.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct ExecuteQuery {
    wait: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_id: Option<ChannelId>,
}

impl ExecuteQuery {
    /// Waits for the message, so that it comes back. The facade always does.
    pub fn new() -> Self {
        Self { wait: true, thread_id: None }
    }

    /// Posts in this thread of the webhook's channel.
    pub fn thread(mut self, thread: ChannelId) -> Self {
        self.thread_id = Some(thread);
        self
    }
}

/// Webhook endpoints. Get one from [`Rest::webhooks`].
#[derive(Clone, Copy, Debug)]
pub struct WebhooksApi<'a>(pub(crate) &'a Rest);

impl WebhooksApi<'_> {
    fn with_token<T>(&self, method: Method, path: String, body: Result<RequestBody, dr_core::BodyError>) -> Request<T> {
        Request::new(self.0.clone(), method, path, body).auth(Auth::None)
    }

    /// `POST /channels/{channel}/webhooks`. Use `.reason(...)` for the audit log.
    pub fn create(&self, channel: ChannelId, body: impl IntoBody) -> Request<Webhook> {
        Request::new(self.0.clone(), Method::Post, routes::channel_webhooks(channel), body.into_body())
    }

    /// `GET /channels/{channel}/webhooks`.
    pub fn get_channel_webhooks(&self, channel: ChannelId) -> Request<Vec<Webhook>> {
        Request::new(self.0.clone(), Method::Get, routes::channel_webhooks(channel), Ok(RequestBody::empty()))
    }

    /// `GET /guilds/{guild}/webhooks`.
    pub fn get_guild_webhooks(&self, guild: GuildId) -> Request<Vec<Webhook>> {
        Request::new(self.0.clone(), Method::Get, routes::guild_webhooks(guild), Ok(RequestBody::empty()))
    }

    /// `GET /webhooks/{id}`, with the bot's token.
    pub fn get(&self, webhook: WebhookId) -> Request<Webhook> {
        Request::new(self.0.clone(), Method::Get, routes::webhook_by_id(webhook), Ok(RequestBody::empty()))
    }

    /// `GET /webhooks/{id}/{token}`.
    pub fn get_with_token(&self, webhook: WebhookId, token: &Secret) -> Request<Webhook> {
        let path = routes::webhook_with_token(webhook, &encode_path_segment(token.expose()));
        self.with_token(Method::Get, path, Ok(RequestBody::empty()))
    }

    /// `PATCH /webhooks/{id}`, with the bot's token. Use `.reason(...)` for the audit log.
    pub fn modify(&self, webhook: WebhookId, body: impl IntoBody) -> Request<Webhook> {
        Request::new(self.0.clone(), Method::Patch, routes::webhook_by_id(webhook), body.into_body())
    }

    /// `PATCH /webhooks/{id}/{token}`. It cannot move the webhook to another channel.
    pub fn modify_with_token(&self, webhook: WebhookId, token: &Secret, body: impl IntoBody) -> Request<Webhook> {
        let path = routes::webhook_with_token(webhook, &encode_path_segment(token.expose()));
        self.with_token(Method::Patch, path, body.into_body())
    }

    /// `DELETE /webhooks/{id}`, with the bot's token. Use `.reason(...)` for the audit log.
    pub fn delete(&self, webhook: WebhookId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Delete, routes::webhook_by_id(webhook), Ok(RequestBody::empty()))
    }

    /// `DELETE /webhooks/{id}/{token}`.
    pub fn delete_with_token(&self, webhook: WebhookId, token: &Secret) -> Request<Empty> {
        let path = routes::webhook_with_token(webhook, &encode_path_segment(token.expose()));
        self.with_token(Method::Delete, path, Ok(RequestBody::empty()))
    }

    /// `POST /webhooks/{id}/{token}?wait=true`: posts a message and gives it back.
    pub fn execute(
        &self,
        webhook: WebhookId,
        token: &Secret,
        query: ExecuteQuery,
        body: impl IntoBody,
    ) -> Request<Message> {
        let path = routes::webhook_with_token(webhook, &encode_path_segment(token.expose()));
        self.with_token(Method::Post, path, body.into_body()).query(query)
    }

    /// `GET /webhooks/{id}/{token}/messages/{message}`.
    pub fn get_message(&self, webhook: WebhookId, token: &Secret, message: MessageId) -> Request<Message> {
        let path = routes::webhook_token_message(webhook, &encode_path_segment(token.expose()), message);
        self.with_token(Method::Get, path, Ok(RequestBody::empty()))
    }

    /// `PATCH /webhooks/{id}/{token}/messages/{message}`.
    pub fn edit_message(
        &self,
        webhook: WebhookId,
        token: &Secret,
        message: MessageId,
        body: impl IntoBody,
    ) -> Request<Message> {
        let path = routes::webhook_token_message(webhook, &encode_path_segment(token.expose()), message);
        self.with_token(Method::Patch, path, body.into_body())
    }

    /// `DELETE /webhooks/{id}/{token}/messages/{message}`.
    pub fn delete_message(&self, webhook: WebhookId, token: &Secret, message: MessageId) -> Request<Empty> {
        let path = routes::webhook_token_message(webhook, &encode_path_segment(token.expose()), message);
        self.with_token(Method::Delete, path, Ok(RequestBody::empty()))
    }
}
