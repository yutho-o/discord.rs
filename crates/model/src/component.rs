//! Ported from discord.js packages/discord.js/src/util/Components.js and structures/Component.js
//! (Apache-2.0), and discord-api-types payloads/v10/message.ts and utils/v10.ts (MIT).
//! Changed: translated to Rust; classes replaced by one `tagged_union!` tree, ARCHITECTURE.md 7.4.
//!
//! Message components: a tree of `tagged_union!` with an unknown arm.
//!
//! Scope so far: action rows, buttons, the five select menus and text input are
//! typed. Every other kind (Components V2, labels, file upload, radio and checkbox groups) goes
//! through `Component::Unknown`, which re-serializes byte-exact, so those kinds survive a decode and
//! an edit unchanged.
//!
//! Builders go through the `new` of each type and never a struct literal, so that they keep compiling
//! whatever fields are added.

use crate::{ButtonStyle, ChannelType, Emoji, EmojiId, GenericId, TextInputStyle};
use dr_core::{null_default, open_str, tagged_union};
use serde::de::{Deserializer, Error as _};
use serde::{Deserialize, Serialize};
use std::cell::Cell;

tagged_union! {
    #[tag = "type"]
    /// A message component. An unknown kind round-trips byte-exact through `Unknown`.
    pub enum Component {
        1 => ActionRow(ActionRow),
        2 => Button(Button),
        3 => StringSelect(StringSelect),
        4 => TextInput(TextInput),
        5 => UserSelect(AutoPopulatedSelect),
        6 => RoleSelect(AutoPopulatedSelect),
        7 => MentionableSelect(AutoPopulatedSelect),
        8 => ChannelSelect(ChannelSelect),
        _ => Unknown,
    }
}

impl Component {
    /// The optional numeric identifier, from `APIBaseComponent`. Read from the raw JSON on `Unknown`.
    pub fn id(&self) -> Option<i32> {
        match self {
            Self::ActionRow(c) => c.id,
            Self::Button(c) => c.id,
            Self::StringSelect(c) => c.id,
            Self::TextInput(c) => c.id,
            Self::UserSelect(c) | Self::RoleSelect(c) | Self::MentionableSelect(c) => c.id,
            Self::ChannelSelect(c) => c.id,
            Self::Unknown { raw, .. } => {
                #[derive(Deserialize)]
                struct Shape {
                    #[serde(default)]
                    id: Option<i32>,
                }
                serde_json::from_str::<Shape>(raw.get()).ok().and_then(|s| s.id)
            }
        }
    }

    /// The `custom_id` of a typed component that has one. `None` on an action row, a link or premium
    /// button, and `Unknown`. [`find_component_by_custom_id`] also reads `Unknown`.
    #[doc(alias = "customId")]
    pub fn custom_id(&self) -> Option<&str> {
        match self {
            Self::ActionRow(_) | Self::Unknown { .. } => None,
            Self::Button(c) => c.custom_id.as_deref(),
            Self::StringSelect(c) => Some(&c.custom_id),
            Self::TextInput(c) => Some(&c.custom_id),
            Self::UserSelect(c) | Self::RoleSelect(c) | Self::MentionableSelect(c) => Some(&c.custom_id),
            Self::ChannelSelect(c) => Some(&c.custom_id),
        }
    }

    /// Whether this component's `custom_id`, typed or raw, is exactly `custom_id`.
    fn has_custom_id(&self, custom_id: &str) -> bool {
        match self {
            Self::Unknown { raw, .. } => {
                #[derive(Deserialize)]
                struct Shape {
                    #[serde(default)]
                    custom_id: Option<String>,
                }
                serde_json::from_str::<Shape>(raw.get()).is_ok_and(|s| s.custom_id.as_deref() == Some(custom_id))
            }
            typed => typed.custom_id() == Some(custom_id),
        }
    }
}

