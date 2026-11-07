//! Messages, embeds, attachments, reactions. Upstream: `payloads/v10/message.ts` of
//! discord-api-types (MIT, (c) 2020 vladfrangu), plus the gateway extras of
//! `GatewayMessageEventExtraFields` and the interaction records of `_interactions/base.ts`.
//!
//! Checked field by field against those files. What upstream has and this file leaves out, on
//! purpose: the cut features `poll`, `sticker_items`, `stickers`, `thread` and
//! `role_subscription_data`; `Message.application` and `Attachment.application`, because the proof
//! of concept has no `Application` struct; and `APIMessageSnapshot.guild_id`, which upstream marks as
//! not existing on the real API. All of them are ignored on read, like any unknown key.
//! `mentions[].member` of the gateway form is ignored too, because `mentions` is a plain list of
//! [`User`].
//!
//! A `?` field that is unset is not written, and a `| null` one is written as `null`, so a serialized
//! message has the shape Discord sends.

use crate::{
    ApplicationId, ApplicationIntegrationType, AttachmentId, ChannelId, ChannelType, Component, Emoji, GenericId,
    GuildId, InteractionId, InteractionType, Member, MessageFlags, MessageId, MessageReferenceType, MessageType, Nonce,
    ResolvedData, RoleId, User, UserId, WebhookId,
};
use dr_core::{Patch, Timestamp, null_default, open_flags, open_int, open_str};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A message, from `MESSAGE_CREATE` or REST. `id`, `channel_id` and `author` are required, so a
/// missing one is a decode failure. A partial `MESSAGE_UPDATE` decodes as `MessageUpdateEvent`.
///
/// Without the `MESSAGE_CONTENT` intent, `content`, `embeds`, `attachments` and `components` arrive
/// empty. Empty does not mean the message has no content.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    /// Message id.
    pub id: MessageId,
    /// Channel it was sent in.
    pub channel_id: ChannelId,
    /// Guild, on gateway payloads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// Kind of the channel it was sent in, on gateway payloads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_type: Option<ChannelType>,
    /// Author. A plain [`User`]: to act on it, use `msg.author()` on a `Bound<Message>`. When
    /// `webhook_id` is set this is the webhook's id, name and avatar, not a real user.
    pub author: User,
    /// Member data of the author, on guild gateway payloads. It has no `user`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
    /// Text. Empty without the `MESSAGE_CONTENT` intent.
    #[serde(default)]
    pub content: String,
    /// When it was sent. Prefer the id for its age, `Message::created_at_ms`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    /// When it was last edited, `None` if never.
    #[serde(default)]
    pub edited_timestamp: Option<Timestamp>,
    /// Message kind.
    #[serde(rename = "type", default = "default_message_type")]
    #[doc(alias = "type")]
    pub kind: MessageType,
    /// Whether it is text to speech.
    #[serde(default)]
    pub tts: bool,
    /// Whether it mentions everyone.
    #[serde(default)]
    pub mention_everyone: bool,
    /// Mentioned users.
    #[serde(default, deserialize_with = "null_default")]
    pub mentions: Vec<User>,
    /// Mentioned roles.
    #[serde(default, deserialize_with = "null_default")]
    pub mention_roles: Vec<RoleId>,
    /// Mentioned channels. Discord sends this only on crossposted messages, and not every channel
    /// mention is listed.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub mention_channels: Vec<ChannelMention>,
    /// Attachments.
    #[serde(default, deserialize_with = "null_default")]
    pub attachments: Vec<Attachment>,
    /// Embeds.
    #[serde(default, deserialize_with = "null_default")]
    pub embeds: Vec<Embed>,
    /// Reactions.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<Reaction>,
    /// Nonce echoed back, only once and only on `MESSAGE_CREATE`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<Nonce>,
    /// Whether it is pinned.
    #[serde(default)]
    pub pinned: bool,
    /// Webhook that sent it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_id: Option<WebhookId>,
    /// Rich Presence activity of the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<MessageActivity>,
    /// Application that sent it, for interaction replies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    /// What this message replies to or forwards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<MessageReference>,
    /// Flags. Unknown bits are kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    /// The replied-to message: absent means not a reply or not fetched, null means deleted.
    #[serde(default, deserialize_with = "crate::message_ext::bounded_depth", skip_serializing_if = "Patch::is_absent")]
    pub referenced_message: Patch<Box<Message>>,
    /// The interaction this message was sent in response to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_metadata: Option<Box<MessageInteractionMetadata>>,
    /// The interaction this message responds to. Upstream deprecates it in favour of
    /// `interaction_metadata`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction: Option<Box<MessageInteraction>>,
    /// Components. Unknown kinds are kept.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,
    /// Approximate position in a thread. It can have gaps and duplicates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<i64>,
    /// The forwarded message, on a forward: a subset of a message's fields. The forward's own
    /// `content` is empty.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub message_snapshots: Vec<MessageSnapshot>,
    /// The call this message is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<MessageCall>,
    /// A custom client theme shared through the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_client_theme: Option<MessageSharedClientTheme>,
    /// The objects the auto-populated selects of this message name. A resolved message can hold
    /// another, hence the depth limit.
    #[serde(default, deserialize_with = "crate::message_ext::bounded_depth", skip_serializing_if = "Option::is_none")]
    pub resolved: Option<Box<ResolvedData>>,
}

