//! `ctx.users()`.

use crate::{Bound, Call, Client, Error};
use dr_model::{CurrentUser, User, UserId};

/// Users. Get one from [`Client::users`].
#[derive(Clone, Debug)]
pub struct UsersManager {
    client: Client,
}

impl Client {
    /// Users, by id.
    pub fn users(&self) -> UsersManager {
        UsersManager { client: self.clone() }
    }
}

impl UsersManager {
    /// The bot's own user from the cache, after `READY`. No I/O.
    pub fn me(&self) -> Option<Bound<CurrentUser>> {
        self.client.cache().me().map(|u| self.client.bind_arc(u))
    }

    /// The bot's own user from Discord.
    pub fn fetch_me(&self) -> Call<Bound<CurrentUser>> {
        Call::new(self.client.clone(), self.client.rest().users().get_current())
    }

    /// The user from the cache. No I/O.
    pub fn get(&self, id: UserId) -> Option<Bound<User>> {
        self.client.cache().user(id).map(|u| self.client.bind_arc(u))
    }

    /// The user from Discord.
    pub fn fetch(&self, id: UserId) -> Call<Bound<User>> {
        Call::new(self.client.clone(), self.client.rest().users().get(id))
    }

    /// The user from the cache, else from Discord.
    pub async fn resolve(&self, id: UserId) -> Result<Bound<User>, Error> {
        if let Some(user) = self.get(id) {
            return Ok(user);
        }
        let fetched = self.fetch(id).await?;
        self.client.cache().insert_user_if_absent(fetched.data().clone());
        Ok(fetched)
    }
}
