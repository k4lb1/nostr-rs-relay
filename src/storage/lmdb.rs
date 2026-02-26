//! LMDB-backed storage for events (NIP-50, high read performance).
//! Schema: events by hash; indexes: author+created_at, kind+created_at.

use crate::db::QueryResult;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::storage::Storage;
use crate::subscription::ReqFilter;
use async_trait::async_trait;
use futures::stream::empty;
use futures::Stream;
use heed::types::SerdeBincode;
use heed::{Database, Env, EnvOpenOptions};
use std::ops::Bound;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info};

const EVENTS_DB: &str = "events";
const AUTHOR_IDX_DB: &str = "author_idx";
const KIND_IDX_DB: &str = "kind_idx";

fn db_err(e: heed::Error) -> Error {
    Error::CustomError(e.to_string())
}

/// LMDB-backed event storage. Events keyed by hash; indexes for author+created_at, kind+created_at.
#[derive(Clone)]
pub struct LmdbStorage {
    env: Arc<Env>,
    events: Database<SerdeBincode<Vec<u8>>, SerdeBincode<String>>,
    author_idx: Database<SerdeBincode<Vec<u8>>, SerdeBincode<()>>,
    kind_idx: Database<SerdeBincode<Vec<u8>>, SerdeBincode<()>>,
}

