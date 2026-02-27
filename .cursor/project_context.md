# nostr-rs-relay 2.0 – Project Context

## Current State

- **Phase 0–J:** Handler-Migration abgeschlossen; db_writer bereinigt
- **Teil 3:** NIP-50 FTS (SQLite FTS5) implementiert
- **Teil 2:** Nostr-Crate 0.44 – abgeschlossen
- **Teil 4:** LMDB abgeschlossen: lmdb/ Modul; build_repo nur LMDB; NIP-50 tantivy; Config lmdb_map_size; NostrRepo stubs (LmdbUnsupported Error, Doc comments)
- **Build:** cargo build/test OK

## Active Sub-Agent Task

- (keiner)

## Decisions & Rationale

- **KV-Backend:** LMDB (nicht RocksDB) – Nostr-Standard (strfry, rnostr)
- **LMDB:** Tags, Search (tantivy) supported; Pay-to-Relay/NIP-05 unsupported (Error::LmdbUnsupported); pay_to_relay.enabled Panic beim Start
- **Audit 2025-02-26:** AUDIT.md erstellt; .gitignore enthält AUDIT.md (intern nur)
- **Handler-Reihenfolge:** Nip01 → KindFilter → NIP-26 → Whitelist → NIP-42 → NIP-05 → PayToRelay → GRPC → NIP-09 → NIP-50
- **NIP-50:** SQLite FTS5, content='event', content_rowid='id'; Suchterm via Parameter

## Handoff / Next Steps

1. bulkloader: nutzt weiterhin SQLite (separate binary)
2. Auditor-TODO (2025-02-27): siehe Auditor Findings; Priorität: 1=dead storage entfernen, 2=Kommentare, 3=is_64_hex, 4–8 optional

## Auditor Findings (2025-02-27)

**Erledigt:**
- storage/lmdb + Storage trait entfernt (toter Code)
- utils::is_64_hex extrahiert, sqlite/postgres/nip09 nutzen es
- db.rs, server.rs: veraltete SQLite-Kommentare angepasst
- nip05_handler, pay_to_relay: LmdbUnsupported explizit behandelt
- config.toml: DB-Kommentare für LMDB-Fork aktualisiert

**Offen (optional):**
- postgres als optionales Feature auslagern
- Integration-Test: in_memory/min_conn/max_conn bei LMDB kommentieren

**Architektur:**
- Zwei LMDB-Implementierungen: repo/lmdb (vollständig, in Nutzung) vs storage/lmdb (simplifiziert, tot). Empfehlung: storage-Modul entfernen oder als optional feature.
- postgres/sqlite: Nur bulkloader nutzt sqlite; postgres wird von main binary nicht verwendet. Optional: postgres als feature-flag auslagern.

**Struktur:**
- config.toml: database-Kommentare verweisen auf sqlite/postgres engine; Default ist lmdb.
- Integration-Test: in_memory, min_conn, max_conn haben mit LMDB keine Wirkung (build_repo ignoriert sie).

**Optional/Klarheit:**
- nip05_handler, pay_to_relay: Err-Zweige behandeln SqlError/SqlxError; LmdbUnsupported explizit erwähnen für Lesbarkeit.

## Audit-Fixes (2025-02-26)

- Typo "diabaled", println->warn!, auskommentierter From entfernt
- DB_FILE nur in repo/sqlite, async-std entfernt (futures/tokio)
- nip09 nutzt utils::is_hex, QueryEventStream in storage/query_stream extrahiert
- SqlxDatabasePoolError→SqlxError, utils is_nip19/nip19_to_hex #[cfg(test)]
