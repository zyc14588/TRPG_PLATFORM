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

impl RealtimeBackend for ProductionRealtimeBackend {
    type Session = ProductionRealtimeSession;

    fn authenticate<'a>(
        &'a self,
        bearer_token: &'a str,
        campaign_id: &'a str,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<Self::Session, BackendError>> {
        let bearer_token = bearer_token.to_owned();
        let campaign_id = campaign_id.to_owned();
        let ordinal = self.connection_sequence.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            let identity = self
                .identity
                .authenticate(Some(&bearer_token), &campaign_id, now_unix_ms)
                .await
                .map_err(map_identity_error)?;
            Ok(ProductionRealtimeSession {
                identity,
                connection_id: format!(
                    "realtime_{}_{}_{}",
                    std::process::id(),
                    now_unix_ms,
                    ordinal
                ),
            })
        })
    }

    fn binding(&self, session: &Self::Session) -> ConnectionBinding {
        self.protocol_binding(session)
    }

    fn authorize_subscription<'a>(
        &'a self,
        session: &'a mut Self::Session,
        subscription: &'a RoomSubscription,
        cursor: u64,
        resume_token: Option<&'a str>,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<(), BackendError>> {
        Box::pin(async move {
            validate_subscription(session.identity.binding(), subscription)?;
            self.reauthorize(session, subscription, now_unix_ms).await?;
            match (cursor, resume_token) {
                (0, None) => Ok(()),
                (_, Some(token)) => self
                    .resume_tokens
                    .verify(token, &self.resume_binding(session), cursor, now_unix_ms)
                    .map(|_| ())
                    .map_err(|_| BackendError::ResumeToken),
                _ => Err(BackendError::ResumeToken),
            }
        })
    }

    fn reauthorize<'a>(
        &'a self,
        session: &'a mut Self::Session,
        subscription: &'a RoomSubscription,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<ConnectionBinding, BackendError>> {
        Box::pin(async move {
            let previous_epoch = session.identity.binding().authority_epoch;
            let previous_campaign = session.identity.binding().campaign_id.clone();
            let refreshed = self
                .identity
                .reauthorize(&mut session.identity, now_unix_ms)
                .await
                .map_err(map_identity_error)?;
            if subscription.kind == RoomKind::Group
                && !session
                    .identity
                    .can_subscribe_private_group(&subscription.room_id, now_unix_ms)
                    .await
                    .map_err(map_identity_error)?
            {
                return Err(BackendError::Authorization);
            }
            if refreshed.campaign_id != previous_campaign {
                return Err(BackendError::Authorization);
            }
            if refreshed.authority_epoch != previous_epoch {
                return Err(BackendError::AuthorityChanged);
            }
            Ok(self.protocol_binding(session))
        })
    }

    fn replay<'a>(
        &'a self,
        session: &'a Self::Session,
        subscription: &'a RoomSubscription,
        cursor: u64,
        limit: usize,
        now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<ReplayBatch, BackendError>> {
        Box::pin(async move {
            let binding = session.identity.binding();
            let bounds = self
                .canonical
                .replay_sequence_bounds(&binding.campaign_id)
                .await
                .map_err(|_| BackendError::Unavailable)?;
            let Some((earliest, latest)) = bounds else {
                return Ok(ReplayBatch {
                    events: Vec::new(),
                    source_cursor: cursor,
                    latest_cursor: cursor,
                    has_more: false,
                });
            };
            if (cursor > 0 && cursor.saturating_add(1) < earliest) || cursor > latest {
                return Err(BackendError::Resync(ResyncRequired {
                    reason: if cursor > latest {
                        "cursor_ahead_of_canonical_tip"
                    } else {
                        "cursor_older_than_retained_history"
                    },
                    earliest_cursor: earliest.saturating_sub(1),
                    latest_cursor: latest,
                }));
            }
            let after = i64::try_from(cursor).map_err(|_| BackendError::InvalidData)?;
            let limit = i64::try_from(limit.min(500)).map_err(|_| BackendError::InvalidData)?;
            let canonical = self
                .canonical
                .load_replay_page(&binding.campaign_id, after, limit)
                .await
                .map_err(|_| BackendError::Unavailable)?;
            let mut source_cursor = cursor;
            let mut events = Vec::with_capacity(canonical.len());
            for record in canonical {
                source_cursor =
                    u64::try_from(record.sequence).map_err(|_| BackendError::InvalidData)?;
                if u64::try_from(record.authority_contract_version)
                    .map_err(|_| BackendError::InvalidData)?
                    != binding.authority_epoch
                    || record.authority_mode.to_ascii_lowercase() != binding.authority_mode
                {
                    return Err(BackendError::AuthorityChanged);
                }
                let visibility = Visibility::try_from_parts(
                    &record.visibility_label,
                    nonempty(&record.visibility_subject),
                )
                .map_err(|_| BackendError::InvalidData)?;
                let campaign_id =
                    EntityId::new(&record.campaign_id).map_err(|_| BackendError::InvalidData)?;
                if !session
                    .identity
                    .can_view(&campaign_id, &visibility, now_unix_ms)
                    .await
                    .map_err(map_identity_error)?
                {
                    continue;
                }
                let event = realtime_event(record)?;
                if subscription.permits(&event) {
                    events.push(event);
                }
            }
            Ok(ReplayBatch {
                events,
                source_cursor,
                latest_cursor: latest,
                has_more: source_cursor < latest,
            })
        })
    }

    fn issue_resume_token(
        &self,
        session: &Self::Session,
        cursor: u64,
        now_unix_ms: u64,
    ) -> Result<String, BackendError> {
        let expires_at = now_unix_ms
            .checked_add(
                u64::try_from(self.resume_ttl.as_millis())
                    .map_err(|_| BackendError::InvalidData)?,
            )
            .ok_or(BackendError::InvalidData)?;
        self.resume_tokens
            .issue(
                &self.resume_binding(session),
                cursor,
                now_unix_ms,
                expires_at,
            )
            .map_err(|_| BackendError::Unavailable)
    }

    fn check_readiness(&self) -> RealtimeFuture<'_, Result<(), BackendError>> {
        Box::pin(async move {
            self.identity
                .check_readiness()
                .await
                .map_err(|_| BackendError::Unavailable)?;
            self.canonical
                .verify_integrity()
                .await
                .map_err(|_| BackendError::Unavailable)?;
            self.jetstream
                .check_readiness()
                .await
                .map_err(|_| BackendError::Unavailable)?;
            let mut cache = self.cache.clone();
            cache
                .check_readiness()
                .await
                .map_err(|_| BackendError::Unavailable)
        })
    }
}