impl LmdbStorage {
    /// Open or create LMDB environment at path.
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path).map_err(|e| Error::CustomError(e.to_string()))?;
        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(100 * 1024 * 1024) // 100MB
                .max_dbs(8)
                .open(path)
                .map_err(db_err)?
        };
        let mut wtxn = env.write_txn().map_err(db_err)?;
        let events = env.create_database(&mut wtxn, Some(EVENTS_DB)).map_err(db_err)?;
        let author_idx = env.create_database(&mut wtxn, Some(AUTHOR_IDX_DB)).map_err(db_err)?;
        let kind_idx = env.create_database(&mut wtxn, Some(KIND_IDX_DB)).map_err(db_err)?;
        wtxn.commit().map_err(db_err)?;
        info!("LMDB storage opened at {:?}", path);
        Ok(Self {
            env: Arc::new(env),
            events,
            author_idx,
            kind_idx,
        })
    }

    fn build_index_keys(e: &Event, hash: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let author_bin = hex::decode(&e.pubkey).unwrap_or_default();
        let author_key: Vec<u8> = author_bin
            .iter()
            .chain(e.created_at.to_be_bytes().iter())
            .chain(hash)
            .cloned()
            .collect();
        let kind_key: Vec<u8> = e
            .kind
            .to_be_bytes()
            .iter()
            .chain(e.created_at.to_be_bytes().iter())
            .chain(hash)
            .cloned()
            .collect();
        (author_key, kind_key)
    }

    /// Run query for filters; blocking. Sends QueryResult to query_tx, then EOSE.
    pub fn run_query(
        &self,
        filters: &[ReqFilter],
        sub_id: &str,
        query_tx: mpsc::Sender<QueryResult>,
        default_limit: u64,
    ) -> Result<()> {
        let rtxn = self.env.read_txn().map_err(db_err)?;
        for filter in filters {
            if filter.force_no_match {
                continue;
            }
            if filter.search.is_some() || (filter.tags.is_some() && !filter.tags.as_ref().unwrap().is_empty()) {
                debug!("LMDB: skipping filter with search/tags (not supported)");
                continue;
            }
            let limit = filter.limit.unwrap_or(default_limit).min(1000);
            let since = filter.since.unwrap_or(0);
            let until = filter.until.unwrap_or(u64::MAX);

            if let Some(ref ids) = filter.ids {
                for id in ids.iter().take(limit as usize) {
                    if let Ok(hash) = hex::decode(id) {
                        if hash.len() == 32 {
                            if let Ok(Some(json)) = self.events.get(&rtxn, &hash) {
                                if query_tx.blocking_send(QueryResult { sub_id: sub_id.to_string(), event: json }).is_err() {
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
                continue;
            }

            if let Some(ref authors) = filter.authors {
                let kinds = filter.kinds.as_ref();
                let mut total_count = 0u64;
                for author_hex in authors.iter() {
                    if total_count >= limit {
                        break;
                    }
                    if let Ok(author_bin) = hex::decode(author_hex) {
                        if author_bin.len() != 32 {
                            continue;
                        }
                        let mut start_key = author_bin.clone();
                        start_key.extend_from_slice(&since.to_be_bytes());
                        start_key.extend_from_slice(&[0u8; 32]);
                        let mut end_key = author_bin.clone();
                        end_key.extend_from_slice(&until.to_be_bytes());
                        end_key.extend_from_slice(&[0xff; 32]);
                        let range = (Bound::Included(start_key), Bound::Included(end_key));
                        if let Ok(iter) = self.author_idx.rev_range(&rtxn, &range) {
                            for item in iter {
                                if total_count >= limit {
                                    break;
                                }
                                if let Ok((key, _)) = item {
                                    if key.len() >= 72 {
                                        let hash: Vec<u8> = key[40..72].to_vec();
                                        if let Ok(Some(json)) = self.events.get(&rtxn, &hash) {
                                            if let Some(ks) = kinds {
                                                if let Ok(ev) = serde_json::from_str::<Event>(&json) {
                                                    if !ks.contains(&ev.kind) {
                                                        continue;
                                                    }
                                                }
                                            }
                                            if query_tx.blocking_send(QueryResult { sub_id: sub_id.to_string(), event: json }).is_err() {
                                                return Ok(());
                                            }
                                            total_count += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                continue;
            }

            if let Some(ref kinds_filter) = filter.kinds {
                let mut total_count = 0u64;
                for &kind in kinds_filter.iter() {
                    if total_count >= limit {
                        break;
                    }
                    let mut start_key = kind.to_be_bytes().to_vec();
                    start_key.extend_from_slice(&since.to_be_bytes());
                    start_key.extend_from_slice(&[0u8; 32]);
                    let mut end_key = kind.to_be_bytes().to_vec();
                    end_key.extend_from_slice(&until.to_be_bytes());
                    end_key.extend_from_slice(&[0xff; 32]);
                    let range = (Bound::Included(start_key), Bound::Included(end_key));
                    if let Ok(iter) = self.kind_idx.rev_range(&rtxn, &range) {
                        for item in iter {
                            if total_count >= limit {
                                break;
                            }
                            if let Ok((key, _)) = item {
                                if key.len() >= 48 {
                                    let hash: Vec<u8> = key[16..48].to_vec();
                                    if let Ok(Some(json)) = self.events.get(&rtxn, &hash) {
                                        if query_tx.blocking_send(QueryResult { sub_id: sub_id.to_string(), event: json }).is_err() {
                                            return Ok(());
                                        }
                                        total_count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
                continue;
            }
        }
        let _ = query_tx.blocking_send(QueryResult { sub_id: sub_id.to_string(), event: "EOSE".to_string() });
        Ok(())
    }
}

#[async_trait]
impl Storage for LmdbStorage {
    async fn write_event(&self, e: &Event) -> Result<u64> {
        let hash = hex::decode(&e.id).map_err(|_| Error::EventInvalidId)?;
        if hash.len() != 32 {
            return Err(Error::EventInvalidId);
        }
        let content =
            serde_json::to_string(e).map_err(|e| Error::CustomError(e.to_string()))?;
        let mut wtxn = self.env.write_txn().map_err(db_err)?;
        let existing = self.events.get(&wtxn, &hash).map_err(db_err)?;
        if existing.is_some() {
            wtxn.commit().map_err(db_err)?;
            return Ok(0); // duplicate
        }
        self.events
            .put(&mut wtxn, &hash, &content)
            .map_err(db_err)?;
        let (author_key, kind_key) = Self::build_index_keys(e, &hash);
        self.author_idx
            .put(&mut wtxn, &author_key, &())
            .map_err(db_err)?;
        self.kind_idx
            .put(&mut wtxn, &kind_key, &())
            .map_err(db_err)?;
        wtxn.commit().map_err(db_err)?;
        debug!("LMDB: wrote event {}", e.get_event_id_prefix());
        Ok(1)
    }

    async fn query_events(
        &self,
        _filters: &[ReqFilter],
        _limit: u64,
        _abandon: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<Pin<Box<dyn Stream<Item = Event> + Send>>> {
        Ok(Box::pin(empty()))
    }
}
