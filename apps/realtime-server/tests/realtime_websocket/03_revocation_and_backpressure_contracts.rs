#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn membership_downgrade_revocation_and_authority_change_are_live() {
    let state = Arc::new(Mutex::new(TestState::fixture()));
    let (_application, address, server) = spawn(Arc::clone(&state), limits(20)).await;
    let mut client = RawWebSocket::connect(address, "player_b").await;
    client.expect_connected().await;
    client
        .subscribe("member", campaign_subscription(), 0, None)
        .await;
    let _ = client.collect_checkpoint(4).await;

    state
        .lock()
        .expect("state")
        .members
        .get_mut("player_b")
        .expect("member")
        .seat = "spectator".to_owned();
    loop {
        let changed = client.next_server().await;
        if matches!(
            changed.message,
            ServerMessage::SubscriptionChanged { ref seat, .. } if seat == "spectator"
        ) {
            break;
        }
    }
    state
        .lock()
        .expect("state")
        .members
        .get_mut("player_b")
        .expect("member")
        .active = false;
    loop {
        let error = client.next_server().await;
        if matches!(
            error.message,
            ServerMessage::Error { ref code, .. } if code == "REALTIME_SUBSCRIPTION_DENIED"
        ) {
            break;
        }
    }
    assert_eq!(client.next_close().await, CLOSE_AUTHORIZATION_REVOKED);

    {
        let mut state = state.lock().expect("state");
        let member = state.members.get_mut("player_a").expect("member");
        member.active = true;
        member.authority_epoch = 1;
    }
    let mut authority = RawWebSocket::connect(address, "player_a").await;
    authority.expect_connected().await;
    authority
        .subscribe("authority", campaign_subscription(), 0, None)
        .await;
    let _ = authority.collect_checkpoint(4).await;
    state
        .lock()
        .expect("state")
        .members
        .get_mut("player_a")
        .expect("member")
        .authority_epoch = 2;
    loop {
        let error = authority.next_server().await;
        if matches!(
            error.message,
            ServerMessage::Error { ref code, .. } if code == "REALTIME_AUTHORITY_EPOCH_CHANGED"
        ) {
            break;
        }
    }
    assert_eq!(authority.next_close().await, CLOSE_AUTHORITY_CHANGED);
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bounded_slow_consumer_and_rate_limit_do_not_block_other_connections() {
    let mut fixture = TestState::fixture();
    fixture.overflow_user = Some("player_a".to_owned());
    let state = Arc::new(Mutex::new(fixture));
    let mut constrained = limits(1);
    constrained.max_messages_per_window = 1;
    let (_application, address, server) = spawn(state, constrained).await;

    let mut slow = RawWebSocket::connect(address, "player_a").await;
    let mut healthy = RawWebSocket::connect(address, "keeper").await;
    slow.expect_connected().await;
    healthy.expect_connected().await;
    slow.subscribe("slow", campaign_subscription(), 0, None)
        .await;
    healthy
        .subscribe("healthy", campaign_subscription(), 0, None)
        .await;
    assert_eq!(slow.next_close().await, CLOSE_SLOW_CONSUMER);
    let healthy_delivery = healthy.collect_checkpoint(4).await;
    assert!(healthy_delivery.0.contains("KeeperNoteRecorded"));

    healthy.pong("pong_1").await;
    healthy.pong("pong_2").await;
    assert_eq!(healthy.next_close().await, CLOSE_RATE_LIMITED);
    server.abort();
}