pub struct RealtimeApplication<B: RealtimeBackend> {
    backend: Arc<B>,
    limits: RealtimeLimits,
    notifications: watch::Sender<u64>,
    cancellation: watch::Sender<bool>,
    connection_slots: Arc<Semaphore>,
}

impl<B: RealtimeBackend> RealtimeApplication<B> {
    pub fn new(backend: B, limits: RealtimeLimits) -> Result<Arc<Self>, BackendError> {
        limits.validate().map_err(|_| BackendError::InvalidData)?;
        let (notifications, _) = watch::channel(0);
        let (cancellation, _) = watch::channel(false);
        Ok(Arc::new(Self {
            backend: Arc::new(backend),
            connection_slots: Arc::new(Semaphore::new(limits.max_connections)),
            limits,
            notifications,
            cancellation,
        }))
    }

    pub fn router(self: &Arc<Self>) -> Router {
        Router::new()
            .route(
                "/ws/v1/campaigns/{campaign_id}/rooms/{room_id}",
                get(websocket_upgrade::<B>),
            )
            .route("/health/live", get(live))
            .route("/health/ready", get(readiness::<B>))
            .fallback(not_found)
            .with_state(Arc::clone(self))
    }

    pub fn notify_canonical_change(&self) {
        self.notifications.send_modify(|sequence| {
            *sequence = sequence.wrapping_add(1);
        });
    }

