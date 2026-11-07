//! Bit sets over `u64`, keeping unknown bits, `ARCHITECTURE.md` 7.6. Checked bit by bit against
//! discord-api-types. A set used by one payload lives in that payload's file, such as
//! `RoleFlags` in `payloads/guild.rs`. Sets that belong to a feature not ported yet do
//! not exist.
//!
//! A deprecated upstream name sits right after the current name of the same bit, so `Debug` prints
//! the current one.
//!
//! Derived from discord-api-types 00cbe03 (MIT, (c) 2020 vladfrangu).
//! Changed: translated to `open_flags!`, and `u64` everywhere fixes upstream's 32-bit truncation.

use dr_core::open_flags;

open_flags! {
    /// Gateway intents. Upstream `GatewayIntentBits`.
    Intents: number {
        GUILDS = 1 << 0,
        GUILD_MEMBERS = 1 << 1,
        GUILD_MODERATION = 1 << 2,
        /// Deprecated upstream. This is the old name for `GUILD_MODERATION`
        GUILD_BANS = 1 << 2,
        GUILD_EXPRESSIONS = 1 << 3,
        /// Deprecated upstream. This is the old name for `GUILD_EXPRESSIONS`
        GUILD_EMOJIS_AND_STICKERS = 1 << 3,
        GUILD_INTEGRATIONS = 1 << 4,
        GUILD_WEBHOOKS = 1 << 5,
        GUILD_INVITES = 1 << 6,
        GUILD_VOICE_STATES = 1 << 7,
        GUILD_PRESENCES = 1 << 8,
        GUILD_MESSAGES = 1 << 9,
        GUILD_MESSAGE_REACTIONS = 1 << 10,
        GUILD_MESSAGE_TYPING = 1 << 11,
        DIRECT_MESSAGES = 1 << 12,
        DIRECT_MESSAGE_REACTIONS = 1 << 13,
        DIRECT_MESSAGE_TYPING = 1 << 14,
        MESSAGE_CONTENT = 1 << 15,
        GUILD_SCHEDULED_EVENTS = 1 << 16,
        AUTO_MODERATION_CONFIGURATION = 1 << 20,
        AUTO_MODERATION_EXECUTION = 1 << 21,
        GUILD_MESSAGE_POLLS = 1 << 24,
        DIRECT_MESSAGE_POLLS = 1 << 25,
    }
}

open_flags! {
    /// Client capabilities sent in an identify.
    GatewayCapabilityBits: number {
        /// Opts the client into receiving obfuscated channel metadata over the Gateway for channels it can't
        /// view
        ///
        /// Unstable upstream. `CHANNEL_OBFUSCATION` is a temporary, testing-only opt-in for channel
        /// obfuscation. This opt-in mechanism will change before the feature reaches general availability.
        /// Obfuscation is then planned to apply to all bots automatically, even when they don't provide this
        /// capability.
        CHANNEL_OBFUSCATION = 1 << 15,
    }
}

