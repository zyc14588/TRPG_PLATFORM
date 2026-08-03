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