    pub fn shutdown_connections(&self) {
        let _ = self.cancellation.send(true);
    }

    pub fn backend(&self) -> Arc<B> {
        Arc::clone(&self.backend)
    }
}

async fn live() -> impl IntoResponse {
    let health = HealthState::new(
        ServiceKind::RealtimeServer,
        env!("CARGO_PKG_VERSION"),
        ServicePhase::Ready,
        Vec::new(),
    );
    json_response(StatusCode::OK, health.live_document(true).to_string())
}

async fn readiness<B: RealtimeBackend>(
    State(application): State<Arc<RealtimeApplication<B>>>,
) -> Response {
    let event_registry = match validate_event_registry() {
        Ok(()) => ComponentCheck::passing("event_registry", "canonical event registry valid"),
        Err(_) => ComponentCheck::failing("event_registry", "canonical event registry invalid"),
    };
    let runtime = match application.backend.check_readiness().await {
        Ok(()) => ComponentCheck::passing(
            "realtime_runtime",
            "identity, canonical replay, NATS, and Redis ready",
        ),
        Err(_) => ComponentCheck::failing(
            "realtime_runtime",
            "identity, canonical replay, NATS, or Redis unavailable",
        ),
    };
    let checks = vec![
        ComponentCheck::passing("configuration", "websocket transport configured"),
        event_registry,
        ComponentCheck::passing("listener", "HTTP/WebSocket listener accepting requests"),
        runtime,
    ];
    let phase = if checks.iter().all(|check| check.ready) {
        ServicePhase::Ready
    } else {
        ServicePhase::Degraded
    };
    let health = HealthState::new(
        ServiceKind::RealtimeServer,
        env!("CARGO_PKG_VERSION"),
        phase,
        checks,
    );
    json_response(
        if health.ready() {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        health.ready_document().to_string(),
    )
}

async fn not_found() -> Response {
    error_response(StatusCode::NOT_FOUND, "NOT_FOUND")
}

async fn websocket_upgrade<B: RealtimeBackend>(
    State(application): State<Arc<RealtimeApplication<B>>>,
    Path((campaign_id, room_id)): Path<(String, String)>,
    headers: HeaderMap,
    websocket: WebSocketUpgrade,
) -> Response {
    if validate_identifier(&campaign_id).is_err() || validate_identifier(&room_id).is_err() {
        return error_response(StatusCode::BAD_REQUEST, "REALTIME_ROOM_INVALID");
    }
    if !offers_realtime_subprotocol(&headers) {
        return error_response(
            StatusCode::UPGRADE_REQUIRED,
            "REALTIME_SUBPROTOCOL_REQUIRED",
        );
    }
    let bearer = match bearer_token(&headers) {
        Some(value) => value,
        None => {
            return error_response(StatusCode::UNAUTHORIZED, "REALTIME_AUTHENTICATION_REQUIRED")
        }
    };
    let permit = match Arc::clone(&application.connection_slots).try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "REALTIME_CONNECTION_CAPACITY_EXHAUSTED",
            )
        }
    };
    let session = match application
        .backend
        .authenticate(bearer, &campaign_id, now_unix_ms())
        .await
    {
        Ok(session) => session,
        Err(error) => {
            return error_response(
                if error == BackendError::Authentication {
                    StatusCode::UNAUTHORIZED
                } else {
                    StatusCode::FORBIDDEN
                },
                error.code(),
            )
        }
    };
    let max_message_bytes = application.limits.max_message_bytes;
    websocket
        .max_message_size(max_message_bytes)
        .max_frame_size(max_message_bytes)
        .protocols([REALTIME_SUBPROTOCOL])
        .on_upgrade(move |socket| connection_loop(socket, application, session, room_id, permit))
}

