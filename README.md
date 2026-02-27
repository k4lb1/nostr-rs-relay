# nostr-rs-relay (LMDB fork)

Fork of [nostr-rs-relay](https://git.sr.ht/~gheartsfield/nostr-rs-relay) by Greg Heartsfield. Replaces SQLite/PostgreSQL with LMDB for event storage.

Upstream: [sourcehut](https://sr.ht/~gheartsfield/nostr-rs-relay/) | [GitHub mirror](https://github.com/scsibug/nostr-rs-relay)

---

## 🔄 What changed vs original

| Original | This fork |
|----------|-----------|
| SQLite (default) or PostgreSQL | LMDB only for relay runtime |
| NIP-50 via SQLite FTS5 | NIP-50 via tantivy (full-text search) |
| Pay-to-Relay and NIP-05 supported | Not supported (returns `Error::LmdbUnsupported`) |
| `build_repo` chooses engine | `build_repo` always uses LMDB |
| `event` + `tag` SQL tables | heed/LMDB: `events` + indices (author, kind, tag, created) |
| Migrations (SQL) | No migrations (LMDB schema is fixed) |
| bulkloader writes to SQLite | bulkloader still writes to SQLite (unchanged) |

New config options: `lmdb_path`, `lmdb_map_size` (default 10GB).

---

## ⚠️ Disclaimer

This fork was developed with AI-assisted tooling. The author's primary background is web development; Rust/relay logic was written with help and may contain bugs. It theoretically works as intended, but there is no warranty. Use at your own risk. Production use should be preceded by thorough testing.

---

## ✨ Features

[NIPs](https://github.com/nostr-protocol/nips) with a relay-specific implementation:

- [x] NIP-01: [Basic protocol flow description](https://github.com/nostr-protocol/nips/blob/master/01.md)
  * Core event model
  * Hide old metadata events
  * Id/Author prefix search
- [x] NIP-02: [Contact List and Petnames](https://github.com/nostr-protocol/nips/blob/master/02.md)
- [ ] NIP-03: [OpenTimestamps Attestations for Events](https://github.com/nostr-protocol/nips/blob/master/03.md)
- [x] NIP-05: [Mapping Nostr keys to DNS-based internet identifiers](https://github.com/nostr-protocol/nips/blob/master/05.md)
- [x] NIP-09: [Event Deletion](https://github.com/nostr-protocol/nips/blob/master/09.md)
- [x] NIP-11: [Relay Information Document](https://github.com/nostr-protocol/nips/blob/master/11.md)
- [x] NIP-12: [Generic Tag Queries](https://github.com/nostr-protocol/nips/blob/master/12.md)
- [x] NIP-15: [End of Stored Events Notice](https://github.com/nostr-protocol/nips/blob/master/15.md)
- [x] NIP-16: [Event Treatment](https://github.com/nostr-protocol/nips/blob/master/16.md)
- [x] NIP-20: [Command Results](https://github.com/nostr-protocol/nips/blob/master/20.md)
- [x] NIP-22: [Event `created_at` limits](https://github.com/nostr-protocol/nips/blob/master/22.md) (_future-dated events only_)
- [x] NIP-26: [Event Delegation](https://github.com/nostr-protocol/nips/blob/master/26.md)
- [x] NIP-28: [Public Chat](https://github.com/nostr-protocol/nips/blob/master/28.md)
- [x] NIP-33: [Parameterized Replaceable Events](https://github.com/nostr-protocol/nips/blob/master/33.md)
- [x] NIP-40: [Expiration Timestamp](https://github.com/nostr-protocol/nips/blob/master/40.md)
- [x] NIP-42: [Authentication of clients to relays](https://github.com/nostr-protocol/nips/blob/master/42.md)
- [x] NIP-50: [Search Capability](https://github.com/nostr-protocol/nips/blob/master/50.md) (tantivy, indexes event content)
- [x] NIP-56: [Reporting](https://github.com/nostr-protocol/nips/blob/master/56.md) (kind 1984)
- [x] NIP-65: [Relay List Metadata](https://github.com/nostr-protocol/nips/blob/master/65.md) (kind 10002)

---

## 🚀 Quick Start

The provided `Dockerfile` will compile and build the server application. Use a bind mount to store the LMDB database outside of the container image.

Add `lmdb_path = "./db/nostr-lmdb"` to `[database]` in your config to persist data.

```console
$ podman build --pull -t nostr-rs-relay .

$ mkdir data

$ podman unshare chown 100:100 data

$ podman run -it --rm -p 7000:8080 \
  --user=100:100 \
  -v $(pwd)/data:/usr/src/app/db:Z \
  -v $(pwd)/config.toml:/usr/src/app/config.toml:ro,Z \
  --name nostr-relay nostr-rs-relay:latest
```

Use a nostr client such as [`noscl`](https://github.com/fiatjaf/noscl) to publish and query events.

A pre-built upstream container is available on DockerHub: https://hub.docker.com/r/scsibug/nostr-rs-relay

---

## 🔧 Build and Run (without Docker)

Requires Cargo & Rust: https://www.rust-lang.org/tools/install

Debian/Ubuntu:
```console
$ sudo apt-get install build-essential cmake protobuf-compiler pkg-config libssl-dev
```

OpenBSD:
```console
$ doas pkg_add rust protobuf
```

```console
$ git clone -q https://github.com/k4lb1/nostr-rs-relay
$ cd nostr-rs-relay
$ cargo build -q -r
```

```console
$ RUST_LOG=warn,nostr_rs_relay=info ./target/release/nostr-rs-relay
```

The relay listens on port `8080` by default.

---

## 💾 LMDB Storage

Event storage uses LMDB only. Default path: `./nostr-lmdb` (config: `lmdb_path`). Optional: `lmdb_map_size` (bytes, default 10GB).

Supported: NIP-09 (deletion), NIP-12 (tag queries), NIP-50 (full-text search via tantivy).

Not supported: NIP-05 verification, Pay-to-Relay. Requires `pay_to_relay.enabled = false` (otherwise the relay panics at startup).

---

## 📌 Still open / TODO

- Pay-to-Relay and NIP-05: not implemented for LMDB (would need account/invoice storage)
- bulkloader: still writes to SQLite; no LMDB bulk import path yet
- NIP-50 search index: created on first run; existing events are not backfilled
- Dead code: `SqliteStorage`, `PostgresStorage` removed; `repo::sqlite` and `repo::postgres` kept for bulkloader and tests

---

## 🧪 Needs testing

- [ ] Integration tests with LMDB (e.g. `start_and_stop`)
- [ ] NIP-50 search under load (tantivy commit frequency, performance)
- [ ] NIP-09 deletion with many events
- [ ] Large datasets (map_size behaviour, disk growth)
- [ ] Concurrent subscriptions and write load
- [ ] Migration path from SQLite (if anyone needs it)

---

## ⚙️ Configuration

Sample [`config.toml`](config.toml) shows available options. You can mount it into a container:

```console
$ docker run -it -p 7000:8080 \
  --mount src=$(pwd)/config.toml,target=/usr/src/app/config.toml,type=bind \
  --mount src=$(pwd)/data,target=/usr/src/app/db,type=bind \
  nostr-rs-relay
```

---

## 🌐 Reverse Proxy

For TLS, load balancing, etc., see [Reverse Proxy](docs/reverse-proxy.md).

---

## 📄 License

MIT License. See [LICENSE](LICENSE).

- Original nostr-rs-relay: Copyright (c) 2021 Greg Heartsfield
- LMDB fork changes: Copyright (c) 2025 k4lb1

---

## 🔗 Links

- [BlockChainCaffe's Nostr Relay Setup Guide](https://github.com/BlockChainCaffe/Nostr-Relay-Setup-Guide)
- [sourcehut mailing list](https://lists.sr.ht/~gheartsfield/nostr-rs-relay-devel) (upstream dev)
