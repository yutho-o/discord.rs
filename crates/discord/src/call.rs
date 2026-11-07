//! `Call<T>`: a `rest::Request` plus the step that binds its result to the client.

use crate::{Bound, Client, Error};
use dr_rest::{Empty, Request};
use futures::future::BoxFuture;
use serde::de::DeserializeOwned;
use std::future::IntoFuture;
use std::sync::Arc;

/// How a wire result becomes a user value.
pub trait Bindable: Sized + Send + 'static {
    /// What the response body decodes to.
    type Wire: DeserializeOwned + Send + 'static;
    /// Binds it to the client.
    fn bind(client: &Client, wire: Self::Wire) -> Self;
}

impl<T: DeserializeOwned + Send + Sync + 'static> Bindable for Bound<T> {
    type Wire = T;
    fn bind(client: &Client, wire: T) -> Self {
        Bound::new(client.clone(), Arc::new(wire))
    }
}

impl<T: DeserializeOwned + Send + Sync + 'static> Bindable for Vec<Bound<T>> {
    type Wire = Vec<T>;
    fn bind(client: &Client, wire: Vec<T>) -> Self {
        wire.into_iter().map(|w| Bound::new(client.clone(), Arc::new(w))).collect()
    }
}

impl Bindable for () {
    type Wire = Empty;
    fn bind(_: &Client, _: Empty) {}
}

type WireFix<W> = Box<dyn FnOnce(&mut W) + Send>;

/// A lazy REST call that gives a bound value: nothing is sent until it is awaited.
#[must_use = "a request does nothing until it is awaited"]
pub struct Call<T: Bindable> {
    client: Client,
    request: Result<Request<T::Wire>, Error>,
    fix: Option<WireFix<T::Wire>>,
}

impl<T: Bindable> std::fmt::Debug for Call<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Call").finish_non_exhaustive()
    }
}

impl<T: Bindable> Call<T> {
    /// A call that sends this request.
    pub fn new(client: Client, request: Request<T::Wire>) -> Self {
        Self { client, request: Ok(request), fix: None }
    }

    /// A call that fails with `error` when awaited, with nothing sent. For any
    /// method that can tell before sending that it cannot work, such as `Error::NotReady`.
    pub fn failed(client: Client, error: Error) -> Self {
        Self { client, request: Err(error), fix: None }
    }

    /// Adjusts the decoded value before binding. For a response that leaves out what its request
    /// already said: a fetched member has no `guild_id`, so the member fetch sets it here.
    pub fn fix_wire(mut self, f: impl FnOnce(&mut T::Wire) + Send + 'static) -> Self {
        self.fix = Some(Box::new(f));
        self
    }

    /// Sets the `X-Audit-Log-Reason`.
    pub fn reason(mut self, r: impl Into<String>) -> Self {
        self.request = self.request.map(|q| q.reason(r));
        self
    }
}

impl<T: Bindable> IntoFuture for Call<T> {
    type Output = Result<T, Error>;
    type IntoFuture = BoxFuture<'static, Self::Output>;
    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let mut wire = self.request?.await?;
            if let Some(fix) = self.fix {
                fix(&mut wire);
            }
            Ok(T::bind(&self.client, wire))
        })
    }
}
