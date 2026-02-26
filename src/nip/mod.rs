//! NIP handler trait and pipeline (Khatru-inspired).

use crate::config::Settings;
use crate::error::Result;
use crate::event::Event;
use crate::payment::PaymentMessage;
use crate::repo::NostrRepo;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;

pub mod grpc;
pub mod kind_filter;
pub mod nip01;
pub mod nip05_handler;
pub mod nip09;
pub mod nip26;
pub mod nip42;
pub mod nip50;
pub mod pay_to_relay;
pub mod whitelist;

/// Result of NIP handler processing an event.
#[derive(Debug)]
pub enum NipHandlerResult {
    /// Event passes, continue to next handler.
    Accept,
    /// Pay-to-Relay: event passes, balance to deduct after persist.
    AcceptWithBalance(u64),
    /// Event rejected with reason (send NOTICE to client).
    Reject { reason: String },
    /// Event transformed (e.g. delegation resolution) - use new event.
    Transform(Event),
}

/// Context passed to handlers (auth state, IP, config references).
pub struct NipContext<'a> {
    pub auth_pubkey: Option<&'a [u8]>,
    pub source_ip: &'a str,
    pub settings: &'a Settings,
    pub repo: Option<&'a Arc<dyn NostrRepo>>,
    pub origin: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub payment_tx: Option<&'a broadcast::Sender<PaymentMessage>>,
    pub grpc: Option<&'a Arc<tokio::sync::Mutex<crate::nauthz::EventAuthzService>>>,
}

/// Handler for NIP-specific validation and transformation.
/// Handlers run in sequence: first Reject wins; Transform chains; Accept continues.
#[async_trait]
pub trait NipHandler: Send + Sync {
    /// Unique identifier (e.g. "nip01", "nip05", "nip42").
    fn name(&self) -> &'static str;

    /// Process incoming event before persistence.
    async fn validate_event(
        &self,
        event: &Event,
        ctx: &NipContext<'_>,
    ) -> Result<NipHandlerResult>;
}
