//! Stage instance endpoints.

use crate::{Empty, Method, Request, Rest};
use dr_core::{IntoBody, RequestBody};
use dr_model::{ChannelId, StageInstance, routes};

/// Stage instance endpoints. Get one from [`Rest::stage_instances`].
#[derive(Clone, Copy, Debug)]
pub struct StageInstancesApi<'a>(pub(crate) &'a Rest);

impl StageInstancesApi<'_> {
    /// `POST /stage-instances`. Use `.reason(...)` for the audit log.
    pub fn create(&self, body: impl IntoBody) -> Request<StageInstance> {
        Request::new(self.0.clone(), Method::Post, routes::stage_instances(), body.into_body())
    }

    /// `GET /stage-instances/{channel}`.
    pub fn get(&self, channel: ChannelId) -> Request<StageInstance> {
        Request::new(self.0.clone(), Method::Get, routes::stage_instance(channel), Ok(RequestBody::empty()))
    }

    /// `PATCH /stage-instances/{channel}`.
    pub fn modify(&self, channel: ChannelId, body: impl IntoBody) -> Request<StageInstance> {
        Request::new(self.0.clone(), Method::Patch, routes::stage_instance(channel), body.into_body())
    }

    /// `DELETE /stage-instances/{channel}`.
    pub fn delete(&self, channel: ChannelId) -> Request<Empty> {
        Request::new(self.0.clone(), Method::Delete, routes::stage_instance(channel), Ok(RequestBody::empty()))
    }
}
