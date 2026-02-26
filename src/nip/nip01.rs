//! NIP-01 basic event validation (canonical form, id, signature).

use super::{NipContext, NipHandler, NipHandlerResult};
use crate::error::Result;
use crate::event::Event;
use async_trait::async_trait;

/// Basic NIP-01 validation: canonical form, event id, schnorr signature.
pub struct Nip01Handler;

impl Nip01Handler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NipHandler for Nip01Handler {
    fn name(&self) -> &'static str {
        "nip01"
    }

    async fn validate_event(
        &self,
        event: &Event,
        _ctx: &NipContext<'_>,
    ) -> Result<NipHandlerResult> {
        match event.validate() {
            Ok(()) => Ok(NipHandlerResult::Accept),
            Err(e) => Ok(NipHandlerResult::Reject {
                reason: e.to_string(),
            }),
        }
    }
}
