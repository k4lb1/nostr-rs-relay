//! LMDB-backed NostrRepo (reduced mode).
//! No Pay-to-Relay, NIP-05, or tag/search support.

use crate::config::Settings;
use crate::db::QueryResult;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::nip05::VerificationRecord;
use crate::payment::{InvoiceInfo, InvoiceStatus};
use crate::repo::NostrRepo;
use crate::server::NostrMetrics;
use crate::storage::lmdb::LmdbStorage;
use crate::storage::Storage;
use crate::subscription::Subscription;
use async_trait::async_trait;
use std::path::Path;
use tokio::task;

const LMDB_UNSUPPORTED: &str = "LMDB engine does not support this feature";

#[derive(Clone)]
pub struct LmdbRepo {
    storage: LmdbStorage,
    _metrics: NostrMetrics,
}

impl LmdbRepo {
    #[must_use]
    pub fn new(settings: &Settings, metrics: NostrMetrics) -> Self {
        let path = settings
            .database
            .lmdb_path
            .as_deref()
            .unwrap_or("./nostr-lmdb");
        let storage = LmdbStorage::open(Path::new(path)).expect("LMDB open");
        Self {
            storage,
            _metrics: metrics,
        }
    }
}

#[async_trait]
impl NostrRepo for LmdbRepo {
    async fn start(&self) -> Result<()> {
        Ok(())
    }

    async fn migrate_up(&self) -> Result<usize> {
        Ok(0)
    }

    async fn write_event(&self, e: &Event) -> Result<u64> {
        self.storage.write_event(e).await
    }

    async fn query_subscription(
        &self,
        sub: Subscription,
        _client_id: String,
        query_tx: tokio::sync::mpsc::Sender<QueryResult>,
        mut abandon_query_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<()> {
        let storage = self.storage.clone();
        let sub_id = sub.id.clone();
        let filters = sub.filters;
        let default_limit = 500;
        if abandon_query_rx.try_recv().is_ok() {
            return Ok(());
        }
        let _ = task::spawn_blocking(move || {
            storage.run_query(&filters, &sub_id, query_tx, default_limit)
        })
        .await;
        Ok(())
    }

    async fn optimize_db(&self) -> Result<()> {
        Ok(())
    }

    async fn create_verification_record(&self, _event_id: &str, _name: &str) -> Result<()> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn update_verification_timestamp(&self, _id: u64) -> Result<()> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn fail_verification(&self, _id: u64) -> Result<()> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn delete_verification(&self, _id: u64) -> Result<()> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn get_latest_user_verification(&self, _pub_key: &str) -> Result<VerificationRecord> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn get_oldest_user_verification(&self, _before: u64) -> Result<VerificationRecord> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn create_account(&self, _pubkey: &str) -> Result<bool> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn admit_account(&self, _pubkey: &str, _admission_cost: u64) -> Result<()> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn get_account_balance(&self, _pubkey: &str) -> Result<(bool, u64)> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn update_account_balance(
        &self,
        _pubkey: &str,
        _positive: bool,
        _new_balance: u64,
    ) -> Result<()> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn create_invoice_record(&self, _pubkey: &str, _invoice_info: InvoiceInfo) -> Result<()> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn update_invoice(&self, _payment_hash: &str, _status: InvoiceStatus) -> Result<String> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn get_unpaid_invoice(&self, _pubkey: &str) -> Result<Option<InvoiceInfo>> {
        Err(Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }
}
