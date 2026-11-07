//! What the cache stores, `ARCHITECTURE.md` 10.3.

/// How many of something to keep.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Limit {
    /// Keep none, apart from what must always be kept.
    None,
    /// Keep all that are seen. The default.
    #[default]
    Unlimited,
    /// Keep up to this many per guild. Eviction is arbitrary.
    PerGuild(u32),
}

/// What the cache stores. The presets are [`CacheConfig::standard`] and [`CacheConfig::none`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheConfig {
    enabled: bool,
    messages_per_channel: u32,
    members: Limit,
    dm_channels: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self::standard()
    }
}

impl CacheConfig {
    /// Stores nothing but the bot's own user. Every event still fires, built from its payload.
    pub fn none() -> Self {
        Self { enabled: false, messages_per_channel: 0, members: Limit::None, dm_channels: 0 }
    }
    /// Guilds, channels and roles, users, the members seen in events, and up to 1000 DM channels.
    /// No messages. The default.
    pub fn standard() -> Self {
        Self { enabled: true, messages_per_channel: 0, members: Limit::Unlimited, dm_channels: 1000 }
    }
    /// Sets the messages kept per channel. 0 keeps none.
    pub fn messages_per_channel(mut self, n: u32) -> Self {
        self.messages_per_channel = n;
        self
    }
    /// Sets the members kept. The bot's own member is always kept.
    pub fn members(mut self, limit: Limit) -> Self {
        self.members = limit;
        self
    }
    /// Sets the DM channels kept.
    pub fn dm_channels(mut self, n: u32) -> Self {
        self.dm_channels = n;
        self
    }
    /// True unless this is [`CacheConfig::none`].
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    /// The messages kept per channel.
    pub fn messages_limit(&self) -> u32 {
        self.messages_per_channel
    }
    /// The members kept.
    pub fn members_limit(&self) -> Limit {
        self.members
    }
    /// The DM channels kept.
    pub fn dm_channels_limit(&self) -> u32 {
        self.dm_channels
    }
}
