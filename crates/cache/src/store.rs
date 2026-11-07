//! `Cache`: the tables and the public reads. The writes are in `apply/`.

use crate::{CacheConfig, Limit, Sharded, Table};
use dr_model::{
    Channel, ChannelId, CurrentUser, DispatchKind, Guild, GuildId, Member, Message, MessageId, Role, RoleId, User,
    UserId,
};
use parking_lot::{Mutex, RwLock};
use std::collections::VecDeque;
use std::sync::Arc;

/// Everything under one guild. A slot can exist with no `meta`, when a member or a role was seen
/// before its guild was.
#[derive(Debug, Default)]
pub(crate) struct GuildSlot {
    pub(crate) meta: RwLock<Option<Arc<Guild>>>,
    pub(crate) channels: Table<ChannelId, Channel>,
    pub(crate) roles: Table<RoleId, Role>,
    pub(crate) members: Table<UserId, Member>,
}

/// The cached messages of one channel, oldest first.
pub(crate) type ChannelMessages = Mutex<VecDeque<Arc<Message>>>;

/// The state of one client.
///
/// Reads return the stored snapshot, and a cold cache returns `None` or an empty list, never an
/// error: the facade decides whether to fetch. The `insert_*_if_absent` methods exist for `resolve`,
/// the one path where a REST result enters the cache. A gateway write always wins over them.
#[derive(Debug)]
pub struct Cache {
    pub(crate) config: CacheConfig,
    pub(crate) me: RwLock<Option<Arc<CurrentUser>>>,
    pub(crate) guilds: Sharded<GuildId, GuildSlot>,
    /// Every channel, of guilds and DMs alike. The guild slot holds the same `Arc`.
    pub(crate) channels: Sharded<ChannelId, Channel>,
    /// DM channels in the order they were stored, to evict the oldest.
    pub(crate) dm_order: Mutex<VecDeque<ChannelId>>,
    pub(crate) users: Sharded<UserId, User>,
    pub(crate) messages: Sharded<ChannelId, ChannelMessages>,
}

