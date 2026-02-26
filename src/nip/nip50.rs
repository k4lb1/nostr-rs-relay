//! NIP-50 search. Scaffold.

use super::{NipContext, NipHandler, NipHandlerResult};
use crate::error::Result;
use crate::event::Event;
use async_trait::async_trait;

/// NIP-50: Full-text search. Applies to REQ/filters, not EVENT.
/// validate_event is No-op; search handled in query path.
pub struct Nip50Handler;

impl Nip50Handler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NipHandler for Nip50Handler {
    fn name(&self) -> &'static str {
        "nip50"
    }

    async fn validate_event(
        &self,
        _event: &Event,
        _ctx: &NipContext<'_>,
    ) -> Result<NipHandlerResult> {
        // NIP-50 affects REQ (search filter), not EVENT
        Ok(NipHandlerResult::Accept)
    }
}
