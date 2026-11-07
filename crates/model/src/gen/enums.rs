//! Open enums, checked value by value against discord-api-types. Hand-written for the proof of concept, in
//! the layout the generator fills later. Lists follow the upstream order. An enum used by one payload
//! lives in that payload's file, such as `GuildMFALevel` in `payloads/guild.rs`. Enums that belong to
//! a feature cut from the proof of concept do not exist.
//!
//! Derived from discord-api-types 00cbe03 (MIT, (c) 2020 vladfrangu).
//! Changed: translated to `open_int!` and `open_str!`. Names follow `ARCHITECTURE.md` 7.1.

use dr_core::{open_int, open_str};

open_int! {
    /// Channel kinds. Unknown values are preserved.
    ChannelType: u16 {
        /// A text channel within a guild
        GUILD_TEXT = 0,
        /// A direct message between users
        DM = 1,
        /// A voice channel within a guild
        GUILD_VOICE = 2,
        /// A direct message between multiple users
        GROUP_DM = 3,
        /// An organizational category that contains up to 50 channels
        GUILD_CATEGORY = 4,
        /// A channel that users can follow and crosspost into their own guild
        GUILD_ANNOUNCEMENT = 5,
        /// A temporary sub-channel within a Guild Announcement channel
        ANNOUNCEMENT_THREAD = 10,
        /// A temporary sub-channel within a Guild Text or Guild Forum channel
        PUBLIC_THREAD = 11,
        /// A temporary sub-channel within a Guild Text channel that is only viewable by those invited and
        /// those with the Manage Threads permission
        PRIVATE_THREAD = 12,
        /// A voice channel for hosting events with an audience
        GUILD_STAGE_VOICE = 13,
        /// The channel in a Student Hub containing the listed servers
        GUILD_DIRECTORY = 14,
        /// A channel that can only contain threads
        GUILD_FORUM = 15,
        /// A channel like forum channels but contains media for server subscriptions
        GUILD_MEDIA = 16;
        /// A channel that users can follow and crosspost into their own guild
        ///
        /// Deprecated upstream. This is the old name for `GUILD_ANNOUNCEMENT`
        alias GUILD_NEWS = GUILD_ANNOUNCEMENT;
        /// A temporary sub-channel within a Guild Announcement channel
        ///
        /// Deprecated upstream. This is the old name for `ANNOUNCEMENT_THREAD`
        alias GUILD_NEWS_THREAD = ANNOUNCEMENT_THREAD;
        /// A temporary sub-channel within a Guild Text channel
        ///
        /// Deprecated upstream. This is the old name for `PUBLIC_THREAD`
        alias GUILD_PUBLIC_THREAD = PUBLIC_THREAD;
        /// A temporary sub-channel within a Guild Text channel that is only viewable by those invited and
        /// those with the Manage Threads permission
        ///
        /// Deprecated upstream. This is the old name for `PRIVATE_THREAD`
        alias GUILD_PRIVATE_THREAD = PRIVATE_THREAD;
    }
}

open_int! {
    /// Message kinds.
    MessageType: u16 {
        DEFAULT = 0,
        RECIPIENT_ADD = 1,
        RECIPIENT_REMOVE = 2,
        CALL = 3,
        CHANNEL_NAME_CHANGE = 4,
        CHANNEL_ICON_CHANGE = 5,
        CHANNEL_PINNED_MESSAGE = 6,
        USER_JOIN = 7,
        GUILD_BOOST = 8,
        GUILD_BOOST_TIER1 = 9,
        GUILD_BOOST_TIER2 = 10,
        GUILD_BOOST_TIER3 = 11,
        CHANNEL_FOLLOW_ADD = 12,
        GUILD_DISCOVERY_DISQUALIFIED = 14,
        GUILD_DISCOVERY_REQUALIFIED = 15,
        GUILD_DISCOVERY_GRACE_PERIOD_INITIAL_WARNING = 16,
        GUILD_DISCOVERY_GRACE_PERIOD_FINAL_WARNING = 17,
        THREAD_CREATED = 18,
        REPLY = 19,
        CHAT_INPUT_COMMAND = 20,
        THREAD_STARTER_MESSAGE = 21,
        GUILD_INVITE_REMINDER = 22,
        CONTEXT_MENU_COMMAND = 23,
        AUTO_MODERATION_ACTION = 24,
        ROLE_SUBSCRIPTION_PURCHASE = 25,
        INTERACTION_PREMIUM_UPSELL = 26,
        STAGE_START = 27,
        STAGE_END = 28,
        STAGE_SPEAKER = 29,
        /// Unstable upstream. <https://github.com/discord/discord-api-docs/pull/5927#discussion_r1107678548>
        STAGE_RAISE_HAND = 30,
        STAGE_TOPIC = 31,
        GUILD_APPLICATION_PREMIUM_SUBSCRIPTION = 32,
        GUILD_INCIDENT_ALERT_MODE_ENABLED = 36,
        GUILD_INCIDENT_ALERT_MODE_DISABLED = 37,
        GUILD_INCIDENT_REPORT_RAID = 38,
        GUILD_INCIDENT_REPORT_FALSE_ALARM = 39,
        PURCHASE_NOTIFICATION = 44,
        POLL_RESULT = 46,
    }
}

