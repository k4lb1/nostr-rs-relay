//! NIP-50 full-text search via tantivy. Indexes event content field.

use crate::error::Result;
use crate::event::Event;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{OwnedValue, *};
use tantivy::{Index, IndexWriter, TantivyDocument, Term};

const EVENT_ID_FIELD: &str = "event_id";
const CONTENT_FIELD: &str = "content";

fn schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_bytes_field(EVENT_ID_FIELD, STORED | INDEXED);
    builder.add_text_field(CONTENT_FIELD, TEXT | STORED);
    builder.build()
}

/// Tantivy-backed NIP-50 search index. Indexes event.content for full-text search.
pub struct SearchIndex {
    index: Index,
    event_id_field: Field,
    content_field: Field,
}

impl SearchIndex {
    /// Open or create search index at path.
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path).map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
        let schema = schema();
        let index = if path.join("meta.json").exists() {
            Index::open_in_dir(path).map_err(|e| crate::error::Error::CustomError(e.to_string()))?
        } else {
            Index::create_in_dir(path, schema.clone())
                .map_err(|e| crate::error::Error::CustomError(e.to_string()))?
        };
        let event_id_field = index.schema().get_field(EVENT_ID_FIELD).expect("event_id field");
        let content_field = index.schema().get_field(CONTENT_FIELD).expect("content field");
        Ok(Self {
            index,
            event_id_field,
            content_field,
        })
    }

    /// Add event to search index. Call after LMDB write. Skips if content empty.
    pub fn add_document(&self, event_id: &[u8], event: &Event) -> Result<()> {
        if event.content.is_empty() {
            return Ok(());
        }
        let mut writer: IndexWriter = self
            .index
            .writer(50_000_000)
            .map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
        writer
            .add_document(tantivy::doc!(
                self.event_id_field => event_id.to_vec(),
                self.content_field => event.content.as_str()
            ))
            .map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
        writer
            .commit()
            .map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
        Ok(())
    }

    /// Remove event from search index. Call when event is deleted (e.g. NIP-09).
    pub fn delete_document(&self, event_id: &[u8]) -> Result<()> {
        let mut writer: IndexWriter = self
            .index
            .writer(50_000_000)
            .map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
        let term = Term::from_field_bytes(self.event_id_field, event_id);
        writer.delete_term(term);
        writer
            .commit()
            .map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
        Ok(())
    }

    /// Search for events matching query. Returns event_ids (32-byte hashes) in relevance order.
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<Vec<u8>>> {
        let reader = self
            .index
            .reader()
            .map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
        let searcher = reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.content_field]);
        let query = query_parser
            .parse_query(query_str.trim())
            .map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
        let mut out = Vec::with_capacity(top_docs.len());
        for (_score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| crate::error::Error::CustomError(e.to_string()))?;
            for val in doc.get_all(self.event_id_field) {
                if let OwnedValue::Bytes(b) = val {
                    if b.len() == 32 {
                        out.push(b.clone());
                    }
                }
            }
        }
        Ok(out)
    }
}
