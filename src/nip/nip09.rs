//! NIP-09 event deletion (soft delete).

use super::{NipContext, NipHandler, NipHandlerResult};
use crate::error::Result;
use crate::event::Event;
use crate::utils::is_hex;
use async_trait::async_trait;

/// NIP-09: Event deletion. Kind 5 must have at least one valid "e" tag (hex, 64 chars).
pub struct Nip09Handler;

fn is_64_hex(s: &str) -> bool {
    is_hex(s) && s.len() == 64
}

impl Nip09Handler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NipHandler for Nip09Handler {
    fn name(&self) -> &'static str {
        "nip09"
    }

    async fn validate_event(
        &self,
        event: &Event,
        _ctx: &NipContext<'_>,
    ) -> Result<NipHandlerResult> {
        if event.kind != 5 {
            return Ok(NipHandlerResult::Accept);
        }
        let e_tags = event.tag_values_by_name("e");
        let has_valid_e = e_tags.iter().any(|x| is_64_hex(x));
        if has_valid_e {
            Ok(NipHandlerResult::Accept)
        } else {
            Ok(NipHandlerResult::Reject {
                reason: "deletion events (kind 5) must reference at least one event with a valid e tag".to_string(),
            })
        }
    }
}
