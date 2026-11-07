//! The wire `Interaction`: one struct, with typed views in the facade (`ARCHITECTURE.md` 7.4), and
//! the callback payloads.
//!
//! Wire shapes: `payloads/v10/interactions.ts` and `payloads/v10/_interactions/*.ts` of
//! `discord-api-types` (MIT). Guards and context menu targets: `structures/BaseInteraction.js`,
//! `ContextMenuCommandInteraction.js` and `ModalComponentResolver.js` of `discord.js` (Apache-2.0),
//! `[client 12]`.
//!
//! The kind of `data` is decided by `type` next to it, so `data` stays raw until the whole object is
//! read. Upstream's DM and guild wrappers collapse into `Option` fields, `[types 3.4]`. `entitlements`
//! is left out until monetization is ported, and thread metadata until threads are.

use crate::{
    ApplicationCommandType, ApplicationId, ApplicationIntegrationType, Attachment, AttachmentId, Channel, ChannelId,
    ChannelType, CommandId, Component, ComponentType, GenericId, GuildFeature, GuildId, HoistedOptions,
    InteractionContextType, InteractionId, InteractionOption, InteractionResponseType, InteractionType, Member,
    Message, MessageFlags, MessageId, Permissions, Role, RoleId, User, UserId,
};
use dr_core::{BodyError, IntoBody, RequestBody, Secret, decode, null_default, tagged_union};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;
use std::collections::BTreeMap;

/// An interaction as received on `INTERACTION_CREATE`.
///
/// `id`, `application_id`, `type` and `token` are required, so a missing one is a decode failure of
/// the event. A `data` that does not decode is kept as [`InteractionData::Malformed`], with the token
/// still at hand.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(from = "InteractionWire")]
pub struct Interaction {
    /// Interaction id. Its age sets the 3 second callback deadline.
    pub id: InteractionId,
    /// Application it was sent to.
    pub application_id: ApplicationId,
    /// Interaction kind. Unknown kinds are kept.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: InteractionType,
    /// Token for the callback and follow-ups. Redacted in `Debug`.
    pub token: Secret,
    /// Kind-specific data. `None` on a ping.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<InteractionData>,
    /// Partial guild it came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild: Option<PartialInteractionGuild>,
    /// Guild it came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// Partial channel it came from: `id` and `type` are always there, the rest depends on the kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<Channel>,
    /// Channel it came from. Upstream deprecates the wire field and reads `channel.id` instead, so
    /// that one wins here when both are sent, `[client 16.52]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    /// Invoking member with its `permissions`, in a guild.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
    /// Invoking user, in a DM. In a guild it is in `member`, see [`Interaction::invoker`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// Always `1` on the wire. Not validated.
    pub version: i64,
    /// Message a component is attached to, or that a modal was opened from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Box<Message>>,
    /// The bot's permissions in the channel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_permissions: Option<Permissions>,
    /// Locale of the invoking user. A ping has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Preferred locale of the guild.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild_locale: Option<String>,
    /// The user or guild id each installation context was authorized for. A guild install used
    /// outside a guild has the id `0`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub authorizing_integration_owners: BTreeMap<ApplicationIntegrationType, GenericId>,
    /// Where it was triggered from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<InteractionContextType>,
    /// Attachment size limit in bytes.
    pub attachment_size_limit: i64,
}

/// The guild of an interaction, sent even when the bot is not in it. Upstream:
/// `APIPartialInteractionGuild`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PartialInteractionGuild {
    /// Guild id.
    pub id: GuildId,
    /// Preferred locale.
    #[serde(default)]
    pub locale: String,
    /// Enabled features. Unknown ones are kept.
    #[serde(default, deserialize_with = "null_default")]
    pub features: Vec<GuildFeature>,
}