const fn default_message_type() -> MessageType {
    MessageType::DEFAULT
}

/// A channel mention, from `mention_channels`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChannelMention {
    /// Channel id.
    pub id: ChannelId,
    /// Guild holding the channel.
    pub guild_id: GuildId,
    /// Channel kind.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: ChannelType,
    /// Channel name.
    #[serde(default)]
    pub name: String,
}

open_int! {
    /// Rich Presence activity kinds on a message.
    MessageActivityType: u16 {
        JOIN = 1,
        SPECTATE = 2,
        LISTEN = 3,
        JOIN_REQUEST = 5,
        STREAM_REQUEST = 6,
    }
}

/// The Rich Presence activity of a message.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageActivity {
    /// Activity kind.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: MessageActivityType,
    /// `party_id` of the Rich Presence event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party_id: Option<String>,
}

/// A reference from one message to another.
///
/// Upstream's receive type requires `channel_id`, and its send type adds `fail_if_not_exists`. This
/// one struct serves both, so `channel_id` is optional: a reply is sent with `message_id` alone.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MessageReference {
    /// Reply or forward. Absent means reply.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    #[doc(alias = "type")]
    pub kind: Option<MessageReferenceType>,
    /// Target message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<MessageId>,
    /// Target channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    /// Target guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// On send: error if the target does not exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_if_not_exists: Option<bool>,
}

/// The fields of a forwarded message that `message_snapshots` carries. It has no author.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageSnapshotFields {
    /// Text.
    #[serde(default)]
    pub content: String,
    /// When the original was sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    /// When the original was last edited, `None` if never.
    #[serde(default)]
    pub edited_timestamp: Option<Timestamp>,
    /// Message kind.
    #[serde(rename = "type", default = "default_message_type")]
    #[doc(alias = "type")]
    pub kind: MessageType,
    /// Flags. Unknown bits are kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    /// Mentioned users.
    #[serde(default, deserialize_with = "null_default")]
    pub mentions: Vec<User>,
    /// Mentioned roles.
    #[serde(default, deserialize_with = "null_default")]
    pub mention_roles: Vec<RoleId>,
    /// Attachments.
    #[serde(default, deserialize_with = "null_default")]
    pub attachments: Vec<Attachment>,
    /// Embeds.
    #[serde(default, deserialize_with = "null_default")]
    pub embeds: Vec<Embed>,
    /// Components. Unknown kinds are kept.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,
}

/// One forwarded message.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageSnapshot {
    /// What was forwarded.
    pub message: MessageSnapshotFields,
}

/// The call a message is about.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MessageCall {
    /// Users who took part.
    #[serde(default, deserialize_with = "null_default")]
    pub participants: Vec<UserId>,
    /// When it ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_timestamp: Option<Timestamp>,
}

open_int! {
    /// Base modes of a shared client theme.
    BaseThemeType: u16 {
        UNSET = 0,
        DARK = 1,
        LIGHT = 2,
        DARKER = 3,
        MIDNIGHT = 4,
    }
}

