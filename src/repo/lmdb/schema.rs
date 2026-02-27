//! LMDB schema: database names and key encoding.

pub const EVENTS_DB: &str = "events";
pub const INDEX_AUTHOR_DB: &str = "index_author";
pub const INDEX_KIND_DB: &str = "index_kind";
pub const INDEX_TAG_DB: &str = "index_tag";
pub const INDEX_CREATED_DB: &str = "index_created";

/// Author index key: [pubkey 32][created_at 8 BE][event_id 32]
pub fn author_key(pubkey: &[u8], created_at: u64, event_id: &[u8]) -> Vec<u8> {
    let mut k = Vec::with_capacity(72);
    k.extend_from_slice(pubkey);
    k.extend_from_slice(&created_at.to_be_bytes());
    k.extend_from_slice(event_id);
    k
}

/// Kind index key: [kind 8 BE][created_at 8 BE][event_id 32]
pub fn kind_key(kind: u64, created_at: u64, event_id: &[u8]) -> Vec<u8> {
    let mut k = Vec::with_capacity(48);
    k.extend_from_slice(&kind.to_be_bytes());
    k.extend_from_slice(&created_at.to_be_bytes());
    k.extend_from_slice(event_id);
    k
}

/// Tag index key: [tag_char 1][tag_value padded to 64][created_at 8 BE][event_id 32]
pub fn tag_key(tag_char: u8, tag_value: &str, created_at: u64, event_id: &[u8]) -> Vec<u8> {
    let mut k = Vec::with_capacity(105);
    k.push(tag_char);
    let val_bytes = tag_value.as_bytes();
    let take = val_bytes.len().min(64);
    k.extend_from_slice(&val_bytes[..take]);
    k.resize(1 + 64, 0);
    k.extend_from_slice(&created_at.to_be_bytes());
    k.extend_from_slice(event_id);
    k
}

/// Created index key: [created_at 8 BE][event_id 32]
pub fn created_key(created_at: u64, event_id: &[u8]) -> Vec<u8> {
    let mut k = Vec::with_capacity(40);
    k.extend_from_slice(&created_at.to_be_bytes());
    k.extend_from_slice(event_id);
    k
}
