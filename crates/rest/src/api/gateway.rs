//! Gateway information endpoints.

use crate::{Method, Request, Rest};
use dr_core::RequestBody;
use dr_model::rest::GetGatewayBotResult;
use dr_model::routes;

/// Gateway information endpoints. Get one from [`Rest::gateway`].
#[derive(Clone, Copy, Debug)]
pub struct GatewayApi<'a>(pub(crate) &'a Rest);

impl GatewayApi<'_> {
    /// `GET /gateway/bot`.
    pub fn get_bot(&self) -> Request<GetGatewayBotResult> {
        Request::new(self.0.clone(), Method::Get, routes::gateway_bot(), Ok(RequestBody::empty()))
    }
}