async fn connection_loop<B: RealtimeBackend>(
    mut socket: WebSocket,
    application: Arc<RealtimeApplication<B>>,
    mut session: B::Session,
    route_room_id: String,
    _permit: OwnedSemaphorePermit,
) {
    let mut server_sequence = 0_u64;
    let mut cancellation = application.cancellation.subscribe();
    let binding = application.backend.binding(&session);
    if send_server(
        &mut socket,
        &mut server_sequence,
        ServerMessage::Connected {
            binding,
            heartbeat_interval_ms: duration_ms(application.limits.heartbeat_interval),
        },
        application.limits.write_timeout,
    )
    .await
    .is_err()
    {
        return;
    }

    let first = tokio::select! {
        first = tokio::time::timeout(application.limits.subscribe_timeout, socket.recv()) => first,
        _ = cancellation.changed() => {
            close(&mut socket, 1001, "server_shutdown").await;
            return;
        }
    };
    let (request_id, subscription, mut cursor, resume_token) = match first {
        Ok(Some(Ok(Message::Text(text)))) => match ClientEnvelope::parse_json(text.as_str()) {
            Ok(ClientEnvelope {
                request_id,
                message:
                    ClientMessage::Subscribe {
                        subscription,
                        cursor,
                        resume_token,
                    },
                ..
            }) => (request_id, subscription, cursor, resume_token),
            _ => {
                close(&mut socket, 1008, "subscription_required").await;
                return;
            }
        },
        _ => {
            close(&mut socket, 1008, "subscription_timeout").await;
            return;
        }
    };
    if subscription.room_id != route_room_id {
        close(&mut socket, 1008, "route_room_mismatch").await;
        return;
    }
    if let Err(error) = application
        .backend
        .authorize_subscription(
            &mut session,
            &subscription,
            cursor,
            resume_token.as_deref(),
            now_unix_ms(),
        )
        .await
    {
        send_backend_error(&mut socket, &mut server_sequence, Some(request_id), error).await;
        return;
    }
    let token = match application
        .backend
        .issue_resume_token(&session, cursor, now_unix_ms())
    {
        Ok(token) => token,
        Err(error) => {
            send_backend_error(&mut socket, &mut server_sequence, Some(request_id), error).await;
            return;
        }
    };
    if send_server(
        &mut socket,
        &mut server_sequence,
        ServerMessage::Subscribed {
            request_id,
            subscription: subscription.clone(),
            cursor,
            resume_token: token,
        },
        application.limits.write_timeout,
    )
    .await
    .is_err()
    {
        return;
    }

    let mut subscription = subscription;
    let mut acknowledged_cursor = cursor;
    let mut last_activity = Instant::now();
    let mut rate = ConnectionRate::new(
        application.limits.max_messages_per_window,
        application.limits.rate_window,
    );
    let mut heartbeat = tokio::time::interval(application.limits.heartbeat_interval);
    let mut durable_poll = tokio::time::interval(application.limits.durable_poll_interval);
    let mut reauthorization = tokio::time::interval(application.limits.reauthorization_interval);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    durable_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    reauthorization.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut notifications = application.notifications.subscribe();

    if deliver(
        &application,
        &mut socket,
        &mut server_sequence,
        &session,
        &subscription,
        &mut cursor,
    )
    .await
    .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            _ = cancellation.changed() => {
                close(&mut socket, 1001, "server_shutdown").await;
                return;
            }
            incoming = socket.recv() => {
                let Some(Ok(message)) = incoming else { return; };
                last_activity = Instant::now();
                if !rate.allow(last_activity) {
                    close(&mut socket, CLOSE_RATE_LIMITED, "rate_limited").await;
                    return;
                }
                match message {
                    Message::Text(text) => {
                        let envelope = match ClientEnvelope::parse_json(text.as_str()) {
                            Ok(envelope) => envelope,
                            Err(error) => {
                                let _ = send_server(
                                    &mut socket,
                                    &mut server_sequence,
                                    ServerMessage::Error {
                                        request_id: None,
                                        code: error.code().to_owned(),
                                        retryable: false,
                                    },
                                    application.limits.write_timeout,
                                ).await;
                                close(&mut socket, 1008, "protocol_error").await;
                                return;
                            }
                        };
                        match envelope.message {
                            ClientMessage::Subscribe {
                                subscription: next_subscription,
                                cursor: next_cursor,
                                resume_token,
                            } => {
                                if let Err(error) = application.backend.authorize_subscription(
                                    &mut session,
                                    &next_subscription,
                                    next_cursor,
                                    resume_token.as_deref(),
                                    now_unix_ms(),
                                ).await {
                                    send_backend_error(
                                        &mut socket,
                                        &mut server_sequence,
                                        Some(envelope.request_id),
                                        error,
                                    ).await;
                                    return;
                                }
                                subscription = next_subscription;
                                cursor = next_cursor;
                                acknowledged_cursor = next_cursor;
                                let token = match application.backend.issue_resume_token(
                                    &session,
                                    cursor,
                                    now_unix_ms(),
                                ) {
                                    Ok(token) => token,
                                    Err(error) => {
                                        send_backend_error(
                                            &mut socket,
                                            &mut server_sequence,
                                            Some(envelope.request_id),
                                            error,
                                        ).await;
                                        return;
                                    }
                                };
                                if send_server(
                                    &mut socket,
                                    &mut server_sequence,
                                    ServerMessage::Subscribed {
                                        request_id: envelope.request_id,
                                        subscription: subscription.clone(),
                                        cursor,
                                        resume_token: token,
                                    },
                                    application.limits.write_timeout,
                                ).await.is_err() {
                                    return;
                                }
                            }
                            ClientMessage::Ack { cursor: ack } => {
                                if ack < acknowledged_cursor || ack > cursor {
                                    close(&mut socket, 1008, "ack_cursor_invalid").await;
                                    return;
                                }
                                acknowledged_cursor = ack;
                                let token = match application.backend.issue_resume_token(
                                    &session,
                                    ack,
                                    now_unix_ms(),
                                ) {
                                    Ok(token) => token,
                                    Err(error) => {
                                        send_backend_error(
                                            &mut socket,
                                            &mut server_sequence,
                                            Some(envelope.request_id),
                                            error,
                                        ).await;
                                        return;
                                    }
                                };
                                if send_server(
                                    &mut socket,
                                    &mut server_sequence,
                                    ServerMessage::Acked {
                                        request_id: envelope.request_id,
                                        cursor: ack,
                                        resume_token: token,
                                    },
                                    application.limits.write_timeout,
                                ).await.is_err() {
                                    return;
                                }
                            }
                            ClientMessage::Pong { .. } => {}
                        }
                    }
                    Message::Pong(_) | Message::Ping(_) => {}
                    Message::Close(_) => return,
                    Message::Binary(_) => {
                        close(&mut socket, 1003, "text_frames_required").await;
                        return;
                    }
                }
            }
            _ = heartbeat.tick() => {
                if last_activity.elapsed() >= application.limits.heartbeat_timeout {
                    close(&mut socket, 1001, "heartbeat_timeout").await;
                    return;
                }
                let heartbeat_nonce = server_sequence.saturating_add(1);
                if send_server(
                    &mut socket,
                    &mut server_sequence,
                    ServerMessage::Heartbeat { nonce: heartbeat_nonce },
                    application.limits.write_timeout,
                ).await.is_err() {
                    return;
                }
            }
            _ = durable_poll.tick() => {
                if deliver(
                    &application,
                    &mut socket,
                    &mut server_sequence,
                    &session,
                    &subscription,
                    &mut cursor,
                ).await.is_err() {
                    return;
                }
            }
            changed = notifications.changed() => {
                if changed.is_err() {
                    return;
                }
                if deliver(
                    &application,
                    &mut socket,
                    &mut server_sequence,
                    &session,
                    &subscription,
                    &mut cursor,
                ).await.is_err() {
                    return;
                }
            }
            _ = reauthorization.tick() => {
                let previous = application.backend.binding(&session);
                match application.backend.reauthorize(
                    &mut session,
                    &subscription,
                    now_unix_ms(),
                ).await {
                    Ok(current) => {
                        if current.seat != previous.seat
                            && send_server(
                                &mut socket,
                                &mut server_sequence,
                                ServerMessage::SubscriptionChanged {
                                    subscription: subscription.clone(),
                                    seat: current.seat,
                                    authority_epoch: current.authority_epoch,
                                },
                                application.limits.write_timeout,
                            )
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(error) => {
                        send_backend_error(&mut socket, &mut server_sequence, None, error).await;
                        return;
                    }
                }
            }
        }
    }
}

