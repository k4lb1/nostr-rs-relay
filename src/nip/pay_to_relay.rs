//! Pay-to-relay: verify user balance before accepting event.

use super::{NipContext, NipHandler, NipHandlerResult};
use crate::error::{Error, Result};
use crate::event::Event;
use crate::payment::PaymentMessage;
use async_trait::async_trait;
use nostr::key::PublicKey;

/// Pay-to-relay: when enabled, user must be admitted and have sufficient balance.
pub struct PayToRelayHandler;

impl PayToRelayHandler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NipHandler for PayToRelayHandler {
    fn name(&self) -> &'static str {
        "pay_to_relay"
    }

    async fn validate_event(
        &self,
        event: &Event,
        ctx: &NipContext<'_>,
    ) -> Result<NipHandlerResult> {
        if !ctx.settings.pay_to_relay.enabled {
            return Ok(NipHandlerResult::Accept);
        }
        let whitelist = &ctx.settings.authorization.pubkey_whitelist;
        if whitelist
            .as_ref()
            .map_or(false, |wl| wl.contains(&event.pubkey))
        {
            return Ok(NipHandlerResult::Accept);
        }
        let Some(repo) = ctx.repo else {
            return Ok(NipHandlerResult::Accept);
        };
        PublicKey::parse(&event.pubkey).map_err(|_| Error::EventMalformedPubkey)?;
        match repo.get_account_balance(&event.pubkey).await {
            Ok((admitted, balance)) => {
                if !admitted {
                    if let Some(tx) = ctx.payment_tx {
                        let _ = tx.send(PaymentMessage::CheckAccount(event.pubkey.clone()));
                    }
                    return Ok(NipHandlerResult::Reject {
                        reason: "User is not admitted".to_string(),
                    });
                }
                if balance < ctx.settings.pay_to_relay.cost_per_event {
                    return Ok(NipHandlerResult::Reject {
                        reason: "Insufficient balance".to_string(),
                    });
                }
                Ok(NipHandlerResult::AcceptWithBalance(balance))
            }
            Err(Error::LmdbUnsupported) => Ok(NipHandlerResult::Reject {
                reason: "Pay-to-Relay not supported (LMDB backend)".to_string(),
            }),
            Err(
                Error::SqlError(rusqlite::Error::QueryReturnedNoRows)
                | Error::SqlxError(sqlx::Error::RowNotFound),
            ) => {
                if ctx.settings.pay_to_relay.sign_ups
                    && ctx.settings.pay_to_relay.direct_message
                {
                    if let Some(tx) = ctx.payment_tx {
                        let _ = tx.send(PaymentMessage::NewAccount(event.pubkey.clone()));
                    }
                }
                Ok(NipHandlerResult::Reject {
                    reason: "Pubkey not registered".to_string(),
                })
            }
            Err(e) => Ok(NipHandlerResult::Reject {
                reason: format!(
                    "relay experienced an error checking your admission status: {e}"
                ),
            }),
        }
    }
}