/// A custom client theme shared through a message.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MessageSharedClientTheme {
    /// Hexadecimal colours, at most 5.
    #[serde(default, deserialize_with = "null_default")]
    pub colors: Vec<String>,
    /// Direction of the colours, at most 360.
    #[serde(default)]
    pub gradient_angle: i64,
    /// Intensity of the colours, at most 100.
    #[serde(default)]
    pub base_mix: i64,
    /// Theme mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_theme: Option<BaseThemeType>,
}

/// The interaction a message responds to. Upstream has one type per interaction kind, which is one
/// flat struct here: `kind` says which of the optional fields can be set.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageInteractionMetadata {
    /// Interaction id.
    pub id: InteractionId,
    /// Interaction kind.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: InteractionType,
    /// User who triggered it.
    pub user: User,
    /// Installation contexts that authorized it. Unknown kinds are kept.
    #[serde(default, deserialize_with = "null_default")]
    pub authorizing_integration_owners: BTreeMap<ApplicationIntegrationType, GenericId>,
    /// Original response, on a follow-up only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_response_message_id: Option<MessageId>,
    /// The user a user command ran on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_user: Option<User>,
    /// The message a message command ran on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_message_id: Option<MessageId>,
    /// The message holding the component that was used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interacted_message_id: Option<MessageId>,
    /// On a modal submit: the interaction that opened the modal.
    #[serde(default, deserialize_with = "crate::message_ext::bounded_depth", skip_serializing_if = "Option::is_none")]
    pub triggering_interaction_metadata: Option<Box<MessageInteractionMetadata>>,
}

/// The older record of the interaction a message responds to.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MessageInteraction {
    /// Interaction id.
    pub id: InteractionId,
    /// Interaction kind.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: InteractionType,
    /// Command name, including subcommands and groups.
    #[serde(default)]
    pub name: String,
    /// User who invoked it.
    pub user: User,
    /// Member who invoked it, on `MESSAGE_CREATE` only. It has no `user`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
}

open_flags! {
    /// Flags on an attachment.
    AttachmentFlags: number {
        /// A clip from a stream.
        IS_CLIP = 1 << 0,
        /// The thumbnail of a thread in a media channel, shown in the grid and not on the message.
        IS_THUMBNAIL = 1 << 1,
        /// Edited with the remix feature on mobile. Upstream marks it deprecated.
        IS_REMIX = 1 << 2,
        /// Marked as a spoiler, blurred until clicked.
        IS_SPOILER = 1 << 3,
        /// Flagged as explicit by Discord's content scanning.
        CONTAINS_EXPLICIT_CONTENT = 1 << 4,
        /// An animated image.
        IS_ANIMATED = 1 << 5,
        /// Flagged as gore by Discord's content scanning.
        CONTAINS_GORE_CONTENT = 1 << 6,
        /// Flagged as self-harm by Discord's content scanning.
        CONTAINS_SELF_HARM_CONTENT = 1 << 7,
    }
}

/// A file on a message.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Attachment {
    /// Attachment id.
    pub id: AttachmentId,
    /// File name.
    #[serde(default)]
    pub filename: String,
    /// Original name with special characters kept, sent when `filename` had to be sanitized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Alt text, at most 1024 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Size in bytes.
    #[serde(default)]
    pub size: u64,
    /// Source URL.
    #[serde(default)]
    pub url: String,
    /// Proxied URL. It serves images, videos and audio, and answers `415` for anything else.
    #[serde(default)]
    pub proxy_url: String,
    /// Height, on an image or video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
    /// Width, on an image or video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    /// ThumbHash placeholder, on an image or video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Version of the placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder_version: Option<i64>,
    /// Whether it is removed after a while. On a message it lasts as long as the message does.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ephemeral: bool,
    /// Length in seconds of an audio or video file. Fractional, unlike the other numbers here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
    /// Base64 of a sampled waveform, on a voice message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waveform: Option<String>,
    /// Flags. Unknown bits are kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<AttachmentFlags>,
    /// On a clip: the users who were in the stream.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub clip_participants: Vec<User>,
    /// On a clip: when it was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_created_at: Option<Timestamp>,
}