/// The receive form: `data` stays raw until `type` is known, wherever it sits in the object.
#[derive(Deserialize)]
struct InteractionWire {
    id: InteractionId,
    application_id: ApplicationId,
    #[serde(rename = "type")]
    kind: InteractionType,
    token: Secret,
    #[serde(default)]
    data: Option<Box<RawValue>>,
    #[serde(default)]
    guild: Option<PartialInteractionGuild>,
    #[serde(default)]
    guild_id: Option<GuildId>,
    #[serde(default)]
    channel: Option<Channel>,
    #[serde(default)]
    channel_id: Option<ChannelId>,
    #[serde(default)]
    member: Option<Member>,
    #[serde(default)]
    user: Option<User>,
    #[serde(default)]
    version: i64,
    #[serde(default)]
    message: Option<Box<Message>>,
    #[serde(default)]
    app_permissions: Option<Permissions>,
    #[serde(default)]
    locale: Option<String>,
    #[serde(default)]
    guild_locale: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    authorizing_integration_owners: BTreeMap<ApplicationIntegrationType, GenericId>,
    #[serde(default)]
    context: Option<InteractionContextType>,
    #[serde(default)]
    attachment_size_limit: i64,
}

impl From<InteractionWire> for Interaction {
    fn from(w: InteractionWire) -> Self {
        Self {
            id: w.id,
            application_id: w.application_id,
            kind: w.kind,
            token: w.token,
            data: w.data.map(|raw| InteractionData::decode(w.kind, raw)),
            guild: w.guild,
            guild_id: w.guild_id,
            channel_id: w.channel.as_ref().map(|c| c.id).or(w.channel_id),
            channel: w.channel,
            member: w.member,
            user: w.user,
            version: w.version,
            message: w.message,
            app_permissions: w.app_permissions,
            locale: w.locale,
            guild_locale: w.guild_locale,
            authorizing_integration_owners: w.authorizing_integration_owners,
            context: w.context,
            attachment_size_limit: w.attachment_size_limit,
        }
    }
}

/// The `data` of an interaction, chosen by the interaction's `type`.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum InteractionData {
    /// `type` 2: a command was used.
    ApplicationCommand(CommandData),
    /// `type` 4: an option with autocomplete is being typed. Same shape, but `INTEGER` and `NUMBER`
    /// values arrive as text.
    Autocomplete(CommandData),
    /// `type` 3: a button or select was used.
    MessageComponent(ComponentData),
    /// `type` 5: a modal was submitted.
    ModalSubmit(ModalSubmitData),
    /// An interaction `type` this build does not know, or data on one that has none. Re-serializes
    /// byte-exact.
    Unknown {
        /// The data as received.
        raw: Box<RawValue>,
    },
    /// A known `type` whose data does not decode. The rest of the interaction, the token included, is
    /// intact. Re-serializes byte-exact.
    Malformed {
        /// The data as received.
        raw: Box<RawValue>,
        /// What went wrong and where.
        error: String,
    },
}

impl InteractionData {
    /// Reads `raw` as `kind` says. Never fails: the shard must survive whatever arrives here.
    fn decode(kind: InteractionType, raw: Box<RawValue>) -> Self {
        fn arm<T: DeserializeOwned>(raw: Box<RawValue>, wrap: impl FnOnce(T) -> InteractionData) -> InteractionData {
            match decode(raw.get()) {
                Ok(data) => wrap(data),
                Err(e) => InteractionData::Malformed { error: e.to_string(), raw },
            }
        }
        match kind {
            InteractionType::APPLICATION_COMMAND => arm(raw, Self::ApplicationCommand),
            InteractionType::APPLICATION_COMMAND_AUTOCOMPLETE => arm(raw, Self::Autocomplete),
            InteractionType::MESSAGE_COMPONENT => arm(raw, Self::MessageComponent),
            InteractionType::MODAL_SUBMIT => arm(raw, Self::ModalSubmit),
            _ => Self::Unknown { raw },
        }
    }
}

impl Serialize for InteractionData {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::ApplicationCommand(d) | Self::Autocomplete(d) => d.serialize(s),
            Self::MessageComponent(d) => d.serialize(s),
            Self::ModalSubmit(d) => d.serialize(s),
            Self::Unknown { raw } | Self::Malformed { raw, .. } => raw.serialize(s),
        }
    }
}