/// Wire tag of a section, which `Component` keeps as `Unknown`.
const SECTION: u32 = 9;
/// Wire tag of a container, which `Component` keeps as `Unknown`.
const CONTAINER: u32 = 17;
/// How many containers [`find_component_by_custom_id`] descends through. Discord allows no nesting
/// at all; a limit keeps wire data from choosing the depth of a recursion.
const MAX_CONTAINER_DEPTH: u8 = 8;

/// Finds a component by `custom_id` in a component tree, depth first in document order.
///
/// Same flattening as upstream's `findComponentByCustomId`: an action row yields its children, a
/// section yields its text displays and its accessory, a container is walked recursively, and
/// anything else is checked directly. A child row of a row is not walked. Sections and containers
/// are `Unknown` arms here, so they are parsed from their raw JSON, which is why the result is owned.
/// One that does not parse is skipped.
#[doc(alias = "findComponentByCustomId")]
pub fn find_component_by_custom_id(components: &[Component], custom_id: &str) -> Option<Component> {
    components.iter().find_map(|c| find_in(c, custom_id, 0))
}

fn find_in(component: &Component, custom_id: &str, depth: u8) -> Option<Component> {
    #[derive(Deserialize)]
    struct Shape {
        #[serde(default, deserialize_with = "null_default")]
        components: Vec<Component>,
        #[serde(default)]
        accessory: Option<Component>,
    }
    match component {
        Component::ActionRow(row) => row.components.iter().find(|c| c.has_custom_id(custom_id)).cloned(),
        Component::Unknown { kind: SECTION, raw } => {
            let section: Shape = serde_json::from_str(raw.get()).ok()?;
            section.components.into_iter().chain(section.accessory).find(|c| c.has_custom_id(custom_id))
        }
        Component::Unknown { kind: CONTAINER, raw } if depth < MAX_CONTAINER_DEPTH => {
            let container: Shape = serde_json::from_str(raw.get()).ok()?;
            container.components.iter().find_map(|c| find_in(c, custom_id, depth + 1))
        }
        Component::Unknown { kind: CONTAINER, .. } => None,
        other => other.has_custom_id(custom_id).then(|| other.clone()),
    }
}

/// How many action rows may nest inside each other in a decode. Discord allows no nesting at all.
const MAX_ROW_DEPTH: u8 = 16;

thread_local! {
    /// How many `ActionRow` decodes are running on this thread.
    static ROW_DEPTH: Cell<u8> = const { Cell::new(0) };
}

/// Counts one running `ActionRow` decode, and releases it on drop.
struct RowDepthGuard;

impl RowDepthGuard {
    fn enter() -> Option<Self> {
        ROW_DEPTH.with(|depth| {
            let current = depth.get();
            (current < MAX_ROW_DEPTH).then(|| {
                depth.set(current + 1);
                Self
            })
        })
    }
}

impl Drop for RowDepthGuard {
    fn drop(&mut self) {
        ROW_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// Reads the children of a row, or `null` as none, under a nesting limit.
///
/// Each `tagged_union!` arm starts a fresh parser, which resets serde_json's own recursion limit, so
/// without this limit a deeply nested payload overflows the stack and aborts the process.
fn row_children<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Component>, D::Error> {
    let _depth = RowDepthGuard::enter().ok_or_else(|| D::Error::custom("action rows nested too deeply"))?;
    null_default(d)
}

/// A row of components.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ActionRow {
    /// Optional numeric identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// Children of the row.
    #[serde(default, deserialize_with = "row_children")]
    pub components: Vec<Component>,
}

impl ActionRow {
    /// A row holding `components`.
    pub fn new(components: Vec<Component>) -> Self {
        Self { id: None, components }
    }
}

/// The small emoji form used on buttons and select options, upstream `APIMessageComponentEmoji`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MessageComponentEmoji {
    /// Id of a custom emoji. A unicode emoji has none, and Discord may send `null`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<EmojiId>,
    /// Name, or the unicode character.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Whether it is animated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animated: Option<bool>,
}

impl MessageComponentEmoji {
    /// A unicode emoji.
    pub fn unicode(character: impl Into<String>) -> Self {
        Self { name: Some(character.into()), ..Self::default() }
    }