open_str! {
    /// Embed kinds. A webhook embed is always `Rich`.
    EmbedType {
        Rich = "rich",
        Image = "image",
        Video = "video",
        /// An animated gif rendered as a video.
        GIFV = "gifv",
        Article = "article",
        Link = "link",
        /// An auto moderation alert. Upstream marks it unstable: returned, not documented.
        AutoModerationMessage = "auto_moderation_message",
        PollResult = "poll_result",
        /// A link preview built from components.
        Components = "components",
    }
}

open_flags! {
    /// Flags on an embed.
    EmbedFlags: number {
        /// Media flagged as explicit by Discord's content scanning.
        CONTAINS_EXPLICIT_CONTENT = 1 << 4,
        /// A fallback for a reply to an activity card.
        IS_CONTENT_INVENTORY_ENTRY = 1 << 5,
        /// Media flagged as gore by Discord's content scanning.
        CONTAINS_GORE_CONTENT = 1 << 6,
        /// Media flagged as self-harm by Discord's content scanning.
        CONTAINS_SELF_HARM_CONTENT = 1 << 7,
    }
}

open_flags! {
    /// Flags on embed media.
    EmbedMediaFlags: number {
        /// The image is animated.
        IS_ANIMATED = 1 << 5,
    }
}

/// An embed. Everything is optional on the wire.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Embed {
    /// Title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Embed kind.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    #[doc(alias = "type")]
    pub kind: Option<EmbedType>,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Title link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Display time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    /// Colour as `0xRRGGBB`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    /// Footer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<EmbedFooter>,
    /// Image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<EmbedImage>,
    /// Thumbnail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<EmbedImage>,
    /// Video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<EmbedVideo>,
    /// Provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<EmbedProvider>,
    /// Author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<EmbedAuthor>,
    /// Fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "null_default")]
    pub fields: Vec<EmbedField>,
    /// The single container of a `Components` embed. Unknown kinds are kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "null_default")]
    pub components: Vec<Component>,
    /// Flags. Unknown bits are kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<EmbedFlags>,
}

/// One field of an embed.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct EmbedField {
    /// Heading.
    #[serde(default)]
    pub name: String,
    /// Text.
    #[serde(default)]
    pub value: String,
    /// Whether it sits beside its neighbours.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inline: bool,
}

/// The image or thumbnail of an embed.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct EmbedImage {
    /// Source URL, `http(s)` or `attachment`.
    #[serde(default)]
    pub url: String,
    /// Proxied URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<String>,
    /// Height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
    /// Width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    /// Media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// ThumbHash placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Version of the placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder_version: Option<i64>,
    /// Alt text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Flags. Unknown bits are kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<EmbedMediaFlags>,
}

/// The video of an embed. Unlike an image, even `url` is optional.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct EmbedVideo {
    /// Source URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Proxied URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<String>,
    /// Height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
    /// Width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    /// Media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// ThumbHash placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Version of the placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder_version: Option<i64>,
    /// Alt text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Flags. Unknown bits are kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<EmbedMediaFlags>,
}

/// The provider of an embed.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct EmbedProvider {
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// The author of an embed.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct EmbedAuthor {
    /// Name.
    #[serde(default)]
    pub name: String,
    /// Link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Icon URL, `http(s)` or `attachment`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// Proxied icon URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_icon_url: Option<String>,
}

/// The footer of an embed.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct EmbedFooter {
    /// Text.
    #[serde(default)]
    pub text: String,
    /// Icon URL, `http(s)` or `attachment`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// Proxied icon URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_icon_url: Option<String>,
}

/// One reaction on a message.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Reaction {
    /// How many times it was used, including super reactions.
    #[serde(default)]
    pub count: u64,
    /// The same count, split by reaction kind.
    pub count_details: ReactionCountDetails,
    /// Whether the bot used it.
    #[serde(default)]
    pub me: bool,
    /// Whether the bot used it as a super reaction.
    #[serde(default)]
    pub me_burst: bool,
    /// The emoji.
    pub emoji: Emoji,
    /// Hexadecimal colours of the super reaction.
    #[serde(default, deserialize_with = "null_default")]
    pub burst_colors: Vec<String>,
}

/// Reaction counts by kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ReactionCountDetails {
    /// Super reactions.
    #[serde(default)]
    pub burst: u64,
    /// Normal reactions.
    #[serde(default)]
    pub normal: u64,
}
