//! Storage abstraction for event persistence and querying.
//! Slimmer than NostrRepo; enables future KV backends (e.g. LMDB).

pub mod lmdb;
mod postgres;
mod query_stream;
mod sqlite;

pub(super) use query_stream::QueryEventStream;

use crate::error::Result;
use crate::event::Event;
use crate::subscription::ReqFilter;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

/// Slim storage trait for event write/query. Separates core event operations
/// from NostrRepo's payment, NIP-05, verification concerns.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Persist event. Returns row id or 0 if duplicate/ignored.
    async fn write_event(&self, e: &Event) -> Result<u64>;

    /// Query events by filters. Returns stream of events.
    /// `limit` applies per filter; `abandon` cancels the query when signaled.
    async fn query_events(
        &self,
        filters: &[ReqFilter],
        limit: u64,
        abandon: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<Pin<Box<dyn Stream<Item = Event> + Send>>>;
}
