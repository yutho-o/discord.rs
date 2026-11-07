//! `UserHandle`.

use crate::{Bound, Call, Client};
use dr_model::{User, UserId};

/// A user by id: a client and an id, no I/O to create.
#[derive(Clone, Debug)]
pub struct UserHandle {
    pub(crate) client: Client,
    pub(crate) id: UserId,
}

impl Client {
    /// A handle to a user. No I/O.
    pub fn user(&self, id: UserId) -> UserHandle {
        UserHandle { client: self.clone(), id }
    }
}

impl UserHandle {
    /// The id.
    pub fn id(&self) -> UserId {
        self.id
    }

    /// The user from the cache. No I/O.
    pub fn cached(&self) -> Option<Bound<User>> {
        self.client.cache().user(self.id).map(|u| self.client.bind_arc(u))
    }

    /// Fetches the user from Discord.
    pub fn fetch(&self) -> Call<Bound<User>> {
        Call::new(self.client.clone(), self.client.rest().users().get(self.id))
    }
}

impl Bound<User> {
    /// This user, as a handle.
    pub fn handle(&self) -> UserHandle {
        self.client().user(self.id)
    }
}
