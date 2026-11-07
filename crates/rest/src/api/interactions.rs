//! Interaction callback and interaction webhook endpoints. All of them use `Auth::None`, so a 401
//! does not invalidate the bot token, and the token in the path is encoded and masked.

use crate::encode::encode_path_segment;
use crate::{Auth, Empty, Method, Request, Rest, RetryClass};
use dr_core::{BodyError, IntoBody, RequestBody, Secret};
use dr_model::{ApplicationId, InteractionId, Message, MessageId, routes};
use std::time::Duration;

/// How long Discord waits for an interaction callback.
const CALLBACK_DEADLINE: Duration = Duration::from_secs(3);

/// Interaction endpoints. Get one from [`Rest::interactions`].
#[derive(Clone, Copy, Debug)]
pub struct InteractionsApi<'a>(pub(crate) &'a Rest);

impl InteractionsApi<'_> {
    fn webhook<T>(&self, method: Method, path: String, body: Result<RequestBody, BodyError>) -> Request<T> {
        Request::new(self.0.clone(), method, path, body).auth(Auth::None)
    }

    /// `POST /interactions/{id}/{token}/callback`. `RetryClass::Callback` with the 3 s deadline.
    /// `body` is an `InteractionResponse`.
    pub fn callback(&self, id: InteractionId, token: &Secret, body: impl IntoBody) -> Request<Empty> {
        let path = routes::interaction_callback(id, &encode_path_segment(token.expose()));
        self.webhook(Method::Post, path, body.into_body()).retry_class(RetryClass::Callback).deadline(CALLBACK_DEADLINE)
    }

    /// `GET /webhooks/{app}/{token}/messages/@original`.
    pub fn get_original_reply(&self, app: ApplicationId, token: &Secret) -> Request<Message> {
        let path = routes::webhook_original_message(app, &encode_path_segment(token.expose()));
        self.webhook(Method::Get, path, Ok(RequestBody::empty()))
    }

    /// `PATCH /webhooks/{app}/{token}/messages/@original`.
    pub fn edit_reply(&self, app: ApplicationId, token: &Secret, body: impl IntoBody) -> Request<Message> {
        let path = routes::webhook_original_message(app, &encode_path_segment(token.expose()));
        self.webhook(Method::Patch, path, body.into_body())
    }

    /// `DELETE /webhooks/{app}/{token}/messages/@original`.
    pub fn delete_reply(&self, app: ApplicationId, token: &Secret) -> Request<Empty> {
        let path = routes::webhook_original_message(app, &encode_path_segment(token.expose()));
        self.webhook(Method::Delete, path, Ok(RequestBody::empty()))
    }

    /// `POST /webhooks/{app}/{token}`.
    pub fn follow_up(&self, app: ApplicationId, token: &Secret, body: impl IntoBody) -> Request<Message> {
        let path = routes::webhook(app, &encode_path_segment(token.expose()));
        self.webhook(Method::Post, path, body.into_body())
    }

    /// `GET /webhooks/{app}/{token}/messages/{message}`.
    pub fn get_follow_up(&self, app: ApplicationId, token: &Secret, message: MessageId) -> Request<Message> {
        let path = routes::webhook_message(app, &encode_path_segment(token.expose()), message);
        self.webhook(Method::Get, path, Ok(RequestBody::empty()))
    }

    /// `PATCH /webhooks/{app}/{token}/messages/{message}`.
    pub fn edit_follow_up(
        &self,
        app: ApplicationId,
        token: &Secret,
        message: MessageId,
        body: impl IntoBody,
    ) -> Request<Message> {
        let path = routes::webhook_message(app, &encode_path_segment(token.expose()), message);
        self.webhook(Method::Patch, path, body.into_body())
    }

    /// `DELETE /webhooks/{app}/{token}/messages/{message}`.
    pub fn delete_follow_up(&self, app: ApplicationId, token: &Secret, message: MessageId) -> Request<Empty> {
        let path = routes::webhook_message(app, &encode_path_segment(token.expose()), message);
        self.webhook(Method::Delete, path, Ok(RequestBody::empty()))
    }
}
