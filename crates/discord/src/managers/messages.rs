//! The messages of one channel.

use crate::{Bound, Call, Client, Error};
use dr_builders::BulkDeleteMessages;
use dr_model::{ChannelId, Message, MessageId};
use dr_rest::api::MessagePage;

/// The messages of one channel. Get one from `ChannelHandle::messages`.
#[derive(Clone, Debug)]
pub struct MessagesManager {
    client: Client,
    channel: ChannelId,
}

impl MessagesManager {
    pub(crate) fn new(client: Client, channel: ChannelId) -> Self {
        Self { client, channel }
    }

    /// The message from the cache, if the policy keeps messages. No I/O.
    pub fn get(&self, id: MessageId) -> Option<Bound<Message>> {
        self.client.cache().message(self.channel, id).map(|m| self.client.bind_arc(m))
    }

    /// The message from Discord.
    pub fn fetch(&self, id: MessageId) -> Call<Bound<Message>> {
        Call::new(self.client.clone(), self.client.rest().channels().get_message(self.channel, id))
    }

    /// One page of messages from Discord, newest first.
    pub fn list(&self, page: MessagePage) -> Call<Vec<Bound<Message>>> {
        Call::new(self.client.clone(), self.client.rest().channels().get_messages(self.channel, page))
    }

    /// Deletes 2 to 100 messages at once. Discord refuses messages older than 14 days. Use
    /// `.reason(...)` for the audit log.
    pub fn bulk_delete(&self, messages: impl IntoIterator<Item = MessageId>) -> Call<()> {
        let body = BulkDeleteMessages::new(messages);
        Call::new(self.client.clone(), self.client.rest().channels().bulk_delete_messages(self.channel, body))
    }

    /// The message from the cache, else from Discord. A fetched message does not enter the cache.
    pub async fn resolve(&self, id: MessageId) -> Result<Bound<Message>, Error> {
        if let Some(message) = self.get(id) {
            return Ok(message);
        }
        self.fetch(id).await
    }
}
