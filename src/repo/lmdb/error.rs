//! LMDB-specific errors.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum LmdbError {
    #[error("heed error: {0}")]
    Heed(#[from] heed::Error),

    #[error("hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("event not found")]
    EventNotFound,

    #[error("index corrupted or invalid key")]
    IndexCorrupt,

    #[error("{0}")]
    Unsupported(String),
}
