//! NIP-05 verification check.

use super::{NipContext, NipHandler, NipHandlerResult};
use crate::error::{Error, Result};
use crate::event::Event;
use async_trait::async_trait;

/// NIP-05: Verified-users check. When enabled, only verified NIP-05 users may publish.
pub struct Nip05Handler;

impl Nip05Handler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NipHandler for Nip05Handler {
    fn name(&self) -> &'static str {
        "nip05"
    }

    async fn validate_event(
        &self,
        event: &Event,
        ctx: &NipContext<'_>,
    ) -> Result<NipHandlerResult> {
        if !ctx.settings.verified_users.is_enabled() {
            return Ok(NipHandlerResult::Accept);
        }
        let Some(repo) = ctx.repo else {
            return Ok(NipHandlerResult::Accept);
        };
        let validation = repo.get_latest_user_verification(&event.pubkey).await;
        match validation {
            Ok(uv) => {
                if uv.is_valid(&ctx.settings.verified_users) {
                    Ok(NipHandlerResult::Accept)
                } else {
                    Ok(NipHandlerResult::Reject {
                        reason: "NIP-05 verification is no longer valid (expired/wrong domain)"
                            .to_string(),
                    })
                }
            }
            Err(
                Error::SqlError(rusqlite::Error::QueryReturnedNoRows)
                | Error::SqlxError(sqlx::Error::RowNotFound),
            ) => Ok(NipHandlerResult::Reject {
                reason: "NIP-05 verification needed to publish events".to_string(),
            }),
            Err(Error::LmdbUnsupported) => Ok(NipHandlerResult::Reject {
                reason: "NIP-05 verification not supported (LMDB backend)".to_string(),
            }),
            Err(e) => Ok(NipHandlerResult::Reject {
                reason: format!("verification check failed: {e}"),
            }),
        }
    }
}