/// The data of a command or autocomplete interaction: chat input, user and message context menus and
/// the primary entry point share this one struct.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommandData {
    /// The command's id.
    pub id: CommandId,
    /// The command's name.
    #[serde(default)]
    pub name: String,
    /// Chat input, user, message or entry point.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: ApplicationCommandType,
    /// The guild, for a guild command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The options, chat input only.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<InteractionOption>,
    /// The objects the options and the target name. Autocomplete normally has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<ResolvedData>,
    /// What a context menu command was used on, a user or a message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[doc(alias = "targetId")]
    pub target_id: Option<GenericId>,
}

impl CommandData {
    /// The options with a leading subcommand group and subcommand taken off.
    pub fn hoisted(&self) -> HoistedOptions<'_> {
        HoistedOptions::new(&self.options, self.resolved.as_ref())
    }

    /// The user a user command was used on.
    pub fn target_user(&self) -> Option<&User> {
        self.resolved.as_ref()?.users.get(&self.target_id?.cast())
    }

    /// The member a user command was used on, in a guild. It has no `user`, see
    /// [`ResolvedData::member_with_user`].
    pub fn target_member(&self) -> Option<&Member> {
        self.resolved.as_ref()?.members.get(&self.target_id?.cast())
    }

    /// The message a message command was used on.
    pub fn target_message(&self) -> Option<&Message> {
        self.resolved.as_ref()?.messages.get(&self.target_id?.cast())
    }
}

/// The data of a button or select interaction.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ComponentData {
    /// The component's `custom_id`.
    #[serde(default)]
    pub custom_id: String,
    /// The component's kind. Unknown kinds are kept.
    pub component_type: ComponentType,
    /// The values chosen in a select: strings, or ids for the auto-populated kinds. Empty for a
    /// button.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<String>,
    /// The objects an auto-populated select names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<ResolvedData>,
}

/// The objects named by ids in the options or the select values of an interaction. These are partial
/// and stay wire data with no cache write, `ARCHITECTURE.md` 10.6.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ResolvedData {
    /// Users.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "BTreeMap::is_empty")]
    pub users: BTreeMap<UserId, User>,
    /// Roles.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "BTreeMap::is_empty")]
    pub roles: BTreeMap<RoleId, Role>,
    /// Members, keyed by user id. They have `permissions` and no `user`.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "BTreeMap::is_empty")]
    pub members: BTreeMap<UserId, Member>,
    /// Channels.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "BTreeMap::is_empty")]
    pub channels: BTreeMap<ChannelId, ResolvedChannel>,
    /// Attachments.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "BTreeMap::is_empty")]
    pub attachments: BTreeMap<AttachmentId, Attachment>,
    /// Messages, for a message command.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "BTreeMap::is_empty")]
    pub messages: BTreeMap<MessageId, Message>,
}

impl ResolvedData {
    /// The user an option names, by the option's value as upstream's `transformOption` looks it up.
    pub fn user_of(&self, option: &InteractionOption) -> Option<&User> {
        self.users.get(&option.as_snowflake()?.cast())
    }

    /// The member an option names.
    pub fn member_of(&self, option: &InteractionOption) -> Option<&Member> {
        self.members.get(&option.as_snowflake()?.cast())
    }

    /// The role an option names.
    pub fn role_of(&self, option: &InteractionOption) -> Option<&Role> {
        self.roles.get(&option.as_snowflake()?.cast())
    }

    /// The channel an option names.
    pub fn channel_of(&self, option: &InteractionOption) -> Option<&ResolvedChannel> {
        self.channels.get(&option.as_snowflake()?.cast())
    }

    /// The attachment an option names.
    pub fn attachment_of(&self, option: &InteractionOption) -> Option<&Attachment> {
        self.attachments.get(&option.as_snowflake()?.cast())
    }

    /// A member with its `user` filled in from `users`, upstream's `{ user, ...member }`.
    pub fn member_with_user(&self, id: UserId) -> Option<Member> {
        let mut member = self.members.get(&id)?.clone();
        if member.user.is_none() {
            member.user = self.users.get(&id).cloned();
        }
        Some(member)
    }
}

/// A channel in [`ResolvedData`]. Upstream: `APIInteractionDataResolvedChannel`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResolvedChannel {
    /// Channel id.
    pub id: ChannelId,
    /// Channel kind. Unknown kinds are kept.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: ChannelType,
    /// Name.
    #[serde(default)]
    pub name: Option<String>,
    /// What the invoking user may do there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    /// What the bot may do there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_permissions: Option<Permissions>,
    /// Parent channel of a thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<ChannelId>,
}

