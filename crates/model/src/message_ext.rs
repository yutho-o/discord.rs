//! Ported from discord.js packages/discord.js/src/structures/{Message,Embed,Attachment,Emoji}.js,
//! packages/discord.js/src/util/Util.js (`parseEmoji`, `resolvePartialEmoji`),
//! packages/util/src/functions/embedLength.ts and packages/formatters/src/formatters.ts
//! (`formatEmoji`, `messageLink`), all Apache-2.0.
//! Changed: translated to Rust; getters that need the cache or permissions live on the facade's
//! `Bound<Message>`; malformed input gives `None` rather than an exception.
//!
//! Hand-written helpers on the payload structs, separate from the plain data the generator will emit.

use crate::{
    Attachment, AttachmentFlags, ChannelId, Embed, Emoji, EmojiId, GuildId, Message, MessageId, MessageType, User,
};
use dr_core::{Id, Patch};
use serde::de::{Deserialize, Deserializer, Error as _};
use std::cell::Cell;
use std::fmt;

/// How many recursive fields may nest inside one another. Discord sends one level of
/// `referenced_message` and of `triggering_interaction_metadata`, and a modal submission nests one
/// level of row or label, so this leaves room to spare.
const MAX_NESTING: u32 = 8;

thread_local! {
    /// Recursive fields open on this thread. Decoding is synchronous, so no task switch can happen
    /// while a count is held.
    static NESTING: Cell<u32> = const { Cell::new(0) };
}

/// Lowers the count when a level ends, on success, error or unwind.
struct NestingGuard;

impl Drop for NestingGuard {
    fn drop(&mut self) {
        NESTING.with(|n| n.set(n.get().saturating_sub(1)));
    }
}

/// Reads a field that can hold its own struct, failing past [`MAX_NESTING`].
///
/// serde_json stops at 128 levels, but in a debug build one level of `Message` takes so much stack
/// that a thread of 2 MiB, the default for tokio workers and for tests, overflowed first, and a
/// `tagged_union!` arm starts a new parser, so a union holding itself has no limit at all. An overflow
/// aborts the process, and neither an error nor `catch_unwind` could contain it. Here the payload
/// becomes an ordinary decode failure and the shard goes on.
pub(crate) fn bounded_depth<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let depth = NESTING.with(|n| {
        let depth = n.get().saturating_add(1);
        n.set(depth);
        depth
    });
    let _guard = NestingGuard;
    if depth > MAX_NESTING {
        return Err(D::Error::custom("fields nested too deeply"));
    }
    T::deserialize(deserializer)
}

impl Message {
    /// A message with only its ids known and an empty placeholder author (id 0). The cache uses
    /// this as the base for a partial `MESSAGE_UPDATE` when it holds no previous version, since
    /// every event fires whether or not the message was cached.
    pub fn partial(id: MessageId, channel_id: ChannelId) -> Self {
        Self {
            id,
            channel_id,
            guild_id: None,
            channel_type: None,
            author: User::partial(Id::new(0)),
            member: None,
            content: String::new(),
            timestamp: None,
            edited_timestamp: None,
            kind: MessageType::DEFAULT,
            tts: false,
            mention_everyone: false,
            mentions: Vec::new(),
            mention_roles: Vec::new(),
            mention_channels: Vec::new(),
            attachments: Vec::new(),
            embeds: Vec::new(),
            reactions: Vec::new(),
            nonce: None,
            pinned: false,
            webhook_id: None,
            activity: None,
            application_id: None,
            message_reference: None,
            flags: None,
            referenced_message: Patch::Absent,
            interaction_metadata: None,
            interaction: None,
            components: Vec::new(),
            position: None,
            message_snapshots: Vec::new(),
            call: None,
            shared_client_theme: None,
            resolved: None,
        }
    }

    /// True for a message built by [`Message::partial`] and not yet filled in, whose author is the
    /// placeholder with id 0.
    #[doc(alias = "partial")]
    pub fn is_partial(&self) -> bool {
        self.author.id == Id::new(0)
    }