open_flags! {
    /// Permissions, written as a decimal string. Upstream `PermissionFlagsBits`.
    Permissions: string {
        /// Allows creation of instant invites
        ///
        /// Applies to channel types: Text, Voice, Stage
        CREATE_INSTANT_INVITE = 1 << 0,
        /// Allows kicking members
        KICK_MEMBERS = 1 << 1,
        /// Allows banning members
        BAN_MEMBERS = 1 << 2,
        /// Allows all permissions and bypasses channel permission overwrites
        ADMINISTRATOR = 1 << 3,
        /// Allows management and editing of channels
        ///
        /// Applies to channel types: Text, Voice, Stage
        MANAGE_CHANNELS = 1 << 4,
        /// Allows management and editing of the guild
        MANAGE_GUILD = 1 << 5,
        /// Allows for the addition of reactions to messages. This permission does not apply to reacting with
        /// an existing reaction on a message
        ///
        /// Applies to channel types: Text, Voice, Stage
        ADD_REACTIONS = 1 << 6,
        /// Allows for viewing of audit logs
        VIEW_AUDIT_LOG = 1 << 7,
        /// Allows for using priority speaker in a voice channel
        ///
        /// Applies to channel types: Voice
        PRIORITY_SPEAKER = 1 << 8,
        /// Allows the user to go live
        ///
        /// Applies to channel types: Voice, Stage
        STREAM = 1 << 9,
        /// Allows guild members to view a channel, which includes reading messages in text channels and
        /// joining voice channels
        ///
        /// Applies to channel types: Text, Voice, Stage
        VIEW_CHANNEL = 1 << 10,
        /// Allows for sending messages in a channel and creating threads in a forum (does not allow sending
        /// messages in threads)
        ///
        /// Applies to channel types: Text, Voice, Stage
        SEND_MESSAGES = 1 << 11,
        /// Allows for sending of `/tts` messages
        ///
        /// Applies to channel types: Text, Voice, Stage
        SEND_TTS_MESSAGES = 1 << 12,
        /// Allows for deletion of other users messages
        ///
        /// Applies to channel types: Text, Voice, Stage
        MANAGE_MESSAGES = 1 << 13,
        /// Links sent by users with this permission will be auto-embedded
        ///
        /// Applies to channel types: Text, Voice, Stage
        EMBED_LINKS = 1 << 14,
        /// Allows for uploading images and files
        ///
        /// Applies to channel types: Text, Voice, Stage
        ATTACH_FILES = 1 << 15,
        /// Allows for reading of message history
        ///
        /// Applies to channel types: Text, Voice, Stage
        READ_MESSAGE_HISTORY = 1 << 16,
        /// Allows for using the `@everyone` tag to notify all users in a channel, and the `@here` tag to
        /// notify all online users in a channel
        ///
        /// Applies to channel types: Text, Voice, Stage
        MENTION_EVERYONE = 1 << 17,
        /// Allows the usage of custom emojis from other servers
        ///
        /// Applies to channel types: Text, Voice, Stage
        USE_EXTERNAL_EMOJIS = 1 << 18,
        /// Allows for viewing guild insights
        VIEW_GUILD_INSIGHTS = 1 << 19,
        /// Allows for joining of a voice channel
        ///
        /// Applies to channel types: Voice, Stage
        CONNECT = 1 << 20,
        /// Allows for speaking in a voice channel
        ///
        /// Applies to channel types: Voice
        SPEAK = 1 << 21,
        /// Allows for muting members in a voice channel
        ///
        /// Applies to channel types: Voice, Stage
        MUTE_MEMBERS = 1 << 22,
        /// Allows for deafening of members in a voice channel
        ///
        /// Applies to channel types: Voice
        DEAFEN_MEMBERS = 1 << 23,
        /// Allows for moving of members between voice channels
        ///
        /// Applies to channel types: Voice, Stage
        MOVE_MEMBERS = 1 << 24,
        /// Allows for using voice-activity-detection in a voice channel
        ///
        /// Applies to channel types: Voice
        USE_VAD = 1 << 25,
        /// Allows for modification of own nickname
        CHANGE_NICKNAME = 1 << 26,
        /// Allows for modification of other users nicknames
        MANAGE_NICKNAMES = 1 << 27,
        /// Allows management and editing of roles
        ///
        /// Applies to channel types: Text, Voice, Stage
        MANAGE_ROLES = 1 << 28,
        /// Allows management and editing of webhooks
        ///
        /// Applies to channel types: Text, Voice, Stage
        MANAGE_WEBHOOKS = 1 << 29,
        /// Allows for editing and deleting emojis, stickers, and soundboard sounds created by all users
        MANAGE_GUILD_EXPRESSIONS = 1 << 30,
        /// Allows management and editing of emojis, stickers, and soundboard sounds
        ///
        /// Deprecated upstream. This is the old name for `MANAGE_GUILD_EXPRESSIONS`
        MANAGE_EMOJIS_AND_STICKERS = 1 << 30,
        /// Allows members to use application commands, including slash commands and context menu commands
        ///
        /// Applies to channel types: Text, Voice, Stage
        USE_APPLICATION_COMMANDS = 1 << 31,
        /// Allows for requesting to speak in stage channels
        ///
        /// Applies to channel types: Stage
        REQUEST_TO_SPEAK = 1 << 32,
        /// Allows for editing and deleting scheduled events created by all users
        ///
        /// Applies to channel types: Voice, Stage
        MANAGE_EVENTS = 1 << 33,
        /// Allows for deleting and archiving threads, and viewing all private threads
        ///
        /// Applies to channel types: Text
        MANAGE_THREADS = 1 << 34,
        /// Allows for creating public and announcement threads
        ///
        /// Applies to channel types: Text
        CREATE_PUBLIC_THREADS = 1 << 35,
        /// Allows for creating private threads
        ///
        /// Applies to channel types: Text
        CREATE_PRIVATE_THREADS = 1 << 36,
        /// Allows the usage of custom stickers from other servers
        ///
        /// Applies to channel types: Text, Voice, Stage
        USE_EXTERNAL_STICKERS = 1 << 37,
        /// Allows for sending messages in threads
        ///
        /// Applies to channel types: Text
        SEND_MESSAGES_IN_THREADS = 1 << 38,
        /// Allows for using Activities (applications with the `EMBEDDED` flag)
        ///
        /// Applies to channel types: Text, Voice
        USE_EMBEDDED_ACTIVITIES = 1 << 39,
        /// Allows for timing out users to prevent them from sending or reacting to messages in chat and
        /// threads, and from speaking in voice and stage channels
        MODERATE_MEMBERS = 1 << 40,
        /// Allows for viewing role subscription insights
        VIEW_CREATOR_MONETIZATION_ANALYTICS = 1 << 41,
        /// Allows for using soundboard in a voice channel
        ///
        /// Applies to channel types: Voice
        USE_SOUNDBOARD = 1 << 42,
        /// Allows for creating emojis, stickers, and soundboard sounds, and editing and deleting those
        /// created by the current user
        CREATE_GUILD_EXPRESSIONS = 1 << 43,
        /// Allows for creating scheduled events, and editing and deleting those created by the current user
        ///
        /// Applies to channel types: Voice, Stage
        CREATE_EVENTS = 1 << 44,
        /// Allows the usage of custom soundboard sounds from other servers
        ///
        /// Applies to channel types: Voice
        USE_EXTERNAL_SOUNDS = 1 << 45,
        /// Allows sending voice messages
        ///
        /// Applies to channel types: Text, Voice, Stage
        SEND_VOICE_MESSAGES = 1 << 46,
        /// Allows setting voice channel status
        ///
        /// Applies to channel types: Voice
        SET_VOICE_CHANNEL_STATUS = 1 << 48,
        /// Allows sending polls
        ///
        /// Applies to channel types: Text, Voice, Stage
        SEND_POLLS = 1 << 49,
        /// Allows user-installed apps to send public responses. When disabled, users will still be allowed to
        /// use their apps but the responses will be ephemeral. This only applies to apps not also installed
        /// to the server
        ///
        /// Applies to channel types: Text, Voice, Stage
        USE_EXTERNAL_APPS = 1 << 50,
        /// Allows pinning and unpinning messages
        ///
        /// Applies to channel types: Text
        PIN_MESSAGES = 1 << 51,
        /// Allows bypassing slowmode restrictions
        ///
        /// Applies to channel types: Text, Voice, Stage
        BYPASS_SLOWMODE = 1 << 52,
    }
}

