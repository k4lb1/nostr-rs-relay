//! SQLite storage adapter.

use crate::db::QueryResult;
use crate::error::Result;
use crate::event::Event;
use crate::repo::NostrRepo;
use crate::storage::{QueryEventStream, Storage};
use crate::subscription::{ReqFilter, Subscription};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use tokio::sync::mpsc;

/// SQLite-backed storage. Wraps SqliteRepo and implements the slim Storage trait.
pub struct SqliteStorage {
    repo: std::sync::Arc<crate::repo::sqlite::SqliteRepo>,
}

impl SqliteStorage {
    /// Create from existing SqliteRepo.
    #[must_use]
    pub fn new(repo: std::sync::Arc<crate::repo::sqlite::SqliteRepo>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    async fn write_event(&self, e: &Event) -> Result<u64> {
        self.repo.write_event(e).await
    }

    async fn query_events(
        &self,
        filters: &[ReqFilter],
        limit: u64,
        abandon: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<Pin<Box<dyn Stream<Item = Event> + Send>>> {
        let mut filters_with_limit: Vec<ReqFilter> = filters.to_vec();
        for f in &mut filters_with_limit {
            if f.limit.is_none() {
                f.limit = Some(limit);
            }
        }
        let sub = Subscription {
            id: "storage_query".to_string(),
            filters: filters_with_limit,
        };
        let (query_tx, query_rx) = mpsc::channel::<QueryResult>(256);
        let repo = self.repo.clone();
        tokio::spawn(async move {
            let _ = repo
                .query_subscription(sub, "storage".to_string(), query_tx, abandon)
                .await;
        });
        Ok(Box::pin(QueryEventStream { rx: query_rx }))
    }
}