/// The data of a modal submission. Upstream: `APIModalSubmission`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModalSubmitData {
    /// The modal's `custom_id`.
    #[serde(default)]
    pub custom_id: String,
    /// What was submitted: labels, or action rows in the legacy form.
    #[serde(default, deserialize_with = "null_default")]
    pub components: Vec<ModalSubmitComponent>,
    /// The objects the auto-populated selects name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<ResolvedData>,
}

impl ModalSubmitData {
    /// The bottom-level components: the children of action rows and the component of each label, one
    /// level down, as upstream's `ModalComponentResolver` collects them. Anything else at the top
    /// level, such as a text display, is ignored.
    #[doc(alias = "hoistedComponents")]
    pub fn hoisted(&self) -> Vec<&ModalSubmitComponent> {
        let mut out = Vec::new();
        for top in &self.components {
            match top {
                ModalSubmitComponent::ActionRow(row) => out.extend(&row.components),
                ModalSubmitComponent::Label(label) => out.push(&*label.component),
                _ => {}
            }
        }
        out
    }

    /// A bottom-level component by `custom_id`. On a duplicate the last one wins, as in upstream's
    /// map.
    #[doc(alias = "getComponent")]
    pub fn component(&self, custom_id: &str) -> Option<&ModalSubmitComponent> {
        self.hoisted().into_iter().rev().find(|c| c.custom_id() == Some(custom_id))
    }

    /// What was typed into a text input.
    #[doc(alias = "getTextInputValue")]
    pub fn text_input_value(&self, custom_id: &str) -> Option<&str> {
        match self.component(custom_id)? {
            ModalSubmitComponent::TextInput(input) => Some(&input.value),
            _ => None,
        }
    }

    /// Each text input's `custom_id` and value, in order.
    pub fn text_inputs(&self) -> Vec<(&str, &str)> {
        self.hoisted()
            .into_iter()
            .filter_map(|c| match c {
                ModalSubmitComponent::TextInput(input) => Some((input.custom_id.as_str(), input.value.as_str())),
                _ => None,
            })
            .collect()
    }
}

tagged_union! {
    #[tag = "type"]
    /// A component of a modal submission. One union where upstream separates the top level
    /// (`APIModalSubmissionComponent`) from the leaves (`ModalSubmitComponent`), since the tag says
    /// which is which. An unknown kind round-trips byte-exact through `Unknown`.
    pub enum ModalSubmitComponent {
        1 => ActionRow(ModalSubmitActionRow),
        3 => StringSelect(ModalSubmitValues),
        4 => TextInput(ModalSubmitTextInput),
        5 => UserSelect(ModalSubmitValues),
        6 => RoleSelect(ModalSubmitValues),
        7 => MentionableSelect(ModalSubmitValues),
        8 => ChannelSelect(ModalSubmitValues),
        10 => TextDisplay(ModalSubmitTextDisplay),
        18 => Label(ModalSubmitLabel),
        19 => FileUpload(ModalSubmitValues),
        21 => RadioGroup(ModalSubmitRadioGroup),
        22 => CheckboxGroup(ModalSubmitValues),
        23 => Checkbox(ModalSubmitCheckbox),
        _ => Unknown,
    }
}

impl ModalSubmitComponent {
    /// The `custom_id` of a leaf. Action rows, labels and text displays have none.
    pub fn custom_id(&self) -> Option<&str> {
        match self {
            Self::TextInput(c) => Some(&c.custom_id),
            Self::RadioGroup(c) => Some(&c.custom_id),
            Self::Checkbox(c) => Some(&c.custom_id),
            Self::StringSelect(c)
            | Self::UserSelect(c)
            | Self::RoleSelect(c)
            | Self::MentionableSelect(c)
            | Self::ChannelSelect(c)
            | Self::FileUpload(c)
            | Self::CheckboxGroup(c) => Some(&c.custom_id),
            _ => None,
        }
    }

