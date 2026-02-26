//! Admin error reporting via Nostr Kind 1984 (+ optional Kind 4 DM).
//! Broadcasts critical errors to admin pubkey. Rate-limited.
//! Optional: also persist and broadcast encrypted DM (Kind 4) to admin.

use crate::event::Event;
use crate::repo::NostrRepo;
use nostr::event::builder::EventBuilder;
use nostr::event::Kind;
use nostr::key::Keys;
use nostr::nips::nip04;
use nostr::prelude::PublicKey;
use nostr::Tag;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tracing::warn;

/// Context for admin error reporting. Create once at startup.
pub struct AdminErrorContext {
    keys: Keys,
    admin_pubkey_hex: String,
    rate_limit_secs: u64,
    last_sent_at: AtomicU64,
    send_dm: bool,
    dm_repo: Option<Arc<dyn NostrRepo>>,
}

impl AdminErrorContext {
    /// Build from config. Returns None if relay_sk or admin_pubkey is missing/invalid.
    pub fn new(
        relay_sk: &str,
        admin_pubkey_hex: &str,
        rate_limit_secs: u64,
        send_dm: bool,
        dm_repo: Option<Arc<dyn NostrRepo>>,
    ) -> Option<Self> {
        let keys = Keys::parse(relay_sk).ok()?;
        let _ = PublicKey::from_str(admin_pubkey_hex).ok()?;
        if admin_pubkey_hex.len() != 64 || !admin_pubkey_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self {
            keys,
            admin_pubkey_hex: admin_pubkey_hex.to_string(),
            rate_limit_secs,
            last_sent_at: AtomicU64::new(0),
            send_dm,
            dm_repo,
        })
    }
}

/// Send an admin error event (Kind 1984) to the broadcast stream.
/// Tagged with admin pubkey for subscription. Rate-limited.
/// Non-blocking: errors are logged but do not propagate.
/// Uses bcast_tx only - never event_tx (avoids persistence).
pub fn send_admin_error_event(
    error_msg: &str,
    ctx: &AdminErrorContext,
    bcast_tx: &Sender<Event>,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let last = ctx.last_sent_at.load(Ordering::Relaxed);
    if last > 0 && now.saturating_sub(last) < ctx.rate_limit_secs {
        return;
    }
    ctx.last_sent_at.store(now, Ordering::Relaxed);

    let truncated = if error_msg.len() > 500 {
        format!("{}...", &error_msg[..497])
    } else {
        error_msg.to_string()
    };

    // p-tag for admin subscription filter [#p tag]
    let admin_pk = match PublicKey::parse(&ctx.admin_pubkey_hex) {
        Ok(p) => p,
        Err(_) => {
            warn!("admin_error: failed to parse admin pubkey for tag");
            return;
        }
    };
    let tag = Tag::public_key(admin_pk);

    let Ok(nostr_event) =
        EventBuilder::new(Kind::Custom(1984), &truncated)
            .tag(tag)
            .sign_with_keys(&ctx.keys)
    else {
        warn!("admin_error: failed to sign event");
        return;
    };

    let mut relay_event = Event::from(nostr_event);
    relay_event.build_index();

    if bcast_tx.send(relay_event).is_err() {
        warn!("admin_error: broadcast channel closed");
    }

    // Optional: send encrypted DM (Kind 4) to admin – persist + broadcast
    if ctx.send_dm {
        if let Some(ref repo) = ctx.dm_repo {
            let keys = ctx.keys.clone();
            let admin_pubkey_hex = ctx.admin_pubkey_hex.clone();
            let truncated = truncated.clone();
            let repo = Arc::clone(repo);
            let bcast = bcast_tx.clone();

            tokio::spawn(async move {
                let Ok(receiver_pubkey) = PublicKey::parse(&admin_pubkey_hex) else {
                    warn!("admin_error: failed to parse admin pubkey for DM");
                    return;
                };
                let encrypted = match nip04::encrypt(
                    keys.secret_key(),
                    &receiver_pubkey,
                    &truncated,
                ) {
                    Ok(e) => e,
                    Err(_) => {
                        warn!("admin_error: failed to encrypt DM");
                        return;
                    }
                };
                let dm_builder = EventBuilder::new(
                    nostr::event::Kind::EncryptedDirectMessage,
                    encrypted,
                )
                .tag(nostr::Tag::public_key(receiver_pubkey));
                let Ok(dm_event) = dm_builder.sign_with_keys(&keys) else {
                    warn!("admin_error: failed to sign DM");
                    return;
                };

                let mut relay_dm = Event::from(dm_event);
                relay_dm.build_index();

                if repo.write_event(&relay_dm).await.is_err() {
                    warn!("admin_error: failed to persist DM");
                }
                if bcast.send(relay_dm).is_err() {
                    warn!("admin_error: failed to broadcast DM");
                }
            });
        }
    }
}