open_int! {
    /// How a message reference relates to its target.
    MessageReferenceType: u16 {
        /// A standard reference used by replies
        DEFAULT = 0,
        /// Reference used to point to a message at a point in time
        FORWARD = 1,
    }
}

open_int! {
    /// Interaction kinds.
    InteractionType: u16 {
        PING = 1,
        APPLICATION_COMMAND = 2,
        MESSAGE_COMPONENT = 3,
        APPLICATION_COMMAND_AUTOCOMPLETE = 4,
        MODAL_SUBMIT = 5,
    }
}

open_int! {
    /// Interaction callback kinds.
    InteractionResponseType: u16 {
        /// ACK a `Ping`
        PONG = 1,
        /// Respond to an interaction with a message
        CHANNEL_MESSAGE_WITH_SOURCE = 4,
        /// ACK an interaction and edit to a response later, the user sees a loading state
        DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE = 5,
        /// ACK a component or modal submit interaction and update it to a loading state
        DEFERRED_MESSAGE_UPDATE = 6,
        /// ACK a component or modal submit interaction and edit the message to which the component was
        /// attached
        UPDATE_MESSAGE = 7,
        /// For autocomplete interactions
        APPLICATION_COMMAND_AUTOCOMPLETE_RESULT = 8,
        /// Respond to an interaction with an modal for a user to fill-out
        MODAL = 9,
        /// Respond to an interaction with an upgrade button, only available for apps with monetization
        /// enabled
        ///
        /// Deprecated upstream. Send a button with Premium type instead. Learn more here
        PREMIUM_REQUIRED = 10,
        /// Launch the Activity associated with the app.
        LAUNCH_ACTIVITY = 12,
    }
}

open_int! {
    /// Component kinds.
    ComponentType: u16 {
        /// Container to display a row of interactive components
        ACTION_ROW = 1,
        /// Button component
        BUTTON = 2,
        /// Select menu for picking from defined text options
        STRING_SELECT = 3,
        /// Text Input component
        TEXT_INPUT = 4,
        /// Select menu for users
        USER_SELECT = 5,
        /// Select menu for roles
        ROLE_SELECT = 6,
        /// Select menu for users and roles
        MENTIONABLE_SELECT = 7,
        /// Select menu for channels
        CHANNEL_SELECT = 8,
        /// Container to display text alongside an accessory component
        SECTION = 9,
        /// Markdown text
        TEXT_DISPLAY = 10,
        /// Small image that can be used as an accessory
        THUMBNAIL = 11,
        /// Display images and other media
        MEDIA_GALLERY = 12,
        /// Displays an attached file
        FILE = 13,
        /// Component to add vertical padding between other components
        SEPARATOR = 14,
        /// Unstable upstream. This component type is currently not documented by Discord but has a known
        /// value which we will try to keep up to date.
        CONTENT_INVENTORY_ENTRY = 16,
        /// Container that visually groups a set of components
        CONTAINER = 17,
        /// Container associating a label and description with a component
        LABEL = 18,
        /// Component for uploading files
        FILE_UPLOAD = 19,
        /// Single-choice set of radio group option
        RADIO_GROUP = 21,
        /// Multi-select group of checkboxes
        CHECKBOX_GROUP = 22,
        /// Single checkbox for binary choice
        CHECKBOX = 23;
        /// Select menu for picking from defined text options
        ///
        /// Deprecated upstream. This is the old name for `STRING_SELECT`
        alias SELECT_MENU = STRING_SELECT;
    }
}

