//! Kind filter: blacklist/allowlist for event kinds.

use super::{NipContext, NipHandler, NipHandlerResult};
use crate::error::Result;
use crate::event::Event;
use async_trait::async_trait;

/// Filters events by kind blacklist/allowlist from settings.
pub struct KindFilterHandler;

impl KindFilterHandler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NipHandler for KindFilterHandler {
    fn name(&self) -> &'static str {
        "kind_filter"
    }

    async fn validate_event(
        &self,
        event: &Event,
        ctx: &NipContext<'_>,
    ) -> Result<NipHandlerResult> {
        if let Some(blacklist) = &ctx.settings.limits.event_kind_blacklist {
            if blacklist.contains(&event.kind) {
                return Ok(NipHandlerResult::Reject {
                    reason: "event kind is blocked by relay".to_string(),
                });
            }
        }
        if let Some(allowlist) = &ctx.settings.limits.event_kind_allowlist {
            if !allowlist.contains(&event.kind) {
                return Ok(NipHandlerResult::Reject {
                    reason: "event kind is blocked by relay".to_string(),
                });
            }
        }
        Ok(NipHandlerResult::Accept)
    }
}
