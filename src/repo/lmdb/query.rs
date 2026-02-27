//! Filter-to-range mapping and query execution.

use crate::db::QueryResult;
use crate::event::Event;
use crate::subscription::ReqFilter;
use heed::types::SerdeBincode;
use heed::{Database, RoTxn};
use std::collections::HashSet;
use std::ops::Bound;
use tokio::sync::mpsc;

use super::error::LmdbError;
use super::schema::{author_key, kind_key, tag_key};

const DEFAULT_LIMIT: u64 = 500;

type EventsDb = Database<SerdeBincode<Vec<u8>>, SerdeBincode<Vec<u8>>>;
type IndexDb = Database<SerdeBincode<Vec<u8>>, heed::types::Unit>;

pub fn run_query(
    events: &EventsDb,
    index_author: &IndexDb,
    index_kind: &IndexDb,
    index_tag: &IndexDb,
    _index_created: &IndexDb,
    txn: &RoTxn<'_>,
    filters: &[ReqFilter],
    sub_id: &str,
    query_tx: mpsc::Sender<QueryResult>,
) -> Result<(), LmdbError> {
    for filter in filters {
        if filter.force_no_match {
            continue;
        }
        if filter.search.is_some() {
            tracing::debug!("LMDB: skipping search filter (NIP-50 not yet implemented)");
            continue;
        }
        let limit = filter.limit.unwrap_or(DEFAULT_LIMIT).min(1000);
        let since = filter.since.unwrap_or(0);
        let until = filter.until.unwrap_or(u64::MAX);

        if let Some(ref ids) = filter.ids {
            for id in ids.iter().take(limit as usize) {
                if let Ok(hash) = hex::decode(id) {
                    if hash.len() == 32 {
                        if let Ok(Some(json)) = events.get(txn, &hash) {
                            let s = String::from_utf8_lossy(&json);
                            if query_tx.blocking_send(QueryResult {
                                sub_id: sub_id.to_string(),
                                event: s.to_string(),
                            }).is_err()
                            {
                                return Ok(());
                            }
                        }
                    }
                }
            }
            continue;
        }

        let mut seen: HashSet<Vec<u8>> = HashSet::new();
        let mut total = 0u64;

        if let Some(ref authors) = filter.authors {
            for author_hex in authors.iter() {
                if total >= limit {
                    break;
                }
                let Ok(author_bin) = hex::decode(author_hex) else { continue };
                if author_bin.len() != 32 {
                    continue;
                }
                let start = author_key(&author_bin, since, &[0u8; 32]);
                let end = author_key(&author_bin, until, &[0xffu8; 32]);
                let range = (Bound::Included(start), Bound::Included(end));
                if let Ok(iter) = index_author.rev_range(txn, &range) {
                    for item in iter {
                        if total >= limit {
                            break;
                        }
                        let Ok((key, _)) = item else { continue };
                        if key.len() >= 72 {
                            let hash = key[40..72].to_vec();
                            if seen.contains(&hash) {
                                continue;
                            }
                            if let Ok(Some(json)) = events.get(txn, &hash) {
                                let s = String::from_utf8_lossy(&json);
                                if let Some(ks) = &filter.kinds {
                                    if let Ok(ev) = serde_json::from_str::<Event>(&s) {
                                        if !ks.contains(&ev.kind) {
                                            continue;
                                        }
                                    }
                                }
                                seen.insert(hash.clone());
                                if query_tx.blocking_send(QueryResult {
                                    sub_id: sub_id.to_string(),
                                    event: s.to_string(),
                                }).is_err()
                                {
                                    return Ok(());
                                }
                                total += 1;
                            }
                        }
                    }
                }
            }
            continue;
        }

        if let Some(ref tags) = filter.tags {
            if !tags.is_empty() {
                for (tag_char, operand) in tags.iter() {
                    if total >= limit {
                        break;
                    }
                    let values: Vec<&String> = operand.iter().collect();
                    if values.is_empty() {
                        continue;
                    }
                    for val in values {
                        let tag_u8 = *tag_char as u8;
                        let start = tag_key(tag_u8, val, since, &[0u8; 32]);
                        let end = tag_key(tag_u8, val, until, &[0xffu8; 32]);
                        let range = (Bound::Included(start), Bound::Included(end));
                        if let Ok(iter) = index_tag.rev_range(txn, &range) {
                            for item in iter {
                                if total >= limit {
                                    break;
                                }
                                let Ok((key, _)) = item else { continue };
                                if key.len() >= 105 {
                                    let hash = key[73..105].to_vec();
                                    if seen.contains(&hash) {
                                        continue;
                                    }
                                    if let Ok(Some(json)) = events.get(txn, &hash) {
                                        let s = String::from_utf8_lossy(&json);
                                        if let Some(ks) = &filter.kinds {
                                            if let Ok(ev) = serde_json::from_str::<Event>(&s) {
                                                if !ks.contains(&ev.kind) {
                                                    continue;
                                                }
                                            }
                                        }
                                        seen.insert(hash.clone());
                                        if query_tx.blocking_send(QueryResult {
                                            sub_id: sub_id.to_string(),
                                            event: s.to_string(),
                                        }).is_err()
                                        {
                                            return Ok(());
                                        }
                                        total += 1;
                                    }
                                }
                            }
                        }
                    }
                }
                continue;
            }
        }

        if let Some(ref kinds_filter) = filter.kinds {
            for &kind in kinds_filter.iter() {
                if total >= limit {
                    break;
                }
                let start = kind_key(kind, since, &[0u8; 32]);
                let end = kind_key(kind, until, &[0xffu8; 32]);
                let range = (Bound::Included(start), Bound::Included(end));
                if let Ok(iter) = index_kind.rev_range(txn, &range) {
                    for item in iter {
                        if total >= limit {
                            break;
                        }
                        let Ok((key, _)) = item else { continue };
                        if key.len() >= 48 {
                            let hash = key[16..48].to_vec();
                            if seen.contains(&hash) {
                                continue;
                            }
                            if let Ok(Some(json)) = events.get(txn, &hash) {
                                let s = String::from_utf8_lossy(&json);
                                if query_tx.blocking_send(QueryResult {
                                    sub_id: sub_id.to_string(),
                                    event: s.to_string(),
                                }).is_err()
                                {
                                    return Ok(());
                                }
                                seen.insert(hash);
                                total += 1;
                            }
                        }
                    }
                }
            }
            continue;
        }
    }

    let _ = query_tx.blocking_send(QueryResult {
        sub_id: sub_id.to_string(),
        event: "EOSE".to_string(),
    });
    Ok(())
}