open_int! {
    /// Button styles.
    ButtonStyle: u16 {
        /// The most important or recommended action in a group of options
        PRIMARY = 1,
        /// Alternative or supporting actions
        SECONDARY = 2,
        /// Positive confirmation or completion actions
        SUCCESS = 3,
        /// An action with irreversible consequences
        DANGER = 4,
        /// Navigates to a URL
        LINK = 5,
        /// Purchase
        PREMIUM = 6,
    }
}

open_int! {
    /// Text input styles.
    TextInputStyle: u16 {
        /// Single-line input
        SHORT = 1,
        /// Multi-line input
        PARAGRAPH = 2,
    }
}

open_int! {
    /// Application command kinds.
    ApplicationCommandType: u16 {
        /// Slash commands; a text-based command that shows up when a user types `/`
        CHAT_INPUT = 1,
        /// A UI-based command that shows up when you right click or tap on a user
        USER = 2,
        /// A UI-based command that shows up when you right click or tap on a message
        MESSAGE = 3,
        /// A UI-based command that represents the primary way to invoke an app's Activity
        PRIMARY_ENTRY_POINT = 4,
    }
}

open_int! {
    /// Application command option kinds.
    ApplicationCommandOptionType: u16 {
        SUBCOMMAND = 1,
        SUBCOMMAND_GROUP = 2,
        STRING = 3,
        INTEGER = 4,
        BOOLEAN = 5,
        USER = 6,
        CHANNEL = 7,
        ROLE = 8,
        MENTIONABLE = 9,
        NUMBER = 10,
        ATTACHMENT = 11;
        /// The spelling of the Discord documentation.
        alias SUB_COMMAND = SUBCOMMAND;
        /// The spelling of the Discord documentation.
        alias SUB_COMMAND_GROUP = SUBCOMMAND_GROUP;
    }
}

