//! Stickers. Upstream: `payloads/v10/sticker.ts`, `rest/v10/sticker.ts` and
//! `GatewayGuildStickersUpdateDispatchData` in discord-api-types (MIT, (c) 2020 vladfrangu).

use crate::{GuildId, User};
use crate::{SkuId, StickerId, StickerPackId};
use dr_core::{null_default, open_int};
use serde::{Deserialize, Serialize};

open_int! {
    /// Where a sticker comes from.
    StickerType: u16 {
        /// An official sticker in a pack.
        STANDARD = 1,
        /// A sticker uploaded to a guild.
        GUILD = 2,
    }
}

open_int! {
    /// A sticker's image format.
    StickerFormatType: u16 {
        PNG = 1,
        APNG = 2,
        LOTTIE = 3,
        GIF = 4,
    }
}

impl StickerFormatType {
    /// The file extension of the sticker's image on the CDN, `json` for Lottie. `None` for a format
    /// this build does not know.
    pub fn extension(self) -> Option<&'static str> {
        match self {
            Self::PNG | Self::APNG => Some("png"),
            Self::LOTTIE => Some("json"),
            Self::GIF => Some("gif"),
            _ => None,
        }
    }
}

/// A sticker, standard or from a guild.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Sticker {
    /// Its id.
    pub id: StickerId,
    /// The pack of a standard sticker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack_id: Option<StickerPackId>,
    /// Its name.
    pub name: String,
    /// Its description.
    #[serde(default)]
    pub description: Option<String>,
    /// Autocomplete and suggestion tags, up to 200 characters. A guild sticker's is the name of a
    /// unicode emoji, by convention.
    #[serde(default)]
    pub tags: String,
    /// Standard or guild.
    #[serde(rename = "type")]
    #[doc(alias = "type")]
    pub kind: StickerType,
    /// The image format.
    pub format_type: StickerFormatType,
    /// Whether a guild sticker can be used now. It can stop being true when a guild loses Server
    /// Boosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available: Option<bool>,
    /// The guild of a guild sticker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// Who uploaded a guild sticker. Sent only to those with the permission to manage it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// Its place in its pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_value: Option<i64>,
}

/// The smallest form of a sticker, on messages.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StickerItem {
    /// Its id.
    pub id: StickerId,
    /// Its name.
    pub name: String,
    /// The image format.
    pub format_type: StickerFormatType,
}

/// A pack of standard stickers.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StickerPack {
    /// Its id.
    pub id: StickerPackId,
    /// Its stickers.
    #[serde(default, deserialize_with = "null_default")]
    pub stickers: Vec<Sticker>,
    /// Its name.
    pub name: String,
    /// The SKU it is sold as.
    pub sku_id: SkuId,
    /// The sticker shown as the pack's icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_sticker_id: Option<StickerId>,
    /// Its description.
    #[serde(default)]
    pub description: String,
    /// The id of its banner image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_asset_id: Option<crate::GenericId>,
}

/// The answer of `GET /sticker-packs`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StickerPacks {
    /// The packs.
    #[serde(default, deserialize_with = "null_default")]
    pub sticker_packs: Vec<StickerPack>,
}

/// `GUILD_STICKERS_UPDATE`: a guild's stickers changed. It holds the whole new list.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuildStickersUpdateEvent {
    /// The guild.
    pub guild_id: GuildId,
    /// Every sticker the guild has now.
    #[serde(default, deserialize_with = "null_default")]
    pub stickers: Vec<Sticker>,
}
