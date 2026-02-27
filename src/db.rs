//! Event persistence and querying
use crate::admin_error;
use crate::config::Settings;
use crate::error::Result;
use crate::event::Event;
use crate::notice::Notice;
use crate::payment::PaymentMessage;
use crate::repo::lmdb::LmdbRepo;
use crate::repo::NostrRepo;
use crate::server::NostrMetrics;
use governor::clock::Clock;
use governor::{Quota, RateLimiter};
use nostr::key::PublicKey;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use tracing::{debug, info, trace, warn};

/// Events submitted from a client, with a return channel for notices
pub struct SubmittedEvent {
    pub event: Event,
    pub notice_tx: tokio::sync::mpsc::Sender<Notice>,
    pub source_ip: String,
    pub origin: Option<String>,
    pub user_agent: Option<String>,
    pub auth_pubkey: Option<Vec<u8>>,
    pub user_balance: Option<u64>,
}

/// Build repo (LMDB only).
/// # Panics
///
/// Panics if pay_to_relay is enabled (LMDB does not support it).
pub async fn build_repo(settings: &Settings, metrics: NostrMetrics) -> Arc<dyn NostrRepo> {
    if settings.pay_to_relay.enabled {
        panic!(
            "LMDB engine does not support Pay-to-Relay. Disable pay_to_relay in config."
        );
    }
    let repo = LmdbRepo::new(settings, metrics);
    repo.start().await.ok();
    Arc::new(repo)
}

/// Spawn a database writer that persists events to the database store.
pub async fn db_writer(
    repo: Arc<dyn NostrRepo>,
    settings: Settings,
    mut event_rx: tokio::sync::mpsc::Receiver<SubmittedEvent>,
    bcast_tx: tokio::sync::broadcast::Sender<Event>,
    metadata_tx: tokio::sync::broadcast::Sender<Event>,
    _payment_tx: tokio::sync::broadcast::Sender<PaymentMessage>,
    mut shutdown: tokio::sync::broadcast::Receiver<()>,
    admin_error_ctx: Option<Arc<admin_error::AdminErrorContext>>,
) -> Result<()> {
    let nip05_active = settings.verified_users.is_active();
    let pay_to_relay_enabled = settings.pay_to_relay.enabled;
    let cost_per_event = settings.pay_to_relay.cost_per_event;
    debug!("Pay to relay: {}", pay_to_relay_enabled);

    // get rate limit settings
    let rps_setting = settings.limits.messages_per_sec;
    let mut most_recent_rate_limit = Instant::now();
    let mut lim_opt = None;
    let clock = governor::clock::QuantaClock::default();
    if let Some(rps) = rps_setting {
        if rps > 0 {
            info!("Enabling rate limits for event creation ({}/sec)", rps);
            let quota = core::num::NonZeroU32::new(rps * 60).unwrap();
            lim_opt = Some(RateLimiter::direct(Quota::per_minute(quota)));
        }
    }
    loop {
        if shutdown.try_recv().is_ok() {
            info!("shutting down database writer");
            break;
        }
        // call blocking read on channel
        let next_event = event_rx.recv().await;
        // if the channel has closed, we will never get work
        if next_event.is_none() {
            break;
        }
        // track if an event write occurred; this is used to
        // update the rate limiter
        let mut event_write = false;
        let subm_event = next_event.unwrap();
        let event = subm_event.event;
        let notice_tx = subm_event.notice_tx;
        let user_balance = subm_event.user_balance;

        // send any metadata events to the NIP-05 verifier
        if nip05_active && event.is_kind_metadata() {
            // we are sending this prior to even deciding if we
            // persist it.  this allows the nip05 module to
            // inspect it, update if necessary, or persist a new
            // event and broadcast it itself.
            metadata_tx.send(event.clone()).ok();
        }

        let start = Instant::now();
        if event.is_ephemeral() {
            bcast_tx.send(event.clone()).ok();
            debug!(
                "published ephemeral event: {:?} from: {:?} in: {:?}",
                event.get_event_id_prefix(),
                event.get_author_prefix(),
                start.elapsed()
            );
            event_write = true;

            // send OK message
            notice_tx.try_send(Notice::saved(event.id)).ok();
        } else {
            match repo.write_event(&event).await {
                Ok(updated) => {
                    if updated == 0 {
                        trace!("ignoring duplicate or deleted event");
                        notice_tx.try_send(Notice::duplicate(event.id)).ok();
                    } else {
                        info!(
                            "persisted event: {:?} (kind: {}) from: {:?} in: {:?} (IP: {:?})",
                            event.get_event_id_prefix(),
                            event.kind,
                            event.get_author_prefix(),
                            start.elapsed(),
                            subm_event.source_ip,
                        );
                        event_write = true;
                        // send this out to all clients
                        bcast_tx.send(event.clone()).ok();
                        notice_tx.try_send(Notice::saved(event.id)).ok();
                    }
                }
                Err(err) => {
                    warn!("event insert failed: {:?}", err);
                    if let Some(ref ctx) = admin_error_ctx {
                        admin_error::send_admin_error_event(
                            &format!("event insert failed: {:?}", err),
                            ctx,
                            &bcast_tx,
                        );
                    }
                    let msg = "relay experienced an error trying to publish the latest event";
                    notice_tx.try_send(Notice::error(event.id, msg)).ok();
                }
            }
        }

        // use rate limit, if defined, and if an event was actually written.
        if event_write {
            // If pay to relay is disabled or the cost per event is 0
            // No need to update user balance
            if pay_to_relay_enabled && cost_per_event > 0 {
                // If the user balance is some, user was not on whitelist
                // Their balance should be reduced by the cost per event
                if let Some(_balance) = user_balance {
                    let _ = PublicKey::parse(&event.pubkey)?;
                    repo.update_account_balance(&event.pubkey, false, cost_per_event)
                        .await?;
                }
            }
            if let Some(ref lim) = lim_opt {
                if let Err(n) = lim.check() {
                    let wait_for = n.wait_time_from(clock.now());
                    // check if we have recently logged rate
                    // limits, but print out a message only once
                    // per second.
                    if most_recent_rate_limit.elapsed().as_secs() > 10 {
                        warn!(
                            "rate limit reached for event creation (sleep for {:?}) (suppressing future messages for 10 seconds)",
                            wait_for
                        );
                        // reset last rate limit message
                        most_recent_rate_limit = Instant::now();
                    }
                    // block event writes, allowing them to queue up
                    thread::sleep(wait_for);
                    continue;
                }
            }
        }
    }
    info!("database connection closed");
    Ok(())
}

/// Serialized event associated with a specific subscription request.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct QueryResult {
    /// Subscription identifier
    pub sub_id: String,
    /// Serialized event
    pub event: String,
}
