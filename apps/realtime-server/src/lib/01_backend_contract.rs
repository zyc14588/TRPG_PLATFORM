use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tokio::sync::{watch, OwnedSemaphorePermit, Semaphore};
use trpg_api::api_web_socket::RealtimeLimits;
use trpg_api::realtime_room_sync::{RoomKind, RoomSubscription};
use trpg_api::realtime_sync::{RealtimeEvent, ReplayBatch, ResyncRequired};
use trpg_api::websocket_protocol::{
    ClientEnvelope, ClientMessage, ConnectionBinding, ServerEnvelope, ServerMessage,
    CLOSE_AUTHENTICATION_REQUIRED, CLOSE_AUTHORITY_CHANGED, CLOSE_AUTHORIZATION_REVOKED,
    CLOSE_RATE_LIMITED, CLOSE_RESYNC_REQUIRED, CLOSE_SLOW_CONSUMER, REALTIME_SUBPROTOCOL,
};
use trpg_api::{EntityId, Visibility};
use trpg_contracts::{
    validate_event_registry, ComponentCheck, HealthState, ServiceKind, ServicePhase,
};
use trpg_data_eventing::cache_redis_impl::RedisProjectionCache;
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    CanonicalReplayEvent, PostgresCanonicalStore,
};
use trpg_data_eventing::realtime_identity::{
    PersistentRealtimeIdentity, RealtimeIdentityBinding, RealtimeIdentityError,
    RealtimeIdentitySession,
};
use trpg_data_eventing::realtime_resume::{RealtimeResumeBinding, RealtimeResumeTokenCodec};

pub type RealtimeFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

const REALTIME_AUTH_SUBPROTOCOL_PREFIX: &str = "trpg.auth.";

pub trait RealtimeBackend: Send + Sync + 'static {
    type Session: Send + Sync + 'static;

    fn authenticate<'a>(
        &'a self,
        bearer_token: &'a str,
        campaign_id: &'a str,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<Self::Session, BackendError>>;

    fn binding(&self, session: &Self::Session) -> ConnectionBinding;

    fn authorize_subscription<'a>(
        &'a self,
        session: &'a mut Self::Session,
        subscription: &'a RoomSubscription,
        cursor: u64,
        resume_token: Option<&'a str>,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<(), BackendError>>;

    fn reauthorize<'a>(
        &'a self,
        session: &'a mut Self::Session,
        subscription: &'a RoomSubscription,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<ConnectionBinding, BackendError>>;

    fn replay<'a>(
        &'a self,
        session: &'a Self::Session,
        subscription: &'a RoomSubscription,
        cursor: u64,
        limit: usize,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<ReplayBatch, BackendError>>;

    fn issue_resume_token(
        &self,
        session: &Self::Session,
        cursor: u64,
        now_unix_ms: u64,
    ) -> Result<String, BackendError>;

    fn check_readiness(&self) -> RealtimeFuture<'_, Result<(), BackendError>>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendError {
    Authentication,
    Authorization,
    AuthorityChanged,
    ResumeToken,
    Resync(ResyncRequired),
    Unavailable,
    InvalidData,
}

impl BackendError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Authentication => "REALTIME_AUTHENTICATION_REQUIRED",
            Self::Authorization => "REALTIME_SUBSCRIPTION_DENIED",
            Self::AuthorityChanged => "REALTIME_AUTHORITY_EPOCH_CHANGED",
            Self::ResumeToken => "REALTIME_RESUME_TOKEN_REJECTED",
            Self::Resync(_) => "REALTIME_RESYNC_REQUIRED",
            Self::Unavailable => "REALTIME_DEPENDENCY_UNAVAILABLE",
            Self::InvalidData => "REALTIME_CANONICAL_DATA_INVALID",
        }
    }

    const fn close_code(&self) -> u16 {
        match self {
            Self::Authentication => CLOSE_AUTHENTICATION_REQUIRED,
            Self::Authorization => CLOSE_AUTHORIZATION_REVOKED,
            Self::AuthorityChanged => CLOSE_AUTHORITY_CHANGED,
            Self::ResumeToken | Self::Resync(_) => CLOSE_RESYNC_REQUIRED,
            Self::Unavailable | Self::InvalidData => 1011,
        }
    }
}

pub struct ProductionRealtimeSession {
    identity: RealtimeIdentitySession,
    connection_id: String,
}

pub struct ProductionRealtimeBackend {
    tenant_id: String,
    identity: PersistentRealtimeIdentity,
    canonical: PostgresCanonicalStore,
    jetstream: JetStreamOutboxPublisher,
    cache: RedisProjectionCache,
    resume_tokens: RealtimeResumeTokenCodec,
    resume_ttl: Duration,
    connection_sequence: AtomicU64,
}

impl std::fmt::Debug for ProductionRealtimeBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductionRealtimeBackend")
            .field("tenant_id", &self.tenant_id)
            .field("identity", &"[PERSISTENT IDENTITY]")
            .field("canonical", &self.canonical)
            .field("jetstream", &self.jetstream)
            .field("cache", &"[REDIS PROJECTION CACHE]")
            .field("resume_tokens", &self.resume_tokens)
            .finish()
    }
}

impl ProductionRealtimeBackend {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant_id: impl Into<String>,
        identity: PersistentRealtimeIdentity,
        canonical: PostgresCanonicalStore,
        jetstream: JetStreamOutboxPublisher,
        cache: RedisProjectionCache,
        resume_key_id: impl Into<String>,
        resume_key: &[u8],
        resume_ttl: Duration,
    ) -> Result<Self, BackendError> {
        let tenant_id = tenant_id.into();
        validate_identifier(&tenant_id).map_err(|_| BackendError::InvalidData)?;
        if resume_ttl.is_zero() {
            return Err(BackendError::InvalidData);
        }
        Ok(Self {
            tenant_id,
            identity,
            canonical,
            jetstream,
            cache,
            resume_tokens: RealtimeResumeTokenCodec::derive(resume_key_id, resume_key)
                .map_err(|_| BackendError::InvalidData)?,
            resume_ttl,
            connection_sequence: AtomicU64::new(1),
        })
    }

    pub fn jetstream(&self) -> JetStreamOutboxPublisher {
        self.jetstream.clone()
    }

    fn resume_binding<'a>(
        &'a self,
        session: &'a ProductionRealtimeSession,
    ) -> RealtimeResumeBinding<'a> {
        let binding = session.identity.binding();
        RealtimeResumeBinding {
            tenant_id: &self.tenant_id,
            campaign_id: &binding.campaign_id,
            user_id: &binding.user_id,
            authority_epoch: binding.authority_epoch,
        }
    }

    fn protocol_binding(&self, session: &ProductionRealtimeSession) -> ConnectionBinding {
        protocol_binding(
            &self.tenant_id,
            &session.connection_id,
            session.identity.binding(),
        )
    }
}
