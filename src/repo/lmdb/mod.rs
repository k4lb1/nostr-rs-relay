//! LMDB-backed NostrRepo. Zero-copy storage, index_tag (NIP-12), NIP-09 deletion.
//! Pay-to-Relay and NIP-05: unsupported (future phase).

mod error;
mod query;
mod schema;

use crate::config::Settings;
use crate::db::QueryResult;
use crate::error::Result;
use crate::event::{single_char_tagname, Event};
use crate::nip05::VerificationRecord;
use crate::payment::{InvoiceInfo, InvoiceStatus};
use crate::repo::NostrRepo;
use crate::server::NostrMetrics;
use crate::subscription::Subscription;
use async_trait::async_trait;
use error::LmdbError;
use heed::types::{SerdeBincode, Unit};
use heed::{Database, Env, EnvOpenOptions};
use query::run_query;
use schema::{
    author_key, created_key, kind_key, tag_key,
    EVENTS_DB, INDEX_AUTHOR_DB, INDEX_CREATED_DB, INDEX_KIND_DB, INDEX_TAG_DB,
};
use std::path::Path;
use std::sync::Arc;
use tokio::task;
use tracing::{debug, info};

const DEFAULT_MAP_SIZE: usize = 10 * 1024 * 1024 * 1024; // 10GB
const LMDB_UNSUPPORTED: &str = "LMDB does not support Pay-to-Relay / NIP-05. Planned for future release.";

fn map_lmdb_err(e: LmdbError) -> crate::error::Error {
    crate::error::Error::CustomError(e.to_string())
}

#[derive(Clone)]
pub struct LmdbEnv {
    env: Arc<Env>,
    events: Database<SerdeBincode<Vec<u8>>, SerdeBincode<Vec<u8>>>,
    index_author: Database<SerdeBincode<Vec<u8>>, Unit>,
    index_kind: Database<SerdeBincode<Vec<u8>>, Unit>,
    index_tag: Database<SerdeBincode<Vec<u8>>, Unit>,
    index_created: Database<SerdeBincode<Vec<u8>>, Unit>,
}