    /// Creation time from the snowflake, in milliseconds since the Unix epoch. This is what
    /// upstream `createdAt` means, not the `timestamp` field.
    #[doc(alias = "createdTimestamp")]
    pub const fn created_at_ms(&self) -> i64 {
        self.id.timestamp_ms()
    }

    /// True when the message carries a guild id. Only gateway payloads do, so a message from REST
    /// reads as false here even when it was sent in a guild.
    #[doc(alias = "inGuild")]
    pub const fn in_guild(&self) -> bool {
        self.guild_id.is_some()
    }

    /// True for a message Discord generated, such as a join notice. Replies and command responses
    /// are not system messages.
    #[doc(alias = "system")]
    pub fn is_system(&self) -> bool {
        !matches!(
            self.kind,
            MessageType::DEFAULT
                | MessageType::REPLY
                | MessageType::CHAT_INPUT_COMMAND
                | MessageType::CONTEXT_MENU_COMMAND
        )
    }

    /// Link to the message in the Discord client.
    pub fn url(&self) -> String {
        message_link(self.channel_id, self.id, self.guild_id)
    }
}

fn message_link(channel: ChannelId, message: MessageId, guild: Option<GuildId>) -> String {
    match guild {
        Some(guild) => format!("https://discord.com/channels/{guild}/{channel}/{message}"),
        None => format!("https://discord.com/channels/@me/{channel}/{message}"),
    }
}

/// Writes the text content, as upstream's `toString` does.
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.content)
    }
}

impl Embed {
    /// Accumulated length of the title, description, field names and values, footer text and author
    /// name, which Discord limits to 6000. Counts UTF-16 units, as upstream's `embedLength` does.
    #[doc(alias = "embedLength")]
    pub fn length(&self) -> usize {
        fn units(s: &str) -> usize {
            s.encode_utf16().count()
        }
        self.title.as_deref().map_or(0, units)
            + self.description.as_deref().map_or(0, units)
            + self.fields.iter().map(|f| units(&f.name) + units(&f.value)).sum::<usize>()
            + self.footer.as_ref().map_or(0, |f| units(&f.text))
            + self.author.as_ref().map_or(0, |a| units(&a.name))
    }

    /// The colour as `#rrggbb`.
    #[doc(alias = "hexColor")]
    pub fn hex_color(&self) -> Option<String> {
        self.color.map(|c| format!("#{c:06x}"))
    }
}

impl Attachment {
    /// True when the attachment is marked as a spoiler.
    #[doc(alias = "spoiler")]
    pub fn is_spoiler(&self) -> bool {
        self.flags.is_some_and(|f| f.contains(AttachmentFlags::IS_SPOILER))
    }
}

impl Emoji {
    /// A unicode emoji, such as `"🔥"`.
    pub fn unicode(name: impl Into<String>) -> Self {
        Self::bare(None, Some(name.into()), false)
    }

    /// A custom emoji, by id and name.
    pub fn custom(id: EmojiId, name: impl Into<String>, animated: bool) -> Self {
        Self::bare(Some(id), Some(name.into()), animated)
    }

    const fn bare(id: Option<EmojiId>, name: Option<String>, animated: bool) -> Self {
        Self {
            id,
            name,
            roles: Vec::new(),
            user: None,
            require_colons: false,
            managed: false,
            animated,
            available: true,
        }
    }

    /// Reads an emoji from text: a bare id, `<:name:id>`, `<a:name:id>`, `name:id`, or a unicode
    /// emoji, percent-encoded or not. `None` for empty text, for text with a colon that holds no
    /// emoji, and for broken percent-encoding, where upstream throws.
    ///
    /// Matches upstream's pattern on purpose: it is not anchored, so `"x <:ab:12345678901234567> y"`
    /// finds the emoji, and an id of the wrong length is dropped rather than rejected, leaving a
    /// name with no id.
    #[doc(alias = "resolvePartialEmoji")]
    #[doc(alias = "parseEmoji")]
    pub fn parse(text: &str) -> Option<Self> {
        if text.is_empty() {
            return None;
        }
        if is_snowflake_text(text) {
            return Some(Self::bare(Some(text.parse().ok()?), None, false));
        }
        if !text.contains('%') {
            return parse_decoded(text);
        }
        parse_decoded(&decode_uri_component(text)?)
    }