async fn deliver<B: RealtimeBackend>(
    application: &RealtimeApplication<B>,
    socket: &mut WebSocket,
    server_sequence: &mut u64,
    session: &B::Session,
    subscription: &RoomSubscription,
    cursor: &mut u64,
) -> Result<(), ()> {
    loop {
        let batch = match application
            .backend
            .replay(
                session,
                subscription,
                *cursor,
                application.limits.replay_page_size,
                now_unix_ms(),
            )
            .await
        {
            Ok(batch) => batch,
            Err(error) => {
                send_backend_error(socket, server_sequence, None, error).await;
                return Err(());
            }
        };
        if batch.events.len() > application.limits.max_pending_events {
            close(socket, CLOSE_SLOW_CONSUMER, "outbound_queue_limit").await;
            return Err(());
        }
        for event in batch.events {
            if send_server(
                socket,
                server_sequence,
                ServerMessage::Event {
                    cursor: event.cursor,
                    event: Box::new(event),
                },
                application.limits.write_timeout,
            )
            .await
            .is_err()
            {
                close(socket, CLOSE_SLOW_CONSUMER, "slow_consumer").await;
                return Err(());
            }
        }
        let advanced = batch.source_cursor > *cursor;
        *cursor = batch.source_cursor;
        if advanced {
            let token = application
                .backend
                .issue_resume_token(session, *cursor, now_unix_ms())
                .map_err(|_| ())?;
            if send_server(
                socket,
                server_sequence,
                ServerMessage::Checkpoint {
                    cursor: *cursor,
                    resume_token: token,
                },
                application.limits.write_timeout,
            )
            .await
            .is_err()
            {
                close(socket, CLOSE_SLOW_CONSUMER, "slow_consumer").await;
                return Err(());
            }
        }
        if !batch.has_more || !advanced {
            return Ok(());
        }
    }
}

