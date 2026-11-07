//! The messages of one channel.

use crate::{Bound, Call, Client, Error};
use dr_model::{ChannelId, Message, MessageId};

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

    /// The message from the cache, else from Discord. A fetched message does not enter the cache.
    pub async fn resolve(&self, id: MessageId) -> Result<Bound<Message>, Error> {
        if let Some(message) = self.get(id) {
            return Ok(message);
        }
        self.fetch(id).await
    }
}
