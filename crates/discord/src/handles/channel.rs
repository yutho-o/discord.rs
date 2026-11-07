//! `ChannelHandle`.

use crate::handles::SendMessageRequest;
use crate::managers::MessagesManager;
use crate::{Bound, Call, Client};
use dr_builders::{CreateMessage, EditChannel};
use dr_model::{Channel, ChannelId, GenericId, PermissionOverwrite, Permissions, UserId};

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

    /// Edits the channel. Use `.reason(...)` for the audit log.
    pub fn edit(&self, body: EditChannel) -> Call<Bound<Channel>> {
        Call::new(self.client.clone(), self.client.rest().channels().modify(self.id, body))
    }

    /// Deletes the channel, or closes a DM, and gives it back as it was. Use `.reason(...)` for the
    /// audit log.
    pub fn delete(&self) -> Call<Bound<Channel>> {
        Call::new(self.client.clone(), self.client.rest().channels().delete(self.id))
    }

    /// Creates or replaces the overwrite of one role or member, leaving the others alone. Use
    /// `.reason(...)` for the audit log.
    pub fn set_permission(&self, overwrite: PermissionOverwrite) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().channels().edit_permission(self.id, &overwrite))
    }

    /// Removes the overwrite of a role or a member: pass its id, `role.cast()` or `user.cast()`.
    pub fn remove_permission(&self, target: GenericId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().channels().delete_permission(self.id, target))
    }

    /// What `user` may do in this channel, computed from the cache: `None` unless the channel, its
    /// guild and the member are cached. A thread uses its parent's overwrites.
    pub fn permissions_for(&self, user: UserId) -> Option<Permissions> {
        self.client.cache().channel_permissions(self.id, user)
    }

    /// What the bot may do in this channel, computed from the cache. `None` before `READY` or when
    /// the channel is not cached.
    pub fn my_permissions(&self) -> Option<Permissions> {
        let me = self.client.cache().me()?;
        self.permissions_for(me.id)
    }
}

impl Bound<Channel> {
    /// This channel, as a handle.
    pub fn handle(&self) -> ChannelHandle {
        self.client().channel(self.id)
    }
}