    /// A custom emoji.
    pub fn custom(id: EmojiId, name: impl Into<String>) -> Self {
        Self { id: Some(id), name: Some(name.into()), ..Self::default() }
    }
}

/// Keeps what a component can carry, so `Emoji::parse` gives an emoji for a button.
impl From<Emoji> for MessageComponentEmoji {
    fn from(emoji: Emoji) -> Self {
        Self { id: emoji.id, name: emoji.name, animated: emoji.animated.then_some(true) }
    }
}

/// A button. Which fields mean something depends on `style`: see [`Button::view`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Button {
    /// Optional numeric identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// Style. `LINK` carries `url`, `PREMIUM` carries `sku_id`, the others carry `custom_id`.
    pub style: ButtonStyle,
    /// Text on the button. Not on a premium button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Emoji on the button. Not on a premium button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<MessageComponentEmoji>,
    /// Identifier sent back on a click.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<String>,
    /// Target of a link button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The purchasable SKU of a premium button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sku_id: Option<GenericId>,
    /// Whether it is greyed out.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
}

impl Button {
    /// A button of the given style with every other field empty.
    pub fn new(style: ButtonStyle) -> Self {
        Self { id: None, style, label: None, emoji: None, custom_id: None, url: None, sku_id: None, disabled: false }
    }

    /// Whether this is a link button.
    #[doc(alias = "isLinkButton")]
    pub fn is_link(&self) -> bool {
        self.style == ButtonStyle::LINK
    }

    /// Whether a click sends an interaction: any style but link and premium, as in upstream's guard,
    /// so a style this build does not know counts.
    #[doc(alias = "isInteractionButton")]
    pub fn is_interaction(&self) -> bool {
        !matches!(self.style, ButtonStyle::LINK | ButtonStyle::PREMIUM)
    }

    /// This button by style, with the fields that style requires, upstream's
    /// `APIButtonComponentWith{CustomId,URL,SKUId}`.
    pub fn view(&self) -> ButtonView<'_> {
        let label = self.label.as_deref();
        let emoji = self.emoji.as_ref();
        match (self.style, &self.custom_id, &self.url, self.sku_id) {
            (ButtonStyle::LINK, _, Some(url), _) => ButtonView::Link { url, label, emoji },
            (ButtonStyle::PREMIUM, _, _, Some(sku_id)) => ButtonView::Premium { sku_id },
            (ButtonStyle::LINK | ButtonStyle::PREMIUM, ..) | (_, None, ..) => {
                ButtonView::Malformed { style: self.style }
            }
            (style, Some(custom_id), ..) => ButtonView::Interaction { style, custom_id, label, emoji },
        }
    }
}

/// A [`Button`] by style.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum ButtonView<'a> {
    /// A click sends an interaction. Any style but link and premium, known or not.
    Interaction {
        /// The style.
        style: ButtonStyle,
        /// Identifier sent back on a click.
        custom_id: &'a str,
        /// Text on the button.
        label: Option<&'a str>,
        /// Emoji on the button.
        emoji: Option<&'a MessageComponentEmoji>,
    },
    /// A click opens a URL, and the bot hears nothing.
    Link {
        /// The target.
        url: &'a str,
        /// Text on the button.
        label: Option<&'a str>,
        /// Emoji on the button.
        emoji: Option<&'a MessageComponentEmoji>,
    },
    /// A click starts a purchase.
    Premium {
        /// The purchasable SKU.
        sku_id: GenericId,
    },
    /// The field its style requires is absent. Discord never sends this today.
    Malformed {
        /// The style.
        style: ButtonStyle,
    },
}

/// A select menu of strings.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct StringSelect {
    /// Optional numeric identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// Identifier sent back on a choice.
    #[serde(default)]
    pub custom_id: String,
    /// The choices.
    #[serde(default, deserialize_with = "null_default")]
    pub options: Vec<SelectOption>,
    /// Text shown when nothing is chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Fewest choices allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_values: Option<u8>,
    /// Most choices allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_values: Option<u8>,
    /// Whether it is greyed out.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    /// Whether it must be answered in a modal. Discord's default is yes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

