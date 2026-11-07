//! The permission algorithm, case by case, against Discord's documentation.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dr_model::permissions::{MemberContext, TIMEOUT_ALLOWED, all_known, apply_overwrites, base, for_member};
use dr_model::{GuildId, PermissionOverwrite, Permissions as P, RoleId, UserId};

const GUILD: GuildId = GuildId::new(1);
const EVERYONE: RoleId = RoleId::new(1);
const MOD: RoleId = RoleId::new(10);
const MUTED: RoleId = RoleId::new(11);
const ALICE: UserId = UserId::new(100);
const OWNER: UserId = UserId::new(200);

fn roles(id: RoleId) -> Option<P> {
    match id.get() {
        1 => Some(P::VIEW_CHANNEL | P::SEND_MESSAGES | P::READ_MESSAGE_HISTORY),
        10 => Some(P::KICK_MEMBERS | P::MANAGE_MESSAGES),
        11 => Some(P::empty()),
        12 => Some(P::ADMINISTRATOR),
        _ => None,
    }
}

fn alice(member_roles: &[RoleId]) -> MemberContext<'_> {
    MemberContext { guild_id: GUILD, owner_id: Some(OWNER), user_id: ALICE, roles: member_roles, timed_out: false }
}

#[test]
fn the_base_combines_everyone_and_each_role() {
    assert_eq!(base(&alice(&[]), roles), P::VIEW_CHANNEL | P::SEND_MESSAGES | P::READ_MESSAGE_HISTORY);
    let with_mod = base(&alice(&[MOD]), roles);
    assert!(with_mod.contains(P::KICK_MEMBERS | P::SEND_MESSAGES));
    assert_eq!(base(&alice(&[RoleId::new(999)]), roles), base(&alice(&[]), roles), "an unknown role adds nothing");
}

#[test]
fn the_owner_and_administrators_get_every_known_permission() {
    let owner = MemberContext { user_id: OWNER, ..alice(&[]) };
    assert_eq!(base(&owner, roles), all_known());
    assert_eq!(base(&alice(&[RoleId::new(12)]), roles), all_known());
    assert!(all_known().contains(P::ADMINISTRATOR | P::VIEW_CHANNEL));
    assert_ne!(all_known(), P::all(), "undefined bits stay off");

    let deny_all = [PermissionOverwrite::member(OWNER, P::empty(), all_known())];
    assert_eq!(for_member(&owner, roles, Some(&deny_all)), all_known(), "overwrites cannot limit the owner");
}

#[test]
fn overwrites_apply_everyone_then_roles_then_the_member() {
    let member_roles = [MOD, MUTED];
    let me = alice(&member_roles);
    let start = base(&me, roles);
    let hide = [PermissionOverwrite::role(EVERYONE, P::empty(), P::VIEW_CHANNEL)];
    assert!(!apply_overwrites(start, &me, &hide).contains(P::VIEW_CHANNEL));

    // A role allow beats an @everyone deny.
    let staff = [
        PermissionOverwrite::role(EVERYONE, P::empty(), P::VIEW_CHANNEL),
        PermissionOverwrite::role(MOD, P::VIEW_CHANNEL, P::empty()),
    ];
    assert!(apply_overwrites(start, &me, &staff).contains(P::VIEW_CHANNEL));

    // Between roles, allow wins over deny, whatever the order.
    let both = [
        PermissionOverwrite::role(MUTED, P::empty(), P::SEND_MESSAGES),
        PermissionOverwrite::role(MOD, P::SEND_MESSAGES, P::empty()),
    ];
    assert!(apply_overwrites(start, &me, &both).contains(P::SEND_MESSAGES));

    // The member's own overwrite comes last.
    let mut personal = both.to_vec();
    personal.push(PermissionOverwrite::member(ALICE, P::empty(), P::SEND_MESSAGES));
    assert!(!apply_overwrites(start, &me, &personal).contains(P::SEND_MESSAGES));

    // Someone else's overwrite and a role the member lacks change nothing.
    let others = [
        PermissionOverwrite::member(OWNER, P::empty(), P::SEND_MESSAGES),
        PermissionOverwrite::role(RoleId::new(13), P::empty(), P::SEND_MESSAGES),
    ];
    assert_eq!(apply_overwrites(start, &me, &others), start);
}

#[test]
fn a_member_overwrite_with_the_guild_id_is_not_the_everyone_role() {
    let me = MemberContext { user_id: UserId::new(GUILD.get()), ..alice(&[]) };
    let only_member = [PermissionOverwrite::member(UserId::new(GUILD.get()), P::empty(), P::SEND_MESSAGES)];
    assert!(!apply_overwrites(base(&me, roles), &me, &only_member).contains(P::SEND_MESSAGES));
    let other = alice(&[]);
    assert!(apply_overwrites(base(&other, roles), &other, &only_member).contains(P::SEND_MESSAGES));
}

#[test]
fn a_timeout_leaves_only_reading() {
    let member_roles = [MOD];
    let timed_out = MemberContext { timed_out: true, ..alice(&member_roles) };
    assert_eq!(for_member(&timed_out, roles, None), TIMEOUT_ALLOWED);
    let channel = [PermissionOverwrite::role(EVERYONE, P::empty(), P::VIEW_CHANNEL)];
    assert_eq!(for_member(&timed_out, roles, Some(&channel)), P::READ_MESSAGE_HISTORY);
    let admin_roles = [RoleId::new(12)];
    let admin = MemberContext { timed_out: true, ..alice(&admin_roles) };
    assert_eq!(for_member(&admin, roles, None), all_known(), "a timeout does not limit an administrator");
}

#[test]
fn without_a_known_owner_the_owner_rule_does_not_apply() {
    let me = MemberContext { owner_id: None, user_id: OWNER, ..alice(&[]) };
    assert_eq!(base(&me, roles), base(&alice(&[]), roles));
}
