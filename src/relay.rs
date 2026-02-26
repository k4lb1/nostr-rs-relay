//! Relay event pipeline with NIP handler registry.

use crate::config::Settings;
use crate::event::Event;
use crate::nip::{NipContext, NipHandler, NipHandlerResult};
use crate::repo::NostrRepo;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Relay with handler pipeline. Processes events through registered NIP handlers
/// before persistence.
pub struct Relay {
    handlers: Vec<Arc<dyn NipHandler>>,
    _repo: Arc<dyn NostrRepo>,
    _broadcast: broadcast::Sender<Event>,
    _settings: Settings,
}

impl Relay {
    /// Create a new relay with the given repo, broadcast sender, and settings.
    pub fn new(
        repo: Arc<dyn NostrRepo>,
        settings: Settings,
        _broadcast: broadcast::Sender<Event>,
    ) -> Self {
        Self {
            handlers: Vec::new(),
            _repo: repo,
            _broadcast,
            _settings: settings,
        }
    }

    /// Register a handler. Handler order: NIP-01 → NIP-26 → NIP-05 → NIP-42 → Custom.
    pub fn register_handler(&mut self, handler: Arc<dyn NipHandler>) {
        self.handlers.push(handler);
    }

    /// Run all handlers in sequence. Returns the event to persist (possibly transformed),
    /// optional user balance for Pay-to-Relay, or an error reason for rejection.
    pub async fn process_event<'a>(
        &self,
        mut event: Event,
        ctx: NipContext<'a>,
    ) -> std::result::Result<(Event, Option<u64>), String> {
        let mut user_balance: Option<u64> = None;
        for handler in &self.handlers {
            match handler.validate_event(&event, &ctx).await {
                Ok(NipHandlerResult::Accept) => continue,
                Ok(NipHandlerResult::AcceptWithBalance(balance)) => {
                    user_balance = Some(balance);
                }
                Ok(NipHandlerResult::Reject { reason }) => return Err(reason),
                Ok(NipHandlerResult::Transform(transformed)) => {
                    event = transformed;
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok((event, user_balance))
    }
}
