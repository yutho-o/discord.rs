//! `ChannelHandle`.

use crate::handles::SendMessageRequest;
use crate::managers::MessagesManager;
use crate::{Bound, Call, Client};
use dr_builders::CreateMessage;
use dr_model::{Channel, ChannelId};

/// A channel by id: a client and an id, no I/O to create.
#[derive(Clone, Debug)]
pub struct ChannelHandle {
    pub(crate) client: Client,
    pub(crate) id: ChannelId,
}

impl Client {
    /// A handle to a channel. No I/O.
    pub fn channel(&self, id: ChannelId) -> ChannelHandle {
        ChannelHandle { client: self.clone(), id }
    }
}

impl ChannelHandle {
    /// The id.
    pub fn id(&self) -> ChannelId {
        self.id
    }

    /// Sends a message.
    pub fn send(&self, body: impl Into<CreateMessage>) -> SendMessageRequest {
        SendMessageRequest::new(self.client.clone(), self.id, body.into())
    }

    /// Fetches the channel from Discord.
    pub fn fetch(&self) -> Call<Bound<Channel>> {
        Call::new(self.client.clone(), self.client.rest().channels().get(self.id))
    }

    /// The channel from the cache. No I/O.
    pub fn cached(&self) -> Option<Bound<Channel>> {
        self.client.cache().channel(self.id).map(|c| self.client.bind_arc(c))
    }

    /// Shows the bot as typing, for about 10 s.
    pub fn typing(&self) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().channels().show_typing(self.id))
    }

    /// The messages of this channel.
    pub fn messages(&self) -> MessagesManager {
        MessagesManager::new(self.client.clone(), self.id)
    }
}