    /// The values of a select, a file upload or a checkbox group. Empty for anything else.
    pub fn values(&self) -> &[String] {
        match self {
            Self::StringSelect(c)
            | Self::UserSelect(c)
            | Self::RoleSelect(c)
            | Self::MentionableSelect(c)
            | Self::ChannelSelect(c)
            | Self::FileUpload(c)
            | Self::CheckboxGroup(c) => &c.values,
            _ => &[],
        }
    }
}

/// A legacy action row in a modal submission. Upstream types its children as text inputs only.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ModalSubmitActionRow {
    /// Numeric identifier of the component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Children.
    #[serde(default, deserialize_with = "modal_children")]
    pub components: Vec<ModalSubmitComponent>,
}

/// Reads the children of a row, or `null` as none, under the nesting limit: a `tagged_union!` arm that
/// holds its own union would otherwise let wire data overflow the stack.
fn modal_children<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ModalSubmitComponent>, D::Error> {
    let children: Option<Vec<ModalSubmitComponent>> = crate::message_ext::bounded_depth(d)?;
    Ok(children.unwrap_or_default())
}

/// A label in a modal submission, holding one leaf.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModalSubmitLabel {
    /// Numeric identifier of the component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// The leaf.
    #[serde(deserialize_with = "crate::message_ext::bounded_depth")]
    pub component: Box<ModalSubmitComponent>,
}

/// A text display in a modal submission, which carries nothing back.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ModalSubmitTextDisplay {
    /// Numeric identifier of the component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
}

/// A text input in a modal submission.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ModalSubmitTextInput {
    /// Numeric identifier of the component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// The input's `custom_id`.
    #[serde(default)]
    pub custom_id: String,
    /// What was typed.
    #[serde(default)]
    pub value: String,
}

/// A select, a file upload or a checkbox group in a modal submission.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ModalSubmitValues {
    /// Numeric identifier of the component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// The component's `custom_id`.
    #[serde(default)]
    pub custom_id: String,
    /// What was chosen: strings, or ids that [`ResolvedData`] describes.
    #[serde(default, deserialize_with = "null_default")]
    pub values: Vec<String>,
}

/// A radio group in a modal submission.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ModalSubmitRadioGroup {
    /// Numeric identifier of the component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// The group's `custom_id`.
    #[serde(default)]
    pub custom_id: String,
    /// The value chosen, or null when nothing was.
    #[serde(default)]
    pub value: Option<String>,
}

/// A checkbox in a modal submission.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ModalSubmitCheckbox {
    /// Numeric identifier of the component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// The checkbox's `custom_id`.
    #[serde(default)]
    pub custom_id: String,
    /// Whether it was ticked.
    #[serde(default)]
    pub value: bool,
}

impl Interaction {
    /// Who triggered it: `user`, or in a guild `member.user`.
    pub fn invoker(&self) -> Option<&User> {
        self.user.as_ref().or_else(|| self.member.as_ref()?.user.as_ref())
    }

    /// True when `guild_id` was sent. Upstream: `isGuildInteraction`.
    pub fn is_guild(&self) -> bool {
        self.guild_id.is_some()
    }

    /// True when `user` was sent, which happens outside guilds only. Upstream: `isDMInteraction`.
    pub fn is_dm(&self) -> bool {
        self.user.is_some()
    }

    /// True when it has both a guild and a member.
    #[doc(alias = "inGuild")]
    pub fn in_guild(&self) -> bool {
        self.guild_id.is_some() && self.member.is_some()
    }

    /// The permissions of the invoking member in the channel.
    #[doc(alias = "memberPermissions")]
    pub fn member_permissions(&self) -> Option<Permissions> {
        self.member.as_ref()?.permissions
    }

    /// When it was created, in milliseconds since the Unix epoch, from the id.
    #[doc(alias = "createdTimestamp")]
    pub fn created_timestamp_ms(&self) -> i64 {
        self.id.timestamp_ms()
    }

    /// The data of a command or an autocomplete request.
    pub fn command_data(&self) -> Option<&CommandData> {
        match &self.data {
            Some(InteractionData::ApplicationCommand(d) | InteractionData::Autocomplete(d)) => Some(d),
            _ => None,
        }
    }