async fn send_backend_error(
    socket: &mut WebSocket,
    sequence: &mut u64,
    request_id: Option<String>,
    error: BackendError,
) {
    if let BackendError::Resync(resync) = &error {
        let _ = send_server(
            socket,
            sequence,
            ServerMessage::ResyncRequired {
                request_id: request_id.unwrap_or_else(|| "server".to_owned()),
                reason: resync.reason.to_owned(),
                earliest_cursor: resync.earliest_cursor,
                latest_cursor: resync.latest_cursor,
            },
            Duration::from_secs(1),
        )
        .await;
    } else {
        let _ = send_server(
            socket,
            sequence,
            ServerMessage::Error {
                request_id,
                code: error.code().to_owned(),
                retryable: matches!(error, BackendError::Unavailable),
            },
            Duration::from_secs(1),
        )
        .await;
    }
    close(socket, error.close_code(), error.code()).await;
}

async fn send_server(
    socket: &mut WebSocket,
    sequence: &mut u64,
    message: ServerMessage,
    timeout: Duration,
) -> Result<(), ()> {
    *sequence = sequence.checked_add(1).ok_or(())?;
    let envelope = ServerEnvelope::new(*sequence, message).map_err(|_| ())?;
    let json = envelope.to_json().map_err(|_| ())?;
    tokio::time::timeout(timeout, socket.send(Message::Text(json.into())))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
}

