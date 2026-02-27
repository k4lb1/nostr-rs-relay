# nostr-rs-relay 2.0 – Project Context

## Current State

- **Phase 0–J:** Handler-Migration abgeschlossen; db_writer bereinigt
- **Teil 3:** NIP-50 FTS (SQLite FTS5) implementiert
- **Teil 2:** Nostr-Crate 0.44 – abgeschlossen
- **Teil 4:** LMDB abgeschlossen: lmdb/ Modul; write_event inkl. index_author/kind/tag, NIP-09; query_subscription ids/authors/kinds/tags; get_id_range NIP-77; build_repo nur noch LMDB (sqlite/postgres-Pfade entfernt); Config default engine=lmdb; PooledConnection→repo::sqlite
- **Build:** cargo build/test OK

## Active Sub-Agent Task

- (keiner)

## Decisions & Rationale

- **KV-Backend:** LMDB (nicht RocksDB) – Nostr-Standard (strfry, rnostr)
- **LMDB reduced mode:** Keine Tags/Search; bei Pay-to-Relay Panic beim Start
- **Audit 2025-02-26:** AUDIT.md erstellt; .gitignore enthält AUDIT.md (intern nur)
- **Handler-Reihenfolge:** Nip01 → KindFilter → NIP-26 → Whitelist → NIP-42 → NIP-05 → PayToRelay → GRPC → NIP-09 → NIP-50
- **NIP-50:** SQLite FTS5, content='event', content_rowid='id'; Suchterm via Parameter

## Handoff / Next Steps

1. CHANGELOG: LMDB-Release erwähnen (sachlich)
2. bulkloader: nutzt weiterhin SQLite (separate binary)
3. Storage (SqliteStorage, PostgresStorage): Dead code, optional entfernen

## Audit-Fixes (2025-02-26)

- Typo "diabaled", println->warn!, auskommentierter From entfernt
- DB_FILE nur in repo/sqlite, async-std entfernt (futures/tokio)
- nip09 nutzt utils::is_hex, QueryEventStream in storage/query_stream extrahiert
- SqlxDatabasePoolError→SqlxError, utils is_nip19/nip19_to_hex #[cfg(test)]