impl Cache {
    /// An empty cache using this policy.
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            me: RwLock::new(None),
            guilds: Sharded::new(),
            channels: Sharded::new(),
            dm_order: Mutex::new(VecDeque::new()),
            users: Sharded::new(),
            messages: Sharded::new(),
        }
    }

    /// The policy.
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// Whether this policy stores anything from an event of this kind. Under [`CacheConfig::none`]
    /// only `READY` is.
    pub fn wants(&self, kind: DispatchKind) -> bool {
        match kind {
            DispatchKind::Ready => true,
            DispatchKind::GuildCreate
            | DispatchKind::GuildUpdate
            | DispatchKind::GuildDelete
            | DispatchKind::ChannelCreate
            | DispatchKind::ChannelUpdate
            | DispatchKind::ChannelDelete
            | DispatchKind::GuildMemberAdd
            | DispatchKind::GuildMemberUpdate
            | DispatchKind::GuildMemberRemove
            | DispatchKind::MessageCreate
            | DispatchKind::MessageUpdate
            | DispatchKind::MessageDelete => self.config.is_enabled(),
            _ => false,
        }
    }

    /// The bot's own user, after `READY`.
    pub fn me(&self) -> Option<Arc<CurrentUser>> {
        self.me.read().clone()
    }

    /// A guild. Its `channels`, `members` and `roles` lists are empty in the stored form: read those
    /// through [`Cache::guild_channels`], [`Cache::member`] and [`Cache::guild_roles`].
    pub fn guild(&self, id: GuildId) -> Option<Arc<Guild>> {
        self.guilds.get(&id).and_then(|slot| slot.meta.read().clone())
    }

    /// The ids of every cached guild, in no particular order.
    pub fn guild_ids(&self) -> Vec<GuildId> {
        self.guilds.snapshot().iter().filter_map(|slot| slot.meta.read().as_ref().map(|g| g.id)).collect()
    }

    /// A channel of any guild, or a DM channel.
    pub fn channel(&self, id: ChannelId) -> Option<Arc<Channel>> {
        self.channels.get(&id)
    }

    /// The channels of a guild, in no particular order.
    pub fn guild_channels(&self, guild: GuildId) -> Vec<Arc<Channel>> {
        self.guilds.get(&guild).map_or_else(Vec::new, |slot| slot.channels.snapshot())
    }

    /// A user.
    pub fn user(&self, id: UserId) -> Option<Arc<User>> {
        self.users.get(&id)
    }

    /// A member, with its `user` and `guild_id` filled in.
    pub fn member(&self, guild: GuildId, user: UserId) -> Option<Arc<Member>> {
        self.guilds.get(&guild).and_then(|slot| slot.members.get(&user))
    }

    /// A role.
    pub fn role(&self, guild: GuildId, role: RoleId) -> Option<Arc<Role>> {
        self.guilds.get(&guild).and_then(|slot| slot.roles.get(&role))
    }

    /// The roles of a guild, in no particular order.
    pub fn guild_roles(&self, guild: GuildId) -> Vec<Arc<Role>> {
        self.guilds.get(&guild).map_or_else(Vec::new, |slot| slot.roles.snapshot())
    }

    /// A message, if the policy keeps messages.
    pub fn message(&self, channel: ChannelId, id: MessageId) -> Option<Arc<Message>> {
        let list = self.messages.get(&channel)?;

        list.lock().iter().find(|m| m.id == id).cloned()
    }

    /// The cached messages of a channel, oldest first.
    pub fn channel_messages(&self, channel: ChannelId) -> Vec<Arc<Message>> {
        self.messages.get(&channel).map_or_else(Vec::new, |list| list.lock().iter().cloned().collect())
    }

    /// Stores a fetched guild unless one is there.
    pub fn insert_guild_if_absent(&self, guild: Arc<Guild>) {
        if !self.config.is_enabled() {
            return;
        }
        let slot = self.slot(guild.id);
        let mut meta = slot.meta.write();
        if meta.is_none() {
            *meta = Some(guild);
        }
    }

    /// Stores a fetched channel unless one is there.
    pub fn insert_channel_if_absent(&self, channel: Arc<Channel>) {
        if !self.config.is_enabled() || self.channels.get(&channel.id).is_some() {
            return;
        }
        self.store_channel(channel);
    }

    /// Stores a fetched user unless one is there.
    pub fn insert_user_if_absent(&self, user: Arc<User>) {
        if self.config.is_enabled() {
            self.users.insert_if_absent(user.id, user);
        }
    }

    /// Stores a fetched member unless one is there. A member with no `user` is ignored.
    pub fn insert_member_if_absent(&self, guild: GuildId, member: Arc<Member>) {
        if !self.config.is_enabled() {
            return;
        }
        let Some(id) = member.user.as_ref().map(|u| u.id) else { return };
        let slot = self.slot(guild);
        if self.keeps_member(&slot, id) {
            slot.members.insert_if_absent(id, member);
        }
    }

    /// Stores a fetched role unless one is there.
    pub fn insert_role_if_absent(&self, guild: GuildId, role: Arc<Role>) {
        if self.config.is_enabled() {
            self.slot(guild).roles.insert_if_absent(role.id, role);
        }
    }

    /// The slot of a guild, created when missing.
    pub(crate) fn slot(&self, guild: GuildId) -> Arc<GuildSlot> {
        if let Some(slot) = self.guilds.get(&guild) {
            return slot;
        }
        let slot = Arc::new(GuildSlot::default());
        self.guilds.insert_if_absent(guild, slot.clone());
        self.guilds.get(&guild).unwrap_or(slot)
    }

    /// Whether the policy keeps this member. The bot's own member is always kept.
    pub(crate) fn keeps_member(&self, slot: &GuildSlot, user: UserId) -> bool {
        let is_me = self.me.read().as_ref().is_some_and(|me| me.id == user);
        match self.config.members_limit() {
            _ if is_me => true,
            Limit::None => false,
            Limit::PerGuild(n) => {
                let limit = usize::try_from(n).unwrap_or(usize::MAX);
                slot.members.get(&user).is_some() || slot.members.len() < limit
            }
            _ => true,
        }
    }

    /// Stores a channel in the global table, and in its guild's table or the DM order. Returns the
    /// old one.
    pub(crate) fn store_channel(&self, channel: Arc<Channel>) -> Option<Arc<Channel>> {
        let id = channel.id;
        if let Some(guild) = channel.guild_id {
            self.slot(guild).channels.insert(id, channel.clone());
        } else {
            let limit = usize::try_from(self.config.dm_channels_limit()).unwrap_or(usize::MAX);
            if limit == 0 {
                return None;
            }
            let mut order = self.dm_order.lock();
            if !order.contains(&id) {
                order.push_back(id);
            }
            while order.len() > limit {
                if let Some(oldest) = order.pop_front() {
                    self.channels.remove(&oldest);
                    self.messages.remove(&oldest);
                }
            }
        }
        self.channels.insert(id, channel)
    }
}