open_flags! {
    /// Message flags.
    MessageFlags: number {
        /// This message has been published to subscribed channels (via Channel Following)
        CROSSPOSTED = 1 << 0,
        /// This message originated from a message in another channel (via Channel Following)
        IS_CROSSPOST = 1 << 1,
        /// Do not include any embeds when serializing this message
        SUPPRESS_EMBEDS = 1 << 2,
        /// The source message for this crosspost has been deleted (via Channel Following)
        SOURCE_MESSAGE_DELETED = 1 << 3,
        /// This message came from the urgent message system
        URGENT = 1 << 4,
        /// This message has an associated thread, which shares its id
        HAS_THREAD = 1 << 5,
        /// This message is only visible to the user who invoked the Interaction
        EPHEMERAL = 1 << 6,
        /// This message is an Interaction Response and the bot is "thinking"
        LOADING = 1 << 7,
        /// This message failed to mention some roles and add their members to the thread
        FAILED_TO_MENTION_SOME_ROLES_IN_THREAD = 1 << 8,
        /// Unstable upstream. This message flag is currently not documented by Discord but has a known value
        /// which we will try to keep up to date.
        SHOULD_SHOW_LINK_NOT_DISCORD_WARNING = 1 << 10,
        /// This message will not trigger push and desktop notifications
        SUPPRESS_NOTIFICATIONS = 1 << 12,
        /// This message is a voice message
        IS_VOICE_MESSAGE = 1 << 13,
        /// This message has a snapshot (via Message Forwarding)
        HAS_SNAPSHOT = 1 << 14,
        /// Allows you to create fully component-driven messages
        IS_COMPONENTS_V2 = 1 << 15,
    }
}

