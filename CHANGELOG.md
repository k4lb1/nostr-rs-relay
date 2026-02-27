# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

- Config `lmdb_map_size` for LMDB virtual address space
- NIP-26 delegation handler enabled
- NIP-56 (Reporting) and NIP-65 (Relay List Metadata) supported via normal event storage
- NIP-50 full-text search via tantivy (indexes event content)
- NostrRepo stubs: `Error::LmdbUnsupported` for Pay-to-Relay/NIP-05; doc comments on unsupported methods
- Removed dead storage adapters (SqliteStorage, PostgresStorage, QueryEventStream)

## [0.9.0] - LMDB release

### Added

- LMDB storage engine as the default and only backend for the relay
- NIP-12 tag queries via index_tag
- NIP-09 event deletion
- NIP-77 get_id_range support
- Config option `lmdb_map_size` for LMDB virtual address space (default 10GB)
- Config option `lmdb_path` for LMDB data directory (default ./nostr-lmdb)

### Changed

- `build_repo` uses LMDB only; SQLite/Postgres code paths removed from runtime
- Default database engine: sqlite -> lmdb
- NIP-26 delegation handler marked as supported

### Note

- SQLite and PostgreSQL code remain for bulkloader and tests
- Pay-to-Relay and NIP-05 verification unsupported with LMDB
