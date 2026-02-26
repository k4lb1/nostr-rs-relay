//! NIP-26 delegation. Scaffold.

use super::{NipContext, NipHandler, NipHandlerResult};
use crate::error::Result;
use crate::event::Event;
use async_trait::async_trait;

/// NIP-26: Event delegation. Validates delegation tags and can transform
/// event with resolved delegatee. Scaffold: accepts all; delegation
/// resolution currently in event.update_delegation() (called in From).
pub struct Nip26Handler;

impl Nip26Handler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NipHandler for Nip26Handler {
    fn name(&self) -> &'static str {
        "nip26"
    }

    async fn validate_event(
        &self,
        event: &Event,
        _ctx: &NipContext<'_>,
    ) -> Result<NipHandlerResult> {
        let mut modified = event.clone();
        modified.update_delegation();
        Ok(NipHandlerResult::Transform(modified))
    }
}