    /// The text that names this emoji in a reaction route: `name:id` or `a:name:id` for a custom
    /// emoji, the percent-encoded character for a unicode one. `None` when it has no name, such as
    /// a deleted custom emoji in a reaction, where upstream writes the word `null` instead.
    pub fn identifier(&self) -> Option<String> {
        let name = self.name.as_deref()?;
        Some(match self.id {
            Some(id) if self.animated => format!("a:{name}:{id}"),
            Some(id) => format!("{name}:{id}"),
            None => encode_uri_component(name),
        })
    }
}

/// Writes what renders as the emoji in a message: `<:name:id>`, `<a:name:id>` or the unicode
/// character. A custom emoji with no name gets upstream's placeholder name, `emoji`.
impl fmt::Display for Emoji {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.id {
            Some(id) => {
                let animated = if self.animated { "a" } else { "" };
                write!(f, "<{animated}:{}:{id}>", self.name.as_deref().unwrap_or("emoji"))
            }
            None => f.write_str(self.name.as_deref().unwrap_or_default()),
        }
    }
}

/// `/^\d{17,19}$/`.
fn is_snowflake_text(text: &str) -> bool {
    (17..=19).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit())
}

/// `parseEmoji` once the text is decoded.
fn parse_decoded(text: &str) -> Option<Emoji> {
    if !text.contains(':') {
        return Some(Emoji::unicode(text));
    }
    // The upstream pattern is not anchored, so the first start that matches wins.
    (0..text.len()).find_map(|start| custom_emoji_at(text, start))
}

/// `/<?(?:(?<animated>a):)?(?<name>\w{2,32}):(?<id>\d{17,19})?>?/` at one start position. The
/// trailing `>?` cannot change the outcome, so it is not checked.
fn custom_emoji_at(text: &str, start: usize) -> Option<Emoji> {
    let bytes = text.as_bytes();
    let at = if bytes.get(start) == Some(&b'<') { start + 1 } else { start };
    let animated = bytes.get(at) == Some(&b'a') && bytes.get(at + 1) == Some(&b':');
    if animated && let Some(emoji) = named_emoji_at(text, at + 2, true) {
        return Some(emoji);
    }
    named_emoji_at(text, at, false)
}

/// The `name:id?` tail of the pattern.
fn named_emoji_at(text: &str, at: usize, animated: bool) -> Option<Emoji> {
    let tail = text.as_bytes().get(at..)?;
    let name_len = tail.iter().take_while(|b| b.is_ascii_alphanumeric() || **b == b'_').count();
    if !(2..=32).contains(&name_len) || tail.get(name_len) != Some(&b':') {
        return None;
    }
    let name = text.get(at..at + name_len)?;
    let id_start = at + name_len + 1;
    let digits = text.as_bytes().get(id_start..)?.iter().take_while(|b| b.is_ascii_digit()).count();
    let id = if digits >= 17 { text.get(id_start..id_start + digits.min(19))?.parse().ok() } else { None };
    Some(Emoji::bare(id, Some(name.to_owned()), animated))
}

/// `encodeURIComponent`: everything but `A-Za-z0-9 - _ . ! ~ * ' ( )` becomes `%XX` per UTF-8 byte.
fn encode_uri_component(text: &str) -> String {
    use fmt::Write as _;
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(char::from(byte));
        } else {
            // Writing to a `String` cannot fail.
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

/// `decodeURIComponent`. `None` where upstream throws: a bad escape, or bytes that are not UTF-8.
fn decode_uri_component(text: &str) -> Option<String> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = char::from(bytes.next()?).to_digit(16)?;
            let low = char::from(bytes.next()?).to_digit(16)?;
            out.push(u8::try_from(high * 16 + low).ok()?);
        } else {
            out.push(byte);
        }
    }
    String::from_utf8(out).ok()
}
