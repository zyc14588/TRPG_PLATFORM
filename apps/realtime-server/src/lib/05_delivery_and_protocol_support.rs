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
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty() && token.len() <= 2_048);
    if authorization.is_some() {
        return authorization;
    }
    browser_protocol_bearer(headers)
}

fn browser_protocol_bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)?
        .to_str()
        .ok()?
        .split(',')
        .map(str::trim)
        .find_map(|protocol| protocol.strip_prefix(REALTIME_AUTH_SUBPROTOCOL_PREFIX))
        .filter(|token| {
            !token.is_empty()
                && token.len() <= 2_048
                && token.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')
                })
        })
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
