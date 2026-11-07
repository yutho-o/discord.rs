//! User endpoints.

use crate::{Method, Request, Rest};
use dr_core::RequestBody;
use dr_model::{CurrentUser, User, UserId, routes};

/// User endpoints. Get one from [`Rest::users`].
#[derive(Clone, Copy, Debug)]
pub struct UsersApi<'a>(pub(crate) &'a Rest);

impl UsersApi<'_> {
    /// `GET /users/@me`.
    pub fn get_current(&self) -> Request<CurrentUser> {
        Request::new(self.0.clone(), Method::Get, routes::current_user(), Ok(RequestBody::empty()))
    }

    /// `GET /users/{user}`.
    pub fn get(&self, user: UserId) -> Request<User> {
        Request::new(self.0.clone(), Method::Get, routes::user(user), Ok(RequestBody::empty()))
    }
}
