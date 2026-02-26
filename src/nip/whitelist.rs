//! Pubkey whitelist: when pay-to-relay is disabled, only whitelisted pubkeys may publish.

use super::{NipContext, NipHandler, NipHandlerResult};
use crate::error::Result;
use crate::event::Event;
use async_trait::async_trait;

/// When pay-to-relay is disabled and a whitelist exists, only whitelisted pubkeys may post.
/// When pay-to-relay is enabled, whitelist only affects "post for free" (handled by PayToRelay).
pub struct WhitelistHandler;

impl WhitelistHandler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NipHandler for WhitelistHandler {
    fn name(&self) -> &'static str {
        "whitelist"
    }

    async fn validate_event(
        &self,
        event: &Event,
        ctx: &NipContext<'_>,
    ) -> Result<NipHandlerResult> {
        if ctx.settings.pay_to_relay.enabled {
            return Ok(NipHandlerResult::Accept);
        }
        if let Some(whitelist) = &ctx.settings.authorization.pubkey_whitelist {
            let author = event.delegated_by.as_ref().unwrap_or(&event.pubkey);
            if !whitelist.contains(author) {
                return Ok(NipHandlerResult::Reject {
                    reason: "pubkey is not allowed to publish to this relay".to_string(),
                });
            }
        }
        Ok(NipHandlerResult::Accept)
    }
}