    /// The data of a button or select.
    pub fn component_data(&self) -> Option<&ComponentData> {
        match &self.data {
            Some(InteractionData::MessageComponent(d)) => Some(d),
            _ => None,
        }
    }

    /// The data of a modal submission.
    pub fn modal_data(&self) -> Option<&ModalSubmitData> {
        match &self.data {
            Some(InteractionData::ModalSubmit(d)) => Some(d),
            _ => None,
        }
    }

    fn command_type(&self) -> Option<ApplicationCommandType> {
        match &self.data {
            Some(InteractionData::ApplicationCommand(d)) => Some(d.kind),
            _ => None,
        }
    }

    fn component_type(&self) -> Option<ComponentType> {
        self.component_data().map(|d| d.component_type)
    }

    /// True for a ping.
    pub fn is_ping(&self) -> bool {
        self.kind == InteractionType::PING
    }

    /// True for an autocomplete request.
    #[doc(alias = "isAutocomplete")]
    pub fn is_autocomplete(&self) -> bool {
        self.kind == InteractionType::APPLICATION_COMMAND_AUTOCOMPLETE
    }

    /// True for a command of any kind.
    #[doc(alias = "isCommand")]
    pub fn is_command(&self) -> bool {
        self.kind == InteractionType::APPLICATION_COMMAND
    }

    /// True for a slash command.
    #[doc(alias = "isChatInputCommand")]
    pub fn is_chat_input_command(&self) -> bool {
        self.command_type() == Some(ApplicationCommandType::CHAT_INPUT)
    }

    /// True for a user or message context menu command.
    #[doc(alias = "isContextMenuCommand")]
    pub fn is_context_menu_command(&self) -> bool {
        self.is_user_context_menu_command() || self.is_message_context_menu_command()
    }

    /// True for a user context menu command.
    #[doc(alias = "isUserContextMenuCommand")]
    pub fn is_user_context_menu_command(&self) -> bool {
        self.command_type() == Some(ApplicationCommandType::USER)
    }

    /// True for a message context menu command.
    #[doc(alias = "isMessageContextMenuCommand")]
    pub fn is_message_context_menu_command(&self) -> bool {
        self.command_type() == Some(ApplicationCommandType::MESSAGE)
    }

    /// True for a primary entry point command.
    #[doc(alias = "isPrimaryEntryPointCommand")]
    pub fn is_primary_entry_point_command(&self) -> bool {
        self.command_type() == Some(ApplicationCommandType::PRIMARY_ENTRY_POINT)
    }

    /// True for a button or select.
    #[doc(alias = "isMessageComponent")]
    pub fn is_message_component(&self) -> bool {
        self.kind == InteractionType::MESSAGE_COMPONENT
    }

    /// True for a modal submission.
    #[doc(alias = "isModalSubmit")]
    pub fn is_modal_submit(&self) -> bool {
        self.kind == InteractionType::MODAL_SUBMIT
    }

    /// True for a button.
    #[doc(alias = "isButton")]
    pub fn is_button(&self) -> bool {
        self.component_type() == Some(ComponentType::BUTTON)
    }

    /// True for a select of any known kind.
    #[doc(alias = "isSelectMenu")]
    pub fn is_select_menu(&self) -> bool {
        matches!(
            self.component_type(),
            Some(
                ComponentType::STRING_SELECT
                    | ComponentType::USER_SELECT
                    | ComponentType::ROLE_SELECT
                    | ComponentType::MENTIONABLE_SELECT
                    | ComponentType::CHANNEL_SELECT
            )
        )
    }

    /// True for a select of strings.
    #[doc(alias = "isStringSelectMenu")]
    pub fn is_string_select_menu(&self) -> bool {
        self.component_type() == Some(ComponentType::STRING_SELECT)
    }

    /// True for a select of users.
    #[doc(alias = "isUserSelectMenu")]
    pub fn is_user_select_menu(&self) -> bool {
        self.component_type() == Some(ComponentType::USER_SELECT)
    }

    /// True for a select of roles.
    #[doc(alias = "isRoleSelectMenu")]
    pub fn is_role_select_menu(&self) -> bool {
        self.component_type() == Some(ComponentType::ROLE_SELECT)
    }

