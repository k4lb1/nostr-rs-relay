//! NIP-42 auth / DM check.

use super::{NipContext, NipHandler, NipHandlerResult};
use crate::error::Result;
use crate::event::Event;
use async_trait::async_trait;

/// NIP-42: Auth and DM restriction. When nip42_dms is enabled, kind 4/44/1059
/// require authenticated sender whose pubkey matches sender or recipient.
pub struct Nip42Handler;

impl Nip42Handler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NipHandler for Nip42Handler {
    fn name(&self) -> &'static str {
        "nip42"
    }

    async fn validate_event(
        &self,
        event: &Event,
        ctx: &NipContext<'_>,
    ) -> Result<NipHandlerResult> {
        if !ctx.settings.authorization.nip42_dms {
            return Ok(NipHandlerResult::Accept);
        }
        if event.kind != 4 && event.kind != 44 && event.kind != 1059 {
            return Ok(NipHandlerResult::Accept);
        }
        let auth_pubkey = match ctx.auth_pubkey {
            Some(pk) => hex::encode(pk),
            None => {
                return Ok(NipHandlerResult::Reject {
                    reason: "authentication required for DM events".to_string(),
                });
            }
        };
        let recipient = event.tag_values_by_name("p").into_iter().next();
        match recipient {
            Some(recv) => {
                if recv == auth_pubkey || event.pubkey == auth_pubkey {
                    Ok(NipHandlerResult::Accept)
                } else {
                    Ok(NipHandlerResult::Reject {
                        reason: "DM events require auth and must be for you or from you".to_string(),
                    })
                }
            }
            None => Ok(NipHandlerResult::Reject {
                reason: "DM events require a recipient (p tag)".to_string(),
            }),
        }
    }
}