impl LmdbEnv {
    pub fn open(path: &Path, map_size: Option<usize>) -> Result<Self> {
        std::fs::create_dir_all(path).map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
        let map_size = map_size.unwrap_or(DEFAULT_MAP_SIZE);
        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(map_size)
                .max_dbs(8)
                .open(path)
                .map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?
        };
        let mut wtxn = env.write_txn().map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let events = env
            .create_database(&mut wtxn, Some(EVENTS_DB))
            .map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let index_author = env
            .create_database(&mut wtxn, Some(INDEX_AUTHOR_DB))
            .map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let index_kind = env
            .create_database(&mut wtxn, Some(INDEX_KIND_DB))
            .map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let index_tag = env
            .create_database(&mut wtxn, Some(INDEX_TAG_DB))
            .map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let index_created = env
            .create_database(&mut wtxn, Some(INDEX_CREATED_DB))
            .map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        wtxn.commit().map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        info!("LMDB opened at {:?} (map_size={}GB)", path, map_size / (1024 * 1024 * 1024));
        Ok(Self {
            env: Arc::new(env),
            events,
            index_author,
            index_kind,
            index_tag,
            index_created,
        })
    }

    fn write_event_sync(&self, e: &Event) -> Result<u64> {
        let hash = hex::decode(&e.id).map_err(|_| crate::error::Error::EventInvalidId)?;
        if hash.len() != 32 {
            return Err(crate::error::Error::EventInvalidId);
        }
        let content = serde_json::to_string(e).map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
        let content_bytes = content.into_bytes();

        let mut wtxn = self.env.write_txn().map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;

        if e.kind == 5 {
            let to_delete = e.tag_values_by_name("e");
            let author_bin = hex::decode(&e.pubkey).map_err(|e| map_lmdb_err(LmdbError::Hex(e)))?;
            for id_str in to_delete.iter().filter(|x| hex::decode(x).ok().map(|v| v.len() == 32).unwrap_or(false)) {
                if let Ok(id_hash) = hex::decode(id_str) {
                    self.delete_event_in_txn(&mut wtxn, &id_hash, &author_bin)?;
                }
            }
        }

        let existing = self.events.get(&wtxn, &hash).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        if existing.is_some() {
            wtxn.commit().map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
            return Ok(0);
        }

        self.events.put(&mut wtxn, &hash, &content_bytes).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let auth_key = author_key(
            &hex::decode(&e.pubkey).map_err(|e| map_lmdb_err(LmdbError::Hex(e)))?,
            e.created_at,
            &hash,
        );
        self.index_author.put(&mut wtxn, &auth_key, &()).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let kind_key_bytes = kind_key(e.kind, e.created_at, &hash);
        self.index_kind.put(&mut wtxn, &kind_key_bytes, &()).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let created_key_bytes = created_key(e.created_at, &hash);
        self.index_created.put(&mut wtxn, &created_key_bytes, &()).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;

        for tag in &e.tags {
            if tag.len() >= 2 {
                if let Some(c) = single_char_tagname(&tag[0]) {
                    for v in tag.iter().skip(1) {
                        let tk = tag_key(c as u8, v, e.created_at, &hash);
                        self.index_tag.put(&mut wtxn, &tk, &()).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
                    }
                }
            }
        }

        wtxn.commit().map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        debug!("LMDB: wrote event {}", e.get_event_id_prefix());
        Ok(1)
    }

    fn delete_event_in_txn(
        &self,
        wtxn: &mut heed::RwTxn<'_>,
        event_id: &[u8],
        author: &[u8],
    ) -> Result<()> {
        let key: Vec<u8> = event_id.to_vec();
        let Some(json) = self.events.get(wtxn, &key).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))? else {
            return Ok(());
        };
        let ev: Event = serde_json::from_slice(&json).map_err(|e| map_lmdb_err(LmdbError::Serialization(e.to_string())))?;
        if hex::decode(&ev.pubkey).ok().as_deref() != Some(author) {
            return Ok(());
        }
        let created_at = ev.created_at;
        let kind = ev.kind;

        self.events.delete(wtxn, &key).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let auth_k = author_key(author, created_at, event_id);
        self.index_author.delete(wtxn, &auth_k).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let kind_k = kind_key(kind, created_at, event_id);
        self.index_kind.delete(wtxn, &kind_k).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let created_k = created_key(created_at, event_id);
        self.index_created.delete(wtxn, &created_k).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;

        for tag in &ev.tags {
            if tag.len() >= 2 {
                if let Some(c) = single_char_tagname(&tag[0]) {
                    for v in tag.iter().skip(1) {
                        let tk = tag_key(c as u8, v, created_at, event_id);
                        self.index_tag.delete(wtxn, &tk).map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_id_range(&self, since: u64, until: u64, cap: usize) -> Result<Vec<(u64, [u8; 32])>> {
        let rtxn = self.env.read_txn().map_err(|e| map_lmdb_err(LmdbError::Heed(e)))?;
        let start = created_key(since, &[0u8; 32]);
        let end = created_key(until, &[0xffu8; 32]);
        let range = (std::ops::Bound::Included(start), std::ops::Bound::Included(end));
        let mut out = Vec::with_capacity(cap);
        if let Ok(iter) = self.index_created.range(&rtxn, &range) {
            for item in iter {
                if out.len() >= cap {
                    break;
                }
                let Ok((key, _)) = item else { break };
                if key.len() >= 40 {
                    let mut id = [0u8; 32];
                    id.copy_from_slice(&key[8..40]);
                    let created_at = u64::from_be_bytes(key[0..8].try_into().unwrap_or([0; 8]));
                    out.push((created_at, id));
                }
            }
        }
        Ok(out)
    }
}

fn get_lmdb_map_size(_settings: &Settings) -> usize {
    DEFAULT_MAP_SIZE
}

#[derive(Clone)]
pub struct LmdbRepo {
    env: Arc<LmdbEnv>,
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
        let map_size = get_lmdb_map_size(settings);
        let env = LmdbEnv::open(Path::new(path), Some(map_size)).expect("LMDB open");
        Self {
            env: Arc::new(env),
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
        let env = self.env.clone();
        let e = e.clone();
        task::spawn_blocking(move || env.write_event_sync(&e))
            .await
            .map_err(|_| crate::error::Error::JoinError)?
    }

    async fn query_subscription(
        &self,
        sub: Subscription,
        _client_id: String,
        query_tx: tokio::sync::mpsc::Sender<QueryResult>,
        mut abandon_query_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<()> {
        if abandon_query_rx.try_recv().is_ok() {
            return Ok(());
        }
        let env = self.env.clone();
        let sub_id = sub.id;
        let filters = sub.filters;
        let _ = task::spawn_blocking(move || {
            let rtxn = match env.env.read_txn() {
                Ok(t) => t,
                Err(_e) => {
                    let _ = query_tx.blocking_send(QueryResult {
                        sub_id: sub_id.clone(),
                        event: "EOSE".to_string(),
                    });
                    return;
                }
            };
            let _ = run_query(
                &env.events,
                &env.index_author,
                &env.index_kind,
                &env.index_tag,
                &env.index_created,
                &rtxn,
                &filters,
                &sub_id,
                query_tx,
            );
        })
        .await;
        Ok(())
    }

    async fn optimize_db(&self) -> Result<()> {
        Ok(())
    }

    async fn create_verification_record(&self, _event_id: &str, _name: &str) -> Result<()> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn update_verification_timestamp(&self, _id: u64) -> Result<()> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn fail_verification(&self, _id: u64) -> Result<()> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn delete_verification(&self, _id: u64) -> Result<()> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn get_latest_user_verification(&self, _pub_key: &str) -> Result<VerificationRecord> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn get_oldest_user_verification(&self, _before: u64) -> Result<VerificationRecord> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn create_account(&self, _pubkey: &str) -> Result<bool> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn admit_account(&self, _pubkey: &str, _admission_cost: u64) -> Result<()> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn get_account_balance(&self, _pubkey: &str) -> Result<(bool, u64)> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn update_account_balance(
        &self,
        _pubkey: &str,
        _positive: bool,
        _new_balance: u64,
    ) -> Result<()> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn create_invoice_record(&self, _pubkey: &str, _invoice_info: InvoiceInfo) -> Result<()> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn update_invoice(&self, _payment_hash: &str, _status: InvoiceStatus) -> Result<String> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }

    async fn get_unpaid_invoice(&self, _pubkey: &str) -> Result<Option<InvoiceInfo>> {
        Err(crate::error::Error::CustomError(LMDB_UNSUPPORTED.to_string()))
    }
}
