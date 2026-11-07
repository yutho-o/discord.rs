//! The reply state machine and the requests behind `reply`, `defer`, `update` and `follow_up`.
//!
//! A reply takes the state with `reserve`, and goes back with `rollback` if the HTTP call fails or
//! the future is dropped, so the user can try again. Two tasks replying at once see one winner and
//! one `AlreadyReplied`.

use crate::error::InteractionError;
use crate::interactions::AutocompleteChoice;
use crate::{Bound, Call, Client, Error};
use dr_builders::{CreateMessage, EditMessage, ModalBuilder};
use dr_core::{Secret, WithFiles};
use dr_model::{ApplicationId, InteractionId, InteractionResponse, Message, MessageFlags};
use futures::future::BoxFuture;
use std::future::IntoFuture;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const OPEN: u8 = 0;
const RESERVED: u8 = 1;
const COMMITTED: u8 = 2;

/// Discord accepts at most this many suggestions.
const MAX_CHOICES: usize = 25;

/// Whether an interaction was answered. Atomic, so the check and the change are one step.
#[derive(Debug, Default)]
pub struct ReplyState(AtomicU8);

impl ReplyState {
    /// Takes the right to answer. Fails with `AlreadyReplied` if another call has it or used it.
    pub fn reserve(&self) -> Result<(), InteractionError> {
        self.0
            .compare_exchange(OPEN, RESERVED, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| InteractionError::AlreadyReplied)
    }

    /// The answer went through.
    pub fn commit(&self) {
        self.0.store(COMMITTED, Ordering::Release);
    }

    /// The answer failed, so the right to answer goes back.
    pub fn rollback(&self) {
        self.0.store(OPEN, Ordering::Release);
    }

    /// True once an answer went through. `edit_reply` and the like check this.
    pub fn is_committed(&self) -> bool {
        self.0.load(Ordering::Acquire) == COMMITTED
    }
}

/// Gives the right to answer back unless it was committed: on a failure and on a dropped future.
struct Reservation<'a> {
    state: &'a ReplyState,
    committed: bool,
}

impl Reservation<'_> {
    fn commit(mut self) {
        self.committed = true;
        self.state.commit();
    }
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.state.rollback();
        }
    }
}

/// What every answer to an interaction needs. Owned, since a request outlives the borrow it came from.
#[derive(Clone, Debug)]
pub(crate) struct ReplyTarget {
    pub(crate) client: Client,
    pub(crate) id: InteractionId,
    pub(crate) application_id: ApplicationId,
    pub(crate) token: Secret,
    pub(crate) state: Arc<ReplyState>,
}

impl ReplyTarget {
    /// `NotReplied` before the first answer went through.
    fn after_reply<T: crate::Bindable>(&self, make: impl FnOnce(&Self) -> Call<T>) -> Call<T> {
        if self.state.is_committed() {
            make(self)
        } else {
            Call::failed(self.client.clone(), InteractionError::NotReplied.into())
        }
    }

    pub(crate) fn edit_reply(&self, body: EditMessage) -> Call<Bound<Message>> {
        self.after_reply(|t| {
            let request = t.client.rest().interactions().edit_reply(t.application_id, &t.token, body);
            Call::new(t.client.clone(), request)
        })
    }

    pub(crate) fn fetch_reply(&self) -> Call<Bound<Message>> {
        self.after_reply(|t| {
            let request = t.client.rest().interactions().get_original_reply(t.application_id, &t.token);
            Call::new(t.client.clone(), request)
        })
    }

    pub(crate) fn delete_reply(&self) -> Call<()> {
        self.after_reply(|t| {
            let request = t.client.rest().interactions().delete_reply(t.application_id, &t.token);
            Call::new(t.client.clone(), request)
        })
    }
}

/// What an interaction callback carries.
#[derive(Clone, Debug)]
pub(crate) enum ReplyBody {
    /// Type 4.
    Message(CreateMessage),
    /// Type 5.
    Defer { ephemeral: bool },
    /// Type 6.
    DeferUpdate,
    /// Type 7.
    Update(EditMessage),
    /// Type 8.
    Autocomplete(Vec<AutocompleteChoice>),
    /// Type 9.
    Modal(ModalBuilder),
}

