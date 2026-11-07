//! Emoji and sticker endpoints.

use crate::{Empty, Method, Request, Rest};
use dr_core::{IntoBody, RequestBody};
use dr_model::{Emoji, EmojiId, GuildId, Sticker, StickerId, StickerPack, StickerPackId, StickerPacks, routes};

/// Emoji and sticker endpoints. Get one from [`Rest::expressions`].
#[derive(Clone, Copy, Debug)]
pub struct ExpressionsApi<'a>(pub(crate) &'a Rest);

impl ExpressionsApi<'_> {
    /// `GET /guilds/{guild}/emojis`.
    pub fn list_guild_emojis(&self, guild: GuildId) -> Request<Vec<Emoji>> {
        Request::new(self.0.clone(), Method::Get, routes::guild_emojis(guild), Ok(RequestBody::empty()))
    }

    /// `GET /guilds/{guild}/emojis/{emoji}`.
    pub fn get_guild_emoji(&self, guild: GuildId, emoji: EmojiId) -> Request<Emoji> {
        Request::new(self.0.clone(), Method::Get, routes::guild_emoji(guild, emoji), Ok(RequestBody::empty()))
    }

    /// `POST /guilds/{guild}/emojis`. Use `.reason(...)` for the audit log.
    pub fn create_guild_emoji(&self, guild: GuildId, body: impl IntoBody) -> Request<Emoji> {
        Request::new(self.0.clone(), Method::Post, routes::guild_emojis(guild), body.into_body())
    }

    /// `PATCH /guilds/{guild}/emojis/{emoji}`.
    pub fn modify_guild_emoji(&self, guild: GuildId, emoji: EmojiId, body: impl IntoBody) -> Request<Emoji> {
        Request::new(self.0.clone(), Method::Patch, routes::guild_emoji(guild, emoji), body.into_body())
    }

    /// `DELETE /guilds/{guild}/emojis/{emoji}`.
    pub fn delete_guild_emoji(&self, guild: GuildId, emoji: EmojiId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Delete, routes::guild_emoji(guild, emoji), Ok(RequestBody::empty()))
    }

    /// `GET /guilds/{guild}/stickers`.
    pub fn list_guild_stickers(&self, guild: GuildId) -> Request<Vec<Sticker>> {
        Request::new(self.0.clone(), Method::Get, routes::guild_stickers(guild), Ok(RequestBody::empty()))
    }

    /// `GET /guilds/{guild}/stickers/{sticker}`.
    pub fn get_guild_sticker(&self, guild: GuildId, sticker: StickerId) -> Request<Sticker> {
        Request::new(self.0.clone(), Method::Get, routes::guild_sticker(guild, sticker), Ok(RequestBody::empty()))
    }

    /// `POST /guilds/{guild}/stickers`, a `multipart/form-data` upload.
    pub fn create_guild_sticker(&self, guild: GuildId, body: impl IntoBody) -> Request<Sticker> {
        Request::new(self.0.clone(), Method::Post, routes::guild_stickers(guild), body.into_body())
    }

    /// `PATCH /guilds/{guild}/stickers/{sticker}`.
    pub fn modify_guild_sticker(&self, guild: GuildId, sticker: StickerId, body: impl IntoBody) -> Request<Sticker> {
        Request::new(self.0.clone(), Method::Patch, routes::guild_sticker(guild, sticker), body.into_body())
    }

    /// `DELETE /guilds/{guild}/stickers/{sticker}`.
    pub fn delete_guild_sticker(&self, guild: GuildId, sticker: StickerId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Delete, routes::guild_sticker(guild, sticker), Ok(RequestBody::empty()))
    }

    /// `GET /stickers/{sticker}`, any sticker the bot can see.
    pub fn get_sticker(&self, sticker: StickerId) -> Request<Sticker> {
        Request::new(self.0.clone(), Method::Get, routes::sticker(sticker), Ok(RequestBody::empty()))
    }

    /// `GET /sticker-packs`, the packs of standard stickers.
    pub fn list_sticker_packs(&self) -> Request<StickerPacks> {
        Request::new(self.0.clone(), Method::Get, routes::sticker_packs(), Ok(RequestBody::empty()))
    }

    /// `GET /sticker-packs/{pack}`.
    pub fn get_sticker_pack(&self, pack: StickerPackId) -> Request<StickerPack> {
        Request::new(self.0.clone(), Method::Get, routes::sticker_pack(pack), Ok(RequestBody::empty()))
    }
}
