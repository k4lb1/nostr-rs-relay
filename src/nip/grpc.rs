//! GRPC event admission: external service decides whether to accept event.

use super::{NipContext, NipHandler, NipHandlerResult};
use crate::error::Result;
use crate::event::Event;
use async_trait::async_trait;
use tracing::warn;

/// GRPC event admission: when configured, calls external service for admission decision.
pub struct GrpcHandler;

impl GrpcHandler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NipHandler for GrpcHandler {
    fn name(&self) -> &'static str {
        "grpc"
    }

    async fn validate_event(
        &self,
        event: &Event,
        ctx: &NipContext<'_>,
    ) -> Result<NipHandlerResult> {
        let Some(grpc) = ctx.grpc else {
            return Ok(NipHandlerResult::Accept);
        };
        let nip05_addr = if event.is_kind_metadata() {
            event.get_nip05_addr()
        } else if let Some(repo) = ctx.repo {
            if ctx.settings.verified_users.is_active() {
                repo.get_latest_user_verification(&event.pubkey)
                    .await
                    .ok()
                    .map(|uv| uv.name)
            } else {
                None
            }
        } else {
            None
        };
        let mut guard = grpc.lock().await;
        match guard
            .admit_event(
                event,
                ctx.source_ip,
                ctx.origin.map(String::from),
                ctx.user_agent.map(String::from),
                nip05_addr,
                ctx.auth_pubkey.map(|s| s.to_vec()),
            )
            .await
        {
            Ok(decision) => {
                if decision.permitted() {
                    Ok(NipHandlerResult::Accept)
                } else {
                    Ok(NipHandlerResult::Reject {
                        reason: decision
                            .message()
                            .unwrap_or_else(|| "event rejected by relay policy".to_string()),
                    })
                }
            }
            Err(e) => {
                warn!("GRPC server error: {:?}", e);
                Ok(NipHandlerResult::Accept)
            }
        }
    }
}