    /// True for a select of users and roles.
    #[doc(alias = "isMentionableSelectMenu")]
    pub fn is_mentionable_select_menu(&self) -> bool {
        self.component_type() == Some(ComponentType::MENTIONABLE_SELECT)
    }

    /// True for a select of channels.
    #[doc(alias = "isChannelSelectMenu")]
    pub fn is_channel_select_menu(&self) -> bool {
        self.component_type() == Some(ComponentType::CHANNEL_SELECT)
    }

    /// True when it can be answered with a message: not a ping and not an autocomplete request.
    #[doc(alias = "isRepliable")]
    pub fn is_repliable(&self) -> bool {
        !self.is_ping() && !self.is_autocomplete()
    }
}

/// The body of an interaction callback: `{ "type": kind, "data": data }`. `T` is usually a builder
/// from `dr_builders`, already validated by the caller.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InteractionResponse<T> {
    /// Callback kind.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: InteractionResponseType,
    /// Callback data, absent for a bare deferred update.
    #[serde(default = "Option::default", skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> InteractionResponse<T> {
    /// A callback of the given kind.
    pub fn new(kind: InteractionResponseType, data: Option<T>) -> Self {
        Self { kind, data }
    }

    /// Answers with a message.
    pub fn message(data: T) -> Self {
        Self::new(InteractionResponseType::CHANNEL_MESSAGE_WITH_SOURCE, Some(data))
    }

    /// Answers a component or modal submission by editing the message it is attached to.
    pub fn update(data: T) -> Self {
        Self::new(InteractionResponseType::UPDATE_MESSAGE, Some(data))
    }

    /// Answers with a modal. `T` is a [`ModalResponseData`] or a builder of the same form.
    pub fn modal(data: T) -> Self {
        Self::new(InteractionResponseType::MODAL, Some(data))
    }
}

impl InteractionResponse<()> {
    /// Acknowledges a ping.
    pub fn pong() -> Self {
        Self::new(InteractionResponseType::PONG, None)
    }

    /// Acknowledges a component or modal submission with no visible change for now.
    pub fn deferred_update() -> Self {
        Self::new(InteractionResponseType::DEFERRED_MESSAGE_UPDATE, None)
    }
}

impl InteractionResponse<DeferredResponseData> {
    /// Acknowledges now and answers later: the user sees a loading state. `flags` is what makes the
    /// answer ephemeral, and nothing else can be set here.
    pub fn deferred(flags: Option<MessageFlags>) -> Self {
        Self::new(
            InteractionResponseType::DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE,
            flags.map(|flags| DeferredResponseData { flags: Some(flags) }),
        )
    }
}

impl<C> InteractionResponse<AutocompleteResponseData<C>> {
    /// Answers an autocomplete request with suggestions.
    pub fn autocomplete(choices: Vec<C>) -> Self {
        Self::new(
            InteractionResponseType::APPLICATION_COMMAND_AUTOCOMPLETE_RESULT,
            Some(AutocompleteResponseData { choices }),
        )
    }
}

/// Encodes only: the caller validates `data` first, since `T` is opaque here.
impl<T: Serialize> IntoBody for InteractionResponse<T> {
    fn into_body(self) -> Result<RequestBody, BodyError> {
        RequestBody::json(&self)
    }
}

/// The data of a deferred message callback, which holds only `flags`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct DeferredResponseData {
    /// Flags of the coming answer. `EPHEMERAL` is the one that matters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
}

/// The data of an autocomplete callback. `C` is one suggestion: `name`, `value`, and optionally
/// `name_localizations`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AutocompleteResponseData<C> {
    /// The suggestions, at most 25.
    pub choices: Vec<C>,
}

/// The data of a modal callback. Upstream: `APIModalInteractionResponseCallbackData`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModalResponseData {
    /// Identifier sent back on submit.
    #[serde(default)]
    pub custom_id: String,
    /// Title of the popup.
    #[serde(default)]
    pub title: String,
    /// One to five components.
    #[serde(default, deserialize_with = "null_default")]
    pub components: Vec<Component>,
}
