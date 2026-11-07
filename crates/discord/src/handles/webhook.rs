//! `WebhookHandle`, `WebhooksManager` and `Bound<Webhook>`.

use crate::handles::ChannelHandle;
use crate::{Bound, Call, Client, Error};
use dr_builders::{CreateWebhook, EditMessage, EditWebhook, ExecuteWebhook};
use dr_core::Secret;
use dr_model::{ChannelId, Message, MessageId, Webhook, WebhookId};
use dr_rest::api::ExecuteQuery;

/// A webhook by id, with its token if known: the upstream `WebhookClient`. No I/O to create.
///
/// With a token, it posts, edits and deletes without the bot's token, so a client built with any
/// token can drive a webhook from its URL.
#[derive(Clone, Debug)]
pub struct WebhookHandle {
    client: Client,
    id: WebhookId,
    token: Option<Secret>,
}

impl Client {
    /// A handle to a webhook, with its token to post through it.
    pub fn webhook(&self, id: WebhookId, token: impl Into<Secret>) -> WebhookHandle {
        WebhookHandle { client: self.clone(), id, token: Some(token.into()) }
    }

    /// A handle to a webhook from its URL, `https://discord.com/api/webhooks/{id}/{token}`. `None`
    /// when the URL is not one.
    pub fn webhook_from_url(&self, url: &str) -> Option<WebhookHandle> {
        let (id, token) = parse_webhook_url(url)?;
        Some(self.webhook(id, token))
    }

    /// A handle to a webhook by id alone, for what the bot's token can do: fetch, edit, delete.
    pub fn webhook_by_id(&self, id: WebhookId) -> WebhookHandle {
        WebhookHandle { client: self.clone(), id, token: None }
    }
}

/// The id and the token of a webhook URL.
fn parse_webhook_url(url: &str) -> Option<(WebhookId, String)> {
    let path = url.split(['?', '#']).next()?;
    let mut parts = path.trim_end_matches('/').rsplit('/');
    let token = parts.next()?;
    let id = parts.next()?.parse().ok()?;
    let anchor = parts.next()?;
    let valid = anchor == "webhooks"
        && !token.is_empty()
        && token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    valid.then(|| (id, token.to_owned()))
}

impl WebhookHandle {
    /// The id.
    pub fn id(&self) -> WebhookId {
        self.id
    }

    fn token(&self) -> Result<&Secret, Error> {
        self.token.as_ref().ok_or_else(|| Error::Other(String::from("this webhook handle has no token")))
    }

    /// Posts a message and gives it back.
    pub fn execute(&self, body: impl Into<ExecuteWebhook>) -> Call<Bound<Message>> {
        self.execute_with(body, ExecuteQuery::new())
    }

    /// Posts a message in a thread of the webhook's channel.
    pub fn execute_in_thread(&self, thread: ChannelId, body: impl Into<ExecuteWebhook>) -> Call<Bound<Message>> {
        self.execute_with(body, ExecuteQuery::new().thread(thread))
    }

    fn execute_with(&self, body: impl Into<ExecuteWebhook>, query: ExecuteQuery) -> Call<Bound<Message>> {
        match self.token() {
            Ok(token) => Call::new(
                self.client.clone(),
                self.client.rest().webhooks().execute(self.id, token, query, body.into()),
            ),
            Err(e) => Call::failed(self.client.clone(), e),
        }
    }

    /// Fetches the webhook: with its token if known, else with the bot's.
    pub fn fetch(&self) -> Call<Bound<Webhook>> {
        let api = self.client.rest().webhooks();
        let request = match &self.token {
            Some(token) => api.get_with_token(self.id, token),
            None => api.get(self.id),
        };
        Call::new(self.client.clone(), request)
    }

    /// Edits the webhook. Moving it to another channel always uses the bot's token.
    pub fn edit(&self, body: EditWebhook) -> Call<Bound<Webhook>> {
        let api = self.client.rest().webhooks();
        let request = match &self.token {
            Some(token) if !body.moves_channel() => api.modify_with_token(self.id, token, body),
            _ => api.modify(self.id, body),
        };
        Call::new(self.client.clone(), request)
    }

    /// Deletes the webhook: with its token if known, else with the bot's.
    pub fn delete(&self) -> Call<()> {
        let api = self.client.rest().webhooks();
        let request = match &self.token {
            Some(token) => api.delete_with_token(self.id, token),
            None => api.delete(self.id),
        };
        Call::new(self.client.clone(), request)
    }

    /// Fetches a message this webhook posted.
    pub fn fetch_message(&self, message: MessageId) -> Call<Bound<Message>> {
        match self.token() {
            Ok(token) => {
                Call::new(self.client.clone(), self.client.rest().webhooks().get_message(self.id, token, message))
            }
            Err(e) => Call::failed(self.client.clone(), e),
        }
    }

    /// Edits a message this webhook posted.
    pub fn edit_message(&self, message: MessageId, body: EditMessage) -> Call<Bound<Message>> {
        match self.token() {
            Ok(token) => {
                let request = self.client.rest().webhooks().edit_message(self.id, token, message, body);
                Call::new(self.client.clone(), request)
            }
            Err(e) => Call::failed(self.client.clone(), e),
        }
    }

    /// Deletes a message this webhook posted.
    pub fn delete_message(&self, message: MessageId) -> Call<()> {
        match self.token() {
            Ok(token) => {
                Call::new(self.client.clone(), self.client.rest().webhooks().delete_message(self.id, token, message))
            }
            Err(e) => Call::failed(self.client.clone(), e),
        }
    }
}

impl Bound<Webhook> {
    /// This webhook as a handle, with its token if the value has one.
    pub fn handle(&self) -> WebhookHandle {
        WebhookHandle { client: self.client().clone(), id: self.id, token: self.token.clone() }
    }
}

/// The webhooks of one channel. Get one from [`ChannelHandle::webhooks`].
#[derive(Clone, Debug)]
pub struct WebhooksManager {
    client: Client,
    channel: ChannelId,
}

impl WebhooksManager {
    /// Creates a webhook in this channel. Use `.reason(...)` for the audit log.
    pub fn create(&self, body: CreateWebhook) -> Call<Bound<Webhook>> {
        Call::new(self.client.clone(), self.client.rest().webhooks().create(self.channel, body))
    }

    /// Every webhook of this channel, from Discord.
    pub fn fetch_all(&self) -> Call<Vec<Bound<Webhook>>> {
        Call::new(self.client.clone(), self.client.rest().webhooks().get_channel_webhooks(self.channel))
    }
}

impl ChannelHandle {
    /// The webhooks of this channel.
    pub fn webhooks(&self) -> WebhooksManager {
        WebhooksManager { client: self.client.clone(), channel: self.id }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_webhook_url;

    #[test]
    fn webhook_urls_parse() {
        let parsed = parse_webhook_url("https://discord.com/api/webhooks/123/abc-DEF_9");
        assert_eq!(parsed.map(|(id, token)| (id.get(), token)), Some((123, String::from("abc-DEF_9"))));
        let versioned = parse_webhook_url("https://discordapp.com/api/v10/webhooks/5/t/?wait=true");
        assert_eq!(versioned.map(|(id, _)| id.get()), Some(5));
        assert!(parse_webhook_url("https://discord.com/api/webhooks/123").is_none());
        assert!(parse_webhook_url("https://discord.com/api/channels/1/tok").is_none());
        assert!(parse_webhook_url("https://discord.com/api/webhooks/x/tok").is_none());
        assert!(parse_webhook_url("https://discord.com/api/webhooks/1/a%2Fb").is_none(), "no encoded characters");
    }
}