open_flags! {
    /// Flags on a user. Some bits are above `2^31`, which `u64` keeps.
    UserFlags: number {
        /// Discord Employee
        STAFF = 1 << 0,
        /// Partnered Server Owner
        PARTNER = 1 << 1,
        /// HypeSquad Events Member
        HYPESQUAD = 1 << 2,
        /// Bug Hunter Level 1
        BUG_HUNTER_LEVEL1 = 1 << 3,
        /// Unstable upstream. This user flag is currently not documented by Discord but has a known value
        /// which we will try to keep up to date.
        MFA_SMS = 1 << 4,
        /// Unstable upstream. This user flag is currently not documented by Discord but has a known value
        /// which we will try to keep up to date.
        PREMIUM_PROMO_DISMISSED = 1 << 5,
        /// House Bravery Member
        HYPE_SQUAD_ONLINE_HOUSE1 = 1 << 6,
        /// House Brilliance Member
        HYPE_SQUAD_ONLINE_HOUSE2 = 1 << 7,
        /// House Balance Member
        HYPE_SQUAD_ONLINE_HOUSE3 = 1 << 8,
        /// Early Nitro Supporter
        PREMIUM_EARLY_SUPPORTER = 1 << 9,
        /// User is a team
        TEAM_PSEUDO_USER = 1 << 10,
        /// Unstable upstream. This user flag is currently not documented by Discord but has a known value
        /// which we will try to keep up to date.
        HAS_UNREAD_URGENT_MESSAGES = 1 << 13,
        /// Bug Hunter Level 2
        BUG_HUNTER_LEVEL2 = 1 << 14,
        /// Verified Bot
        VERIFIED_BOT = 1 << 16,
        /// Early Verified Bot Developer
        VERIFIED_DEVELOPER = 1 << 17,
        /// Moderator Programs Alumni
        CERTIFIED_MODERATOR = 1 << 18,
        /// Bot uses only HTTP interactions and is shown in the online member list
        BOT_HTTP_INTERACTIONS = 1 << 19,
        /// User has been identified as spammer
        ///
        /// Unstable upstream. This user flag is currently not documented by Discord but has a known value
        /// which we will try to keep up to date.
        SPAMMER = 1 << 20,
        /// Unstable upstream. This user flag is currently not documented by Discord but has a known value
        /// which we will try to keep up to date.
        DISABLE_PREMIUM = 1 << 21,
        /// User is an Active Developer
        ///
        /// Deprecated upstream. This user flag is no longer available. See
        /// <https://support-dev.discord.com/hc/articles/10113997751447-Active-Developer-Badge> for more
        /// information.
        ACTIVE_DEVELOPER = 1 << 22,
        /// User's account has been quarantined based on recent activity
        ///
        /// Unstable upstream. This user flag is currently not documented by Discord but has a known value
        /// which we will try to keep up to date.
        QUARANTINED = 1 << 44,
        /// Unstable upstream. This user flag is currently not documented by Discord but has a known value
        /// which we will try to keep up to date.
        COLLABORATOR = 1 << 50,
        /// Unstable upstream. This user flag is currently not documented by Discord but has a known value
        /// which we will try to keep up to date.
        RESTRICTED_COLLABORATOR = 1 << 51,
    }
}

open_flags! {
    /// Flags on an application.
    ApplicationFlags: number {
        /// Unstable upstream. This application flag is currently not documented by Discord but has a known
        /// value which we will try to keep up to date.
        EMBEDDED_RELEASED = 1 << 1,
        /// Unstable upstream. This application flag is currently not documented by Discord but has a known
        /// value which we will try to keep up to date.
        MANAGED_EMOJI = 1 << 2,
        /// Unstable upstream. This application flag is currently not documented by Discord but has a known
        /// value which we will try to keep up to date.
        EMBEDDED_IAP = 1 << 3,
        /// Unstable upstream. This application flag is currently not documented by Discord but has a known
        /// value which we will try to keep up to date.
        GROUP_DM_CREATE = 1 << 4,
        /// Indicates if an app uses the Auto Moderation API
        APPLICATION_AUTO_MODERATION_RULE_CREATE_BADGE = 1 << 6,
        /// Unstable upstream. This application flag is currently not documented by Discord but has a known
        /// value which we will try to keep up to date.
        RPC_HAS_CONNECTED = 1 << 11,
        /// Intent required for bots in 100 or more servers to receive `presence_update` events
        GATEWAY_PRESENCE = 1 << 12,
        /// Intent required for bots in under 100 servers to receive `presence_update` events, found in Bot
        /// Settings
        GATEWAY_PRESENCE_LIMITED = 1 << 13,
        /// Intent required for bots in 100 or more servers to receive member-related events like
        /// `guild_member_add`.
        GATEWAY_GUILD_MEMBERS = 1 << 14,
        /// Intent required for bots in under 100 servers to receive member-related events like
        /// `guild_member_add`, found in Bot Settings.
        GATEWAY_GUILD_MEMBERS_LIMITED = 1 << 15,
        /// Indicates unusual growth of an app that prevents verification
        VERIFICATION_PENDING_GUILD_LIMIT = 1 << 16,
        /// Indicates if an app is embedded within the Discord client (currently unavailable publicly)
        EMBEDDED = 1 << 17,
        /// Intent required for bots in 100 or more servers to receive message content
        GATEWAY_MESSAGE_CONTENT = 1 << 18,
        /// Intent required for bots in under 100 servers to receive message content, found in Bot Settings
        GATEWAY_MESSAGE_CONTENT_LIMITED = 1 << 19,
        /// Unstable upstream. This application flag is currently not documented by Discord but has a known
        /// value which we will try to keep up to date.
        EMBEDDED_FIRST_PARTY = 1 << 20,
        /// Indicates if an app has registered global application commands
        APPLICATION_COMMAND_BADGE = 1 << 23,
    }
}
