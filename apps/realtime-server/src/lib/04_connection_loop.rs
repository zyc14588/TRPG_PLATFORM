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
