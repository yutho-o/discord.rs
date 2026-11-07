//! `ctx.channels()`.

use crate::{Bound, Call, Client, Error};
use dr_model::{Channel, ChannelId};

/// Channels. Get one from [`Client::channels`].
#[derive(Clone, Debug)]
pub struct ChannelsManager {
    client: Client,
}

impl Client {
    /// Channels, by id.
    pub fn channels(&self) -> ChannelsManager {
        ChannelsManager { client: self.clone() }
    }
}

impl ChannelsManager {
    /// The channel from the cache. No I/O.
    pub fn get(&self, id: ChannelId) -> Option<Bound<Channel>> {
        self.client.cache().channel(id).map(|c| self.client.bind_arc(c))
    }

    /// The channel from Discord.
    pub fn fetch(&self, id: ChannelId) -> Call<Bound<Channel>> {
        Call::new(self.client.clone(), self.client.rest().channels().get(id))
    }

    /// The channel from the cache, else from Discord.
    pub async fn resolve(&self, id: ChannelId) -> Result<Bound<Channel>, Error> {
        if let Some(channel) = self.get(id) {
            return Ok(channel);
        }
        let fetched = self.fetch(id).await?;
        self.client.cache().insert_channel_if_absent(fetched.data().clone());
        Ok(fetched)
    }
}