/// The first answer to an interaction, with options that change its body before any I/O. `Ok` means
/// Discord accepted it. A failure rolls the state back, so that the answer can be tried again.
#[must_use = "a request does nothing until it is awaited"]
#[derive(Debug)]
pub struct ReplyRequest {
    target: ReplyTarget,
    body: ReplyBody,
}

impl ReplyRequest {
    pub(crate) fn new(target: ReplyTarget, body: ReplyBody) -> Self {
        Self { target, body }
    }

    /// Only the person who triggered the interaction sees the answer. Does nothing on an `update`, a
    /// modal or an autocomplete answer, where Discord has no such flag.
    pub fn ephemeral(mut self) -> Self {
        self.body = match self.body {
            ReplyBody::Message(m) => ReplyBody::Message(m.ephemeral()),
            ReplyBody::Defer { .. } => ReplyBody::Defer { ephemeral: true },
            other => other,
        };
        self
    }

    /// Pings nobody.
    pub fn no_mentions(mut self) -> Self {
        self.body = match self.body {
            ReplyBody::Message(m) => ReplyBody::Message(m.no_mentions()),
            ReplyBody::Update(m) => ReplyBody::Update(m.no_mentions()),
            other => other,
        };
        self
    }
}

impl IntoFuture for ReplyRequest {
    type Output = Result<(), Error>;
    type IntoFuture = BoxFuture<'static, Self::Output>;
    fn into_future(self) -> Self::IntoFuture {
        let Self { target, body } = self;
        Box::pin(async move {
            target.state.reserve()?;
            let reservation = Reservation { state: &target.state, committed: false };
            let api = target.client.rest().interactions();
            let (id, token) = (target.id, &target.token);
            let sent = match body {
                ReplyBody::Message(m) => {
                    m.validate()?;
                    let files = m.uploads();
                    api.callback(id, token, WithFiles::new(InteractionResponse::message(m), files)).await
                }
                ReplyBody::Defer { ephemeral } => {
                    let flags = ephemeral.then_some(MessageFlags::EPHEMERAL);
                    api.callback(id, token, InteractionResponse::deferred(flags)).await
                }
                ReplyBody::DeferUpdate => api.callback(id, token, InteractionResponse::deferred_update()).await,
                ReplyBody::Update(m) => {
                    m.validate()?;
                    let files = m.uploads();
                    api.callback(id, token, WithFiles::new(InteractionResponse::update(m), files)).await
                }
                ReplyBody::Autocomplete(mut choices) => {
                    choices.truncate(MAX_CHOICES);
                    api.callback(id, token, InteractionResponse::autocomplete(choices)).await
                }
                ReplyBody::Modal(m) => {
                    m.validate()?;
                    api.callback(id, token, InteractionResponse::modal(m)).await
                }
            };
            match sent {
                Ok(_) => {
                    reservation.commit();
                    Ok(())
                }
                Err(e) => {
                    // Discord applied it even though this call failed, so a retry could only fail.
                    if e.request_succeeded() {
                        reservation.commit();
                    }
                    Err(e.into())
                }
            }
        })
    }
}

/// A further message after the first answer.
#[must_use = "a request does nothing until it is awaited"]
#[derive(Debug)]
pub struct FollowUpRequest {
    target: ReplyTarget,
    body: CreateMessage,
}

impl FollowUpRequest {
    pub(crate) fn new(target: ReplyTarget, body: CreateMessage) -> Self {
        Self { target, body }
    }

    /// Only the person who triggered the interaction sees it.
    pub fn ephemeral(mut self) -> Self {
        self.body = self.body.ephemeral();
        self
    }

    /// Pings nobody.
    pub fn no_mentions(mut self) -> Self {
        self.body = self.body.no_mentions();
        self
    }
}

impl IntoFuture for FollowUpRequest {
    type Output = Result<Bound<Message>, Error>;
    type IntoFuture = BoxFuture<'static, Self::Output>;
    fn into_future(self) -> Self::IntoFuture {
        let Self { target, body } = self;
        Box::pin(async move {
            if !target.state.is_committed() {
                return Err(InteractionError::NotReplied.into());
            }
            let message =
                target.client.rest().interactions().follow_up(target.application_id, &target.token, body).await?;
            Ok(target.client.bind(message))
        })
    }
}
