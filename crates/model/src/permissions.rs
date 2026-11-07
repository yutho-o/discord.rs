//! Ported from discord.js packages/discord.js/src/structures/GuildMember.js (`permissions`) and
//! src/structures/GuildChannel.js (`memberPermissions`, `overwritesFor`) (Apache-2.0), and from the
//! algorithm in Discord's documentation, "Permissions", "Permission Overwrites".
//! Changed: translated to Rust; free functions over plain data instead of methods on cached
//! structures, and the timeout rule of the documentation applied in [`for_member`].
//!
//! What a member may do, computed the way Discord does:
//!
//! 1. The guild owner has every permission.
//! 2. Otherwise, the base is the `@everyone` role (whose id is the guild id) and each of the
//!    member's roles, combined.
//! 3. `ADMINISTRATOR` in the base gives every permission, whatever the channel says.
//! 4. In a channel, the overwrites apply in order: `@everyone`, then all of the member's roles at
//!    once, then the member. Each one removes its `deny` and then adds its `allow`.
//! 5. A member in timeout keeps only `VIEW_CHANNEL` and `READ_MESSAGE_HISTORY`, unless the owner or an
//!    administrator.
//!
//! A thread has no overwrites of its own: compute with its parent channel's.

use crate::{GuildId, OverwriteType, PermissionOverwrite, Permissions, RoleId, UserId};

/// What a timed out member keeps.
pub const TIMEOUT_ALLOWED: Permissions = Permissions::VIEW_CHANNEL.union(Permissions::READ_MESSAGE_HISTORY);

/// Every permission this build knows, what the owner and administrators get. Unlike
/// `Permissions::all()`, it leaves out the bits Discord has not defined.
pub fn all_known() -> Permissions {
    Permissions::all().iter_names().fold(Permissions::empty(), |acc, (_, flag)| acc | flag)
}

/// Who is asking, in one guild.
#[derive(Clone, Copy, Debug)]
pub struct MemberContext<'a> {
    /// The guild.
    pub guild_id: GuildId,
    /// The guild's owner, if known. Without it, the owner rule cannot apply.
    pub owner_id: Option<UserId>,
    /// The member.
    pub user_id: UserId,
    /// The member's roles, without `@everyone`.
    pub roles: &'a [RoleId],
    /// True while the member is in timeout.
    pub timed_out: bool,
}

/// The guild-wide permissions of a member, steps 1 to 3. `role` gives the permissions of a role of the
/// guild, the `@everyone` role included; an unknown role adds nothing.
pub fn base(member: &MemberContext<'_>, role: impl Fn(RoleId) -> Option<Permissions>) -> Permissions {
    if member.owner_id == Some(member.user_id) {
        return all_known();
    }
    let everyone = role(member.guild_id.cast()).unwrap_or(Permissions::empty());
    let combined = member.roles.iter().filter_map(|r| role(*r)).fold(everyone, |acc, p| acc | p);
    if combined.contains(Permissions::ADMINISTRATOR) { all_known() } else { combined }
}

/// Applies a channel's overwrites to `base`, step 4. `base` with `ADMINISTRATOR` is returned whole.
pub fn apply_overwrites(
    base: Permissions,
    member: &MemberContext<'_>,
    overwrites: &[PermissionOverwrite],
) -> Permissions {
    if base.contains(Permissions::ADMINISTRATOR) {
        return all_known();
    }
    let guild = member.guild_id.get();
    let mut permissions = base;
    if let Some(everyone) = overwrites.iter().find(|o| o.kind == OverwriteType::ROLE && o.id.get() == guild) {
        permissions = (permissions - everyone.deny) | everyone.allow;
    }
    let (mut allow, mut deny) = (Permissions::empty(), Permissions::empty());
    let has_role = |id: u64| id != guild && member.roles.iter().any(|r| r.get() == id);
    for o in overwrites.iter().filter(|o| o.kind == OverwriteType::ROLE && has_role(o.id.get())) {
        allow |= o.allow;
        deny |= o.deny;
    }
    permissions = (permissions - deny) | allow;
    let user = member.user_id.get();
    if let Some(own) = overwrites.iter().find(|o| o.kind == OverwriteType::MEMBER && o.id.get() == user) {
        permissions = (permissions - own.deny) | own.allow;
    }
    permissions
}

/// Everything at once, steps 1 to 5: the guild-wide permissions with `overwrites` `None`, or those in
/// a channel with that channel's overwrites.
pub fn for_member(
    member: &MemberContext<'_>,
    role: impl Fn(RoleId) -> Option<Permissions>,
    overwrites: Option<&[PermissionOverwrite]>,
) -> Permissions {
    let base = base(member, role);
    if base.contains(Permissions::ADMINISTRATOR) {
        return all_known();
    }
    let permissions = overwrites.map_or(base, |o| apply_overwrites(base, member, o));
    if member.timed_out { permissions & TIMEOUT_ALLOWED } else { permissions }
}