open_str! {
    /// Guild features.
    GuildFeature {
        /// Guild has access to set an animated guild banner image
        AnimatedBanner = "ANIMATED_BANNER",
        /// Guild has access to set an animated guild icon
        AnimatedIcon = "ANIMATED_ICON",
        /// Guild is using the old permissions configuration behavior
        ApplicationCommandPermissionsV2 = "APPLICATION_COMMAND_PERMISSIONS_V2",
        /// Guild has set up auto moderation rules
        AutoModeration = "AUTO_MODERATION",
        /// Guild has access to set a guild banner image
        Banner = "BANNER",
        /// Guild can enable welcome screen, Membership Screening and discovery, and receives community
        /// updates
        Community = "COMMUNITY",
        /// Guild has enabled monetization
        CreatorMonetizableProvisional = "CREATOR_MONETIZABLE_PROVISIONAL",
        /// Guild has enabled the role subscription promo page
        CreatorStorePage = "CREATOR_STORE_PAGE",
        /// Guild has been set as a support server on the App Directory
        DeveloperSupportServer = "DEVELOPER_SUPPORT_SERVER",
        /// Guild is able to be discovered in the directory
        Discoverable = "DISCOVERABLE",
        /// Guild is able to set gradient colors to roles
        EnhancedRoleColors = "ENHANCED_ROLE_COLORS",
        /// Guild is able to be featured in the directory
        Featurable = "FEATURABLE",
        /// Guild has access to guest invites
        GuestsEnabled = "GUESTS_ENABLED",
        /// Guild has access to set guild tags
        GuildTags = "GUILD_TAGS",
        /// Guild is listed in a directory channel
        HasDirectoryEntry = "HAS_DIRECTORY_ENTRY",
        /// Guild is a Student Hub
        ///
        /// Unstable upstream. This feature is currently not documented by Discord, but has known value
        Hub = "HUB",
        /// Guild has disabled invite usage, preventing users from joining
        InvitesDisabled = "INVITES_DISABLED",
        /// Guild has access to set an invite splash background
        InviteSplash = "INVITE_SPLASH",
        /// Guild is in a Student Hub
        ///
        /// Unstable upstream. This feature is currently not documented by Discord, but has known value
        LinkedToHub = "LINKED_TO_HUB",
        /// Guild has enabled Membership Screening
        MemberVerificationGateEnabled = "MEMBER_VERIFICATION_GATE_ENABLED",
        /// Guild has enabled monetization
        ///
        /// Unstable upstream. This feature is no longer documented by Discord
        MonetizationEnabled = "MONETIZATION_ENABLED",
        /// Guild has increased custom soundboard sound slots
        MoreSoundboard = "MORE_SOUNDBOARD",
        /// Guild has increased custom sticker slots
        MoreStickers = "MORE_STICKERS",
        /// Guild has access to create news channels
        News = "NEWS",
        /// Guild is partnered
        Partnered = "PARTNERED",
        /// Guild has migrated to the new pin messages permission
        ///
        /// Unstable upstream. This feature is currently not documented by Discord, but has known value
        PinPermissionMigrationComplete = "PIN_PERMISSION_MIGRATION_COMPLETE",
        /// Guild can be previewed before joining via Membership Screening or the directory
        PreviewEnabled = "PREVIEW_ENABLED",
        /// Guild has access to create private threads
        PrivateThreads = "PRIVATE_THREADS",
        /// Guild has enabled requiring admin to prune members
        PruneRequiresAdmin = "PRUNE_REQUIRES_ADMIN",
        /// Guild has disabled alerts for join raids in the configured safety alerts channel
        RaidAlertsDisabled = "RAID_ALERTS_DISABLED",
        RelayEnabled = "RELAY_ENABLED",
        /// Guild is able to set role icons
        RoleIcons = "ROLE_ICONS",
        /// Guild has role subscriptions that can be purchased
        RoleSubscriptionsAvailableForPurchase = "ROLE_SUBSCRIPTIONS_AVAILABLE_FOR_PURCHASE",
        /// Guild has enabled role subscriptions
        RoleSubscriptionsEnabled = "ROLE_SUBSCRIPTIONS_ENABLED",
        /// Guild has created soundboard sounds
        Soundboard = "SOUNDBOARD",
        /// Guild has enabled ticketed events
        TicketedEventsEnabled = "TICKETED_EVENTS_ENABLED",
        /// Guild has access to set a vanity URL
        VanityURL = "VANITY_URL",
        /// Guild is verified
        Verified = "VERIFIED",
        /// Guild has access to set 384kbps bitrate in voice (previously VIP voice servers)
        VIPRegions = "VIP_REGIONS",
        /// Guild has enabled the welcome screen
        WelcomeScreenEnabled = "WELCOME_SCREEN_ENABLED",
    }
}

open_str! {
    /// What `allowed_mentions.parse` may contain. Upstream `AllowedMentionsTypes`.
    AllowedMentionType {
        /// Controls `@everyone` and `@here` mentions
        Everyone = "everyone",
        /// Controls role mentions
        Roles = "roles",
        /// Controls user mentions
        Users = "users",
    }
}

open_str! {
    /// Languages Discord supports.
    Locale {
        Indonesian = "id",
        EnglishUS = "en-US",
        EnglishGB = "en-GB",
        Bulgarian = "bg",
        ChineseCN = "zh-CN",
        ChineseTW = "zh-TW",
        Croatian = "hr",
        Czech = "cs",
        Danish = "da",
        Dutch = "nl",
        Finnish = "fi",
        French = "fr",
        German = "de",
        Greek = "el",
        Hindi = "hi",
        Hungarian = "hu",
        Italian = "it",
        Japanese = "ja",
        Korean = "ko",
        Lithuanian = "lt",
        Norwegian = "no",
        Polish = "pl",
        PortugueseBR = "pt-BR",
        Romanian = "ro",
        Russian = "ru",
        SpanishES = "es-ES",
        SpanishLATAM = "es-419",
        Swedish = "sv-SE",
        Thai = "th",
        Turkish = "tr",
        Ukrainian = "uk",
        Vietnamese = "vi",
    }
}
