//! The threads of a channel, and the members of a thread.

use crate::handles::ChannelHandle;
use crate::{Bound, Call, Client};
use dr_builders::{CreateForumPost, CreateThread};
use dr_model::{Channel, ChannelId, ThreadList, ThreadMember, UserId};
use dr_rest::api::{ArchivedThreadPage, ThreadMemberPage};

/// The threads of one channel. Get one from [`ChannelHandle::threads`].
#[derive(Clone, Debug)]
pub struct ThreadsManager {
    client: Client,
    channel: ChannelId,
}

impl ThreadsManager {
    /// Starts a thread with no message. Use `.reason(...)` for the audit log.
    pub fn create(&self, body: CreateThread) -> Call<Bound<Channel>> {
        Call::new(self.client.clone(), self.client.rest().channels().start_thread(self.channel, body))
    }

    /// Creates a post in a forum or media channel, with its first message and its files.
    pub fn create_post(&self, body: CreateForumPost) -> Call<Bound<Channel>> {
        Call::new(self.client.clone(), self.client.rest().channels().start_thread(self.channel, body))
    }

    /// Archived public threads, most recently archived first.
    pub fn archived(&self, page: ArchivedThreadPage) -> Call<Bound<ThreadList>> {
        Call::new(self.client.clone(), self.client.rest().channels().archived_threads(self.channel, false, page))
    }

    /// Archived private threads, which needs `MANAGE_THREADS`.
    pub fn archived_private(&self, page: ArchivedThreadPage) -> Call<Bound<ThreadList>> {
        Call::new(self.client.clone(), self.client.rest().channels().archived_threads(self.channel, true, page))
    }

    /// Archived private threads that the bot joined.
    pub fn joined_archived(&self, page: ArchivedThreadPage) -> Call<Bound<ThreadList>> {
        Call::new(self.client.clone(), self.client.rest().channels().joined_archived_threads(self.channel, page))
    }

    /// The cached threads of this channel, in no particular order. No I/O.
    pub fn cached(&self) -> Vec<Bound<Channel>> {
        let Some(parent) = self.client.cache().channel(self.channel) else { return Vec::new() };
        let Some(guild) = parent.guild_id else { return Vec::new() };
        let threads = self.client.cache().guild_channels(guild).into_iter();
        threads
            .filter(|c| c.is_thread() && c.parent_id == Some(self.channel))
            .map(|c| self.client.bind_arc(c))
            .collect()
    }
}

/// The members of one thread. Get one from [`ChannelHandle::thread_members`].
#[derive(Clone, Debug)]
pub struct ThreadMembersManager {
    client: Client,
    thread: ChannelId,
}

impl ThreadMembersManager {
    /// The bot joins the thread.
    pub fn join(&self) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().channels().join_thread(self.thread))
    }

    /// The bot leaves the thread.
    pub fn leave(&self) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().channels().leave_thread(self.thread))
    }

    /// Adds someone to the thread.
    pub fn add(&self, user: UserId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().channels().add_thread_member(self.thread, user))
    }

    /// Removes someone from the thread.
    pub fn remove(&self, user: UserId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().channels().remove_thread_member(self.thread, user))
    }

    /// One member of the thread, from Discord.
    pub fn fetch(&self, user: UserId) -> Call<Bound<ThreadMember>> {
        Call::new(self.client.clone(), self.client.rest().channels().get_thread_member(self.thread, user))
    }

    /// A page of the members of the thread, from Discord. Needs the `GUILD_MEMBERS` intent.
    pub fn list(&self, page: ThreadMemberPage) -> Call<Vec<Bound<ThreadMember>>> {
        Call::new(self.client.clone(), self.client.rest().channels().get_thread_members(self.thread, page))
    }
}

impl ChannelHandle {
    /// The threads of this channel.
    pub fn threads(&self) -> ThreadsManager {
        ThreadsManager { client: self.client.clone(), channel: self.id }
    }

    /// The members of this thread.
    pub fn thread_members(&self) -> ThreadMembersManager {
        ThreadMembersManager { client: self.client.clone(), thread: self.id }
    }
}
