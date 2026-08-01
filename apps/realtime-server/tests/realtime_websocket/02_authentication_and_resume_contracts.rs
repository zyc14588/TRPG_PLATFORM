#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn health_contract_and_authenticated_upgrade_fail_closed() {
    let state = Arc::new(Mutex::new(TestState::fixture()));
    let (_application, address, server) = spawn(state, limits(20)).await;
    let live = raw_http(
        address,
        &format!("GET /health/live HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"),
    )
    .await;
    assert!(live.starts_with("HTTP/1.1 200"));
    assert!(live.contains(r#""service":"realtime-server""#));
    assert!(live.contains(r#""status":"live""#));

    let ready = raw_http(
        address,
        &format!("GET /health/ready HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"),
    )
    .await;
    assert!(ready.starts_with("HTTP/1.1 200"));
    for check in [
        "configuration",
        "event_registry",
        "listener",
        "realtime_runtime",
    ] {
        assert!(ready.contains(check));
    }

    let unauthenticated = raw_http(
        address,
        &format!(
            "GET /ws/v1/campaigns/{CAMPAIGN}/rooms/{CAMPAIGN} HTTP/1.1\r\n\
             Host: {address}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade, close\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Protocol: trpg.realtime.v1\r\n\r\n"
        ),
    )
    .await;
    assert!(unauthenticated.starts_with("HTTP/1.1 401"));
    assert!(unauthenticated.contains("REALTIME_AUTHENTICATION_REQUIRED"));

    let missing_protocol = raw_http(
        address,
        &format!(
            "GET /ws/v1/campaigns/{CAMPAIGN}/rooms/{CAMPAIGN} HTTP/1.1\r\n\
             Host: {address}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade, close\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Version: 13\r\n\
             Authorization: Bearer player_a\r\n\r\n"
        ),
    )
    .await;
    assert!(missing_protocol.starts_with("HTTP/1.1 426"));
    assert!(missing_protocol.contains("REALTIME_SUBPROTOCOL_REQUIRED"));

    let not_found = raw_http(
        address,
        &format!("GET /not-found HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"),
    )
    .await;
    assert!(not_found.starts_with("HTTP/1.1 404"));
    assert!(not_found.contains(r#""error":"NOT_FOUND""#));
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn browser_subprotocol_authentication_is_accepted_without_echoing_the_token() {
    let state = Arc::new(Mutex::new(TestState::fixture()));
    let (_application, address, server) = spawn(state, limits(20)).await;
    let mut browser = RawWebSocket::connect_as_browser(address, "player_a").await;
    browser.expect_connected().await;
    browser
        .subscribe("browser_auth", campaign_subscription(), 0, None)
        .await;
    let delivery = browser.collect_checkpoint(4).await;
    assert!(delivery.0.contains("PublicSceneChanged"));
    assert!(delivery.0.contains("SecretRollResolved"));
    assert!(!delivery.0.contains("CANARY_KEEPER_NOTE"));
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn application_cancellation_closes_existing_connections() {
    let state = Arc::new(Mutex::new(TestState::fixture()));
    let (application, address, server) = spawn(state, limits(20)).await;
    let mut client = RawWebSocket::connect(address, "player_a").await;
    client.expect_connected().await;
    client
        .subscribe("shutdown", campaign_subscription(), 0, None)
        .await;
    let _ = client.collect_checkpoint(4).await;
    application.shutdown_connections();
    assert_eq!(client.next_close().await, 1001);
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn visibility_cursor_resume_and_notification_outage_use_server_truth() {
    let state = Arc::new(Mutex::new(TestState::fixture()));
    let (application, address, server) = spawn(Arc::clone(&state), limits(20)).await;
    let mut keeper = RawWebSocket::connect(address, "keeper").await;
    let mut player_a = RawWebSocket::connect(address, "player_a").await;
    let mut player_b = RawWebSocket::connect(address, "player_b").await;
    let mut spectator = RawWebSocket::connect(address, "spectator").await;

    for client in [&mut keeper, &mut player_a, &mut player_b, &mut spectator] {
        client.expect_connected().await;
        client
            .subscribe("initial", campaign_subscription(), 0, None)
            .await;
    }

    let keeper_delivery = keeper.collect_checkpoint(4).await;
    let player_a_delivery = player_a.collect_checkpoint(4).await;
    let player_b_delivery = player_b.collect_checkpoint(4).await;
    let spectator_delivery = spectator.collect_checkpoint(4).await;
    assert_eq!(
        keeper_delivery.0,
        event_set(&[
            "PublicSceneChanged",
            "SplitPartyClueRevealed",
            "SecretRollResolved",
            "KeeperNoteRecorded",
        ])
    );
    assert_eq!(
        player_a_delivery.0,
        event_set(&[
            "PublicSceneChanged",
            "SplitPartyClueRevealed",
            "SecretRollResolved",
        ])
    );
    assert_eq!(player_b_delivery.0, event_set(&["PublicSceneChanged"]));
    assert_eq!(spectator_delivery.0, event_set(&["PublicSceneChanged"]));
    player_a.assert_never_saw("CANARY_KEEPER_NOTE");
    for unauthorized in [&player_b, &spectator] {
        unauthorized.assert_never_saw("CANARY_SPLIT_RED");
        unauthorized.assert_never_saw("CANARY_SECRET_ROLL");
        unauthorized.assert_never_saw("CANARY_KEEPER_NOTE");
    }

    let player_a_resume = player_a.ack("ack_4", 4).await;
    drop(player_a);
    state.lock().expect("state").events.push(stored(
        5,
        "ReconnectPrivateDelta",
        Audience::Player("player_a".to_owned()),
    ));
    let mut resumed = RawWebSocket::connect(address, "player_a").await;
    resumed.expect_connected().await;
    resumed
        .subscribe("resume", campaign_subscription(), 4, Some(player_a_resume))
        .await;
    let resumed_delivery = resumed.collect_checkpoint(5).await;
    assert_eq!(
        resumed_delivery.0,
        event_set(&["ReconnectPrivateDelta"]),
        "resume must not duplicate cursors at or before the acknowledged checkpoint"
    );

    state
        .lock()
        .expect("state")
        .events
        .push(stored(6, "DurablePollRecovered", Audience::Public));
    let outage_delivery = resumed.collect_checkpoint(6).await;
    assert_eq!(
        outage_delivery.0,
        event_set(&["DurablePollRecovered"]),
        "durable poll must recover without a NATS notification"
    );

    state.lock().expect("state").events.push(stored(
        7,
        "NotificationWakeupRecovered",
        Audience::Public,
    ));
    application.notify_canonical_change();
    let notified = resumed.collect_checkpoint(7).await;
    assert_eq!(notified.0, event_set(&["NotificationWakeupRecovered"]));

    state.lock().expect("state").earliest = 4;
    let mut stale = RawWebSocket::connect(address, "player_b").await;
    stale.expect_connected().await;
    stale
        .subscribe(
            "stale",
            campaign_subscription(),
            1,
            Some("resume_player_b_1_1".to_owned()),
        )
        .await;
    let resync = stale.next_server().await;
    assert!(matches!(
        resync.message,
        ServerMessage::ResyncRequired {
            earliest_cursor: 3,
            latest_cursor: 7,
            ..
        }
    ));
    assert_eq!(stale.next_close().await, 4009);
    server.abort();
}
