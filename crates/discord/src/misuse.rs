//! The misuse catalogue of `ARCHITECTURE.md` 9.5, as `compile_fail` doctests: each of these must fail
//! to compile. `#[cfg(doctest)]` only, so it is not part of the API.
//!
//! These stand in for the upstream plan of `trybuild`, whose `.stderr` files are
//! brittle and need their own target. Stable does not check the error codes below. To check them,
//! run `RUSTC_BOOTSTRAP=1 cargo test --doc -p discord-rust`.

/// Ids of different kinds are different types, so swapped arguments do not compile.
///
/// ```compile_fail,E0308
/// use discord::prelude::*;
/// fn take(_channel: ChannelId, _message: MessageId) {}
/// fn misuse(channel: ChannelId, message: MessageId) {
///     take(message, channel);
/// }
/// ```
pub struct SwappedIds;

/// Ids of different kinds cannot be compared.
///
/// ```compile_fail,E0308
/// use discord::prelude::*;
/// fn misuse(user: UserId, guild: GuildId) -> bool {
///     user == guild
/// }
/// ```
pub struct WrongIdMarkerCompare;

/// A request does nothing until it is awaited, and forgetting that is an error here.
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use discord::prelude::*;
/// fn misuse(msg: Bound<Message>) {
///     msg.reply("hi");
/// }
/// ```
pub struct ForgotAwait;

/// State can be set only once.
///
/// ```compile_fail,E0599
/// use discord::prelude::*;
/// let _ = Client::builder("t").state(1_u32).state(2_u32);
/// ```
pub struct StateTwice;

/// A command with subcommands cannot take options.
///
/// ```compile_fail,E0599
/// use discord::prelude::*;
/// let _ = SlashCommandBuilder::new("a", "b")
///     .subcommand(Subcommand::new("c", "d"))
///     .string_option("e", "f", |o| o);
/// ```
pub struct OptionAfterSubcommand;

/// A command with options cannot take subcommands.
///
/// ```compile_fail,E0599
/// use discord::prelude::*;
/// let _ = SlashCommandBuilder::new("a", "b")
///     .string_option("e", "f", |o| o)
///     .subcommand(Subcommand::new("c", "d"));
/// ```
pub struct SubcommandAfterOption;

/// An edit does not take the body of a new message, which would send fields you did not ask for.
///
/// ```compile_fail,E0308
/// use discord::prelude::*;
/// fn misuse(msg: Bound<Message>) {
///     let _ = msg.edit(CreateMessage::new());
/// }
/// ```
pub struct EditWithCreateBody;

/// A handler must return a result.
///
/// ```compile_fail,E0271
/// use discord::prelude::*;
/// let _ = Client::builder("t").on(MessageCreate, |_ctx, _msg| async move {});
/// ```
pub struct MissingOk;

/// A handler's future must be `Send`, so a non-`Send` value cannot live across an await.
///
/// ```compile_fail
/// use discord::prelude::*;
/// use std::rc::Rc;
/// let _ = Client::builder("t").on(MessageCreate, |_ctx, msg| async move {
///     let held = Rc::new(1);
///     msg.reply("hi").await?;
///     drop(held);
///     Ok(())
/// });
/// ```
pub struct NotSend;
