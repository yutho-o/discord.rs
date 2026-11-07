//! The emojis and stickers of one guild, and the standard stickers.

use crate::{Bound, Call, Client};
use dr_builders::{CreateEmoji, CreateSticker, EditEmoji, EditSticker};
use dr_model::{Emoji, EmojiId, GuildId, Sticker, StickerId, StickerPack, StickerPackId, StickerPacks};

/// The custom emojis of one guild. Get one from `GuildHandle::emojis`.
#[derive(Clone, Debug)]
pub struct EmojisManager {
    client: Client,
    guild: GuildId,
}

impl EmojisManager {
    pub(crate) fn new(client: Client, guild: GuildId) -> Self {
        Self { client, guild }
    }

    /// The emojis from the cached guild, empty when it is not cached. No I/O.
    pub fn cached(&self) -> Vec<Emoji> {
        self.client.cache().guild(self.guild).map_or_else(Vec::new, |g| g.emojis.clone())
    }

    /// One emoji from the cached guild. No I/O.
    pub fn get(&self, id: EmojiId) -> Option<Emoji> {
        self.client.cache().guild(self.guild)?.emojis.iter().find(|e| e.id == Some(id)).cloned()
    }

    /// Every emoji of this guild, from Discord.
    pub fn fetch_all(&self) -> Call<Vec<Bound<Emoji>>> {
        Call::new(self.client.clone(), self.client.rest().expressions().list_guild_emojis(self.guild))
    }

    /// One emoji, from Discord. Unlike the cached one, it names who uploaded it, given the
    /// permission to manage it.
    pub fn fetch(&self, id: EmojiId) -> Call<Bound<Emoji>> {
        Call::new(self.client.clone(), self.client.rest().expressions().get_guild_emoji(self.guild, id))
    }

    /// Uploads an emoji. Needs `CREATE_GUILD_EXPRESSIONS`. Use `.reason(...)` for the audit log.
    pub fn create(&self, body: CreateEmoji) -> Call<Bound<Emoji>> {
        Call::new(self.client.clone(), self.client.rest().expressions().create_guild_emoji(self.guild, body))
    }

    /// Edits an emoji. Needs `MANAGE_GUILD_EXPRESSIONS`, or to have uploaded it with
    /// `CREATE_GUILD_EXPRESSIONS`.
    pub fn edit(&self, id: EmojiId, body: EditEmoji) -> Call<Bound<Emoji>> {
        Call::new(self.client.clone(), self.client.rest().expressions().modify_guild_emoji(self.guild, id, body))
    }

    /// Deletes an emoji, with the same permissions as an edit.
    pub fn delete(&self, id: EmojiId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().expressions().delete_guild_emoji(self.guild, id))
    }
}

/// The stickers of one guild. Get one from `GuildHandle::stickers`.
#[derive(Clone, Debug)]
pub struct StickersManager {
    client: Client,
    guild: GuildId,
}

impl StickersManager {
    pub(crate) fn new(client: Client, guild: GuildId) -> Self {
        Self { client, guild }
    }

    /// The stickers from the cached guild, empty when it is not cached. No I/O.
    pub fn cached(&self) -> Vec<Sticker> {
        self.client.cache().guild(self.guild).map_or_else(Vec::new, |g| g.stickers.clone())
    }

    /// One sticker from the cached guild. No I/O.
    pub fn get(&self, id: StickerId) -> Option<Sticker> {
        self.client.cache().guild(self.guild)?.stickers.iter().find(|s| s.id == id).cloned()
    }

    /// Every sticker of this guild, from Discord.
    pub fn fetch_all(&self) -> Call<Vec<Bound<Sticker>>> {
        Call::new(self.client.clone(), self.client.rest().expressions().list_guild_stickers(self.guild))
    }

    /// One sticker, from Discord.
    pub fn fetch(&self, id: StickerId) -> Call<Bound<Sticker>> {
        Call::new(self.client.clone(), self.client.rest().expressions().get_guild_sticker(self.guild, id))
    }

    /// Uploads a sticker. Needs `CREATE_GUILD_EXPRESSIONS`. Use `.reason(...)` for the audit log.
    pub fn create(&self, body: CreateSticker) -> Call<Bound<Sticker>> {
        Call::new(self.client.clone(), self.client.rest().expressions().create_guild_sticker(self.guild, body))
    }

    /// Edits a sticker. Needs `MANAGE_GUILD_EXPRESSIONS`, or to have uploaded it with
    /// `CREATE_GUILD_EXPRESSIONS`.
    pub fn edit(&self, id: StickerId, body: EditSticker) -> Call<Bound<Sticker>> {
        Call::new(self.client.clone(), self.client.rest().expressions().modify_guild_sticker(self.guild, id, body))
    }

    /// Deletes a sticker, with the same permissions as an edit.
    pub fn delete(&self, id: StickerId) -> Call<()> {
        Call::new(self.client.clone(), self.client.rest().expressions().delete_guild_sticker(self.guild, id))
    }
}

impl Client {
    /// Any sticker the bot can see, standard or from a guild, from Discord.
    pub fn fetch_sticker(&self, id: StickerId) -> Call<Bound<Sticker>> {
        Call::new(self.clone(), self.rest().expressions().get_sticker(id))
    }

    /// The packs of standard stickers, from Discord.
    pub fn fetch_sticker_packs(&self) -> Call<Bound<StickerPacks>> {
        Call::new(self.clone(), self.rest().expressions().list_sticker_packs())
    }

    /// One pack of standard stickers, from Discord.
    pub fn fetch_sticker_pack(&self, id: StickerPackId) -> Call<Bound<StickerPack>> {
        Call::new(self.clone(), self.rest().expressions().get_sticker_pack(id))
    }
}