async fn close(socket: &mut WebSocket, code: u16, reason: &str) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.to_owned().into(),
        })))
        .await;
}

fn realtime_event(record: CanonicalReplayEvent) -> Result<RealtimeEvent, BackendError> {
    Ok(RealtimeEvent {
        cursor: u64::try_from(record.sequence).map_err(|_| BackendError::InvalidData)?,
        stream_version: u64::try_from(record.stream_version)
            .map_err(|_| BackendError::InvalidData)?,
        event_type: record.event_type,
        event_schema_version: u32::try_from(record.event_schema_version)
            .map_err(|_| BackendError::InvalidData)?,
        campaign_id: record.campaign_id,
        resource_type: record.resource_type,
        resource_id: record.resource_id,
        authority_mode: record.authority_mode.to_ascii_lowercase(),
        authority_epoch: u64::try_from(record.authority_contract_version)
            .map_err(|_| BackendError::InvalidData)?,
        visibility_label: record.visibility_label.to_ascii_lowercase(),
        visibility_subject: nonempty(&record.visibility_subject).map(str::to_owned),
        provenance_kind: record.provenance_kind,
        provenance_reference: record.provenance_reference,
        provenance_recorded_by: record.provenance_recorded_by,
        correlation_id: record.correlation_id,
        causation_id: record.causation_id,
        trace_id: record.trace_id,
        payload: record.payload,
    })
}

fn protocol_binding(
    tenant_id: &str,
    connection_id: &str,
    binding: &RealtimeIdentityBinding,
) -> ConnectionBinding {
    ConnectionBinding {
        connection_id: connection_id.to_owned(),
        tenant_id: tenant_id.to_owned(),
        user_id: binding.user_id.clone(),
        campaign_id: binding.campaign_id.clone(),
        seat: binding.seat.clone(),
        authority_mode: binding.authority_mode.clone(),
        authority_epoch: binding.authority_epoch,
    }
}

fn validate_subscription(
    binding: &RealtimeIdentityBinding,
    subscription: &RoomSubscription,
) -> Result<(), BackendError> {
    subscription
        .validate()
        .map_err(|_| BackendError::Authorization)?;
    if subscription.kind == RoomKind::Campaign && subscription.room_id != binding.campaign_id {
        return Err(BackendError::Authorization);
    }
    Ok(())
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty() && token.len() <= 2_048)
}

fn offers_realtime_subprotocol(headers: &HeaderMap) -> bool {
    headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|protocol| protocol.trim() == REALTIME_SUBPROTOCOL)
        })
}

fn map_identity_error(error: RealtimeIdentityError) -> BackendError {
    match error {
        RealtimeIdentityError::Authentication => BackendError::Authentication,
        RealtimeIdentityError::Authorization => BackendError::Authorization,
        RealtimeIdentityError::Unavailable => BackendError::Unavailable,
        RealtimeIdentityError::InvalidData => BackendError::InvalidData,
    }
}

fn error_response(status: StatusCode, code: &'static str) -> Response {
    json_response(status, format!(r#"{{"error":"{code}"}}"#))
}

fn json_response(status: StatusCode, body: String) -> Response {
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

fn validate_identifier(value: &str) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(());
    }
    Ok(())
}

fn nonempty(value: &str) -> Option<&str> {
    (!value.is_empty() && value != "not_applicable").then_some(value)
}

fn now_unix_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

struct ConnectionRate {
    maximum: u32,
    window: Duration,
    started: Instant,
    observed: u32,
}

impl ConnectionRate {
    fn new(maximum: u32, window: Duration) -> Self {
        Self {
            maximum,
            window,
            started: Instant::now(),
            observed: 0,
        }
    }

    fn allow(&mut self, now: Instant) -> bool {
        if now.duration_since(self.started) >= self.window {
            self.started = now;
            self.observed = 0;
        }
        self.observed = self.observed.saturating_add(1);
        self.observed <= self.maximum
    }
}