impl StringSelect {
    /// A select with the given id and every other field empty.
    pub fn new(custom_id: impl Into<String>) -> Self {
        Self { custom_id: custom_id.into(), ..Self::default() }
    }
}

/// One choice in a select menu.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SelectOption {
    /// What the user sees.
    #[serde(default)]
    pub label: String,
    /// What the bot receives.
    #[serde(default)]
    pub value: String,
    /// More text under the label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Emoji beside the label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<MessageComponentEmoji>,
    /// Whether it starts chosen.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
}

impl SelectOption {
    /// A choice with the given label and value, and every other field empty.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self { label: label.into(), value: value.into(), ..Self::default() }
    }
}

open_str! {
    /// What a preselected value of an auto-populated select refers to.
    SelectMenuDefaultValueType {
        /// A channel.
        Channel = "channel",
        /// A role.
        Role = "role",
        /// A user.
        User = "user",
    }
}

/// A preselected value of an auto-populated select.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SelectMenuDefaultValue {
    /// What `id` refers to.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: SelectMenuDefaultValueType,
    /// The user, role or channel.
    pub id: GenericId,
}

impl SelectMenuDefaultValue {
    /// A preselected value.
    pub fn new(kind: SelectMenuDefaultValueType, id: GenericId) -> Self {
        Self { kind, id }
    }
}

/// A user, role or mentionable select, whose choices Discord fills in. The arm of [`Component`] says
/// which one.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AutoPopulatedSelect {
    /// Optional numeric identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// Identifier sent back on a choice.
    #[serde(default)]
    pub custom_id: String,
    /// Text shown when nothing is chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Fewest choices allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_values: Option<u8>,
    /// Most choices allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_values: Option<u8>,
    /// Whether it is greyed out.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    /// Whether it must be answered in a modal. Discord's default is yes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// What starts chosen.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub default_values: Vec<SelectMenuDefaultValue>,
}

impl AutoPopulatedSelect {
    /// A select with the given id and every other field empty.
    pub fn new(custom_id: impl Into<String>) -> Self {
        Self { custom_id: custom_id.into(), ..Self::default() }
    }
}

/// A channel select.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ChannelSelect {
    /// Optional numeric identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// Identifier sent back on a choice.
    #[serde(default)]
    pub custom_id: String,
    /// Which kinds of channel are offered. Empty means every kind.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub channel_types: Vec<ChannelType>,
    /// Text shown when nothing is chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Fewest choices allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_values: Option<u8>,
    /// Most choices allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_values: Option<u8>,
    /// Whether it is greyed out.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    /// Whether it must be answered in a modal. Discord's default is yes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// What starts chosen.
    #[serde(default, deserialize_with = "null_default", skip_serializing_if = "Vec::is_empty")]
    pub default_values: Vec<SelectMenuDefaultValue>,
}

impl ChannelSelect {
    /// A select with the given id and every other field empty.
    pub fn new(custom_id: impl Into<String>) -> Self {
        Self { custom_id: custom_id.into(), ..Self::default() }
    }
}

/// A text input, in a modal. On a modal submission only `custom_id` and `value` arrive, with `id`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TextInput {
    /// Optional numeric identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// Identifier sent back on submit.
    #[serde(default)]
    pub custom_id: String,
    /// One line or several. Absent on a modal submission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<TextInputStyle>,
    /// Text above the input. Absent on a modal submission, and not allowed inside a label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Shortest allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u16>,
    /// Longest allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u16>,
    /// Whether it must be filled in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// Prefilled text, or on a submission what was typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Text shown while empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

impl TextInput {
    /// A text input with the given id, style and label, and every other field empty.
    pub fn new(custom_id: impl Into<String>, style: TextInputStyle, label: impl Into<String>) -> Self {
        Self { custom_id: custom_id.into(), style: Some(style), label: Some(label.into()), ..Self::default() }
    }
}
