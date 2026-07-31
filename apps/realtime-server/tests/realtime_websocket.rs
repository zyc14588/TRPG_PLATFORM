use std::collections::{BTreeSet, HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use realtime_server::{BackendError, RealtimeApplication, RealtimeBackend, RealtimeFuture};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;
use trpg_api::api_web_socket::RealtimeLimits;
use trpg_api::realtime_room_sync::{RoomKind, RoomSubscription};
use trpg_api::realtime_sync::{RealtimeEvent, ReplayBatch, ResyncRequired};
use trpg_api::websocket_protocol::{
    ClientEnvelope, ClientMessage, ConnectionBinding, ServerEnvelope, ServerMessage,
    CLOSE_AUTHORITY_CHANGED, CLOSE_AUTHORIZATION_REVOKED, CLOSE_RATE_LIMITED, CLOSE_SLOW_CONSUMER,
    REALTIME_PROTOCOL_VERSION,
};

const CAMPAIGN: &str = "campaign_ar07";

#[derive(Clone)]
struct TestBackend {
    state: Arc<Mutex<TestState>>,
}

#[derive(Clone)]
struct TestSession {
    token: String,
    user_id: String,
    seat: String,
    authority_epoch: u64,
}

#[derive(Clone)]
struct Member {
    user_id: String,
    seat: String,
    authority_epoch: u64,
    active: bool,
    groups: HashSet<String>,
}

#[derive(Clone)]
struct StoredEvent {
    event: RealtimeEvent,
    audience: Audience,
}

#[derive(Clone)]
enum Audience {
    Public,
    Keeper,
    Player(String),
    Group(String),
}

struct TestState {
    members: HashMap<String, Member>,
    events: Vec<StoredEvent>,
    earliest: u64,
    overflow_user: Option<String>,
}

impl TestState {
    fn fixture() -> Self {
        let members = [
            ("keeper", member("keeper_user", "human_kp", &["red"])),
            ("player_a", member("player_a", "player", &["red"])),
            ("player_b", member("player_b", "player", &[])),
            ("spectator", member("spectator_user", "spectator", &[])),
        ]
        .into_iter()
        .map(|(token, member)| (token.to_owned(), member))
        .collect();
        Self {
            members,
            events: vec![
                stored(1, "PublicSceneChanged", Audience::Public),
                stored(
                    2,
                    "SplitPartyClueRevealed",
                    Audience::Group("red".to_owned()),
                ),
                stored(
                    3,
                    "SecretRollResolved",
                    Audience::Player("player_a".to_owned()),
                ),
                stored(4, "KeeperNoteRecorded", Audience::Keeper),
            ],
            earliest: 1,
            overflow_user: None,
        }
    }
}

fn member(user_id: &str, seat: &str, groups: &[&str]) -> Member {
    Member {
        user_id: user_id.to_owned(),
        seat: seat.to_owned(),
        authority_epoch: 1,
        active: true,
        groups: groups.iter().map(|value| (*value).to_owned()).collect(),
    }
}

fn stored(cursor: u64, event_type: &str, audience: Audience) -> StoredEvent {
    let (visibility_label, visibility_subject, canary) = match &audience {
        Audience::Public => ("public".to_owned(), None, "CANARY_PUBLIC"),
        Audience::Keeper => ("keeper_only".to_owned(), None, "CANARY_KEEPER_NOTE"),
        Audience::Player(user) => (
            "private_to_player".to_owned(),
            Some(user.clone()),
            "CANARY_SECRET_ROLL",
        ),
        Audience::Group(group) => (
            "private_to_group".to_owned(),
            Some(group.clone()),
            "CANARY_SPLIT_RED",
        ),
    };
    let mut payload = RealtimeEvent::empty_payload();
    payload
        .as_object_mut()
        .expect("object payload")
        .insert("canary".to_owned(), canary.into());
    StoredEvent {
        event: RealtimeEvent {
            cursor,
            stream_version: cursor,
            event_type: event_type.to_owned(),
            event_schema_version: 1,
            campaign_id: CAMPAIGN.to_owned(),
            resource_type: "scene".to_owned(),
            resource_id: "scene_harbor".to_owned(),
            authority_mode: "human_kp".to_owned(),
            authority_epoch: 1,
            visibility_label,
            visibility_subject,
            provenance_kind: "rules_engine_decision".to_owned(),
            provenance_reference: format!("event_{cursor}"),
            provenance_recorded_by: "rules_engine".to_owned(),
            correlation_id: format!("correlation_{cursor}"),
            causation_id: format!("causation_{cursor}"),
            trace_id: format!("trace_{cursor}"),
            payload,
        },
        audience,
    }
}

impl RealtimeBackend for TestBackend {
    type Session = TestSession;

    fn authenticate<'a>(
        &'a self,
        bearer_token: &'a str,
        campaign_id: &'a str,
        _now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<Self::Session, BackendError>> {
        Box::pin(async move {
            if campaign_id != CAMPAIGN {
                return Err(BackendError::Authentication);
            }
            let member = self
                .state
                .lock()
                .map_err(|_| BackendError::Unavailable)?
                .members
                .get(bearer_token)
                .filter(|member| member.active)
                .cloned()
                .ok_or(BackendError::Authentication)?;
            Ok(TestSession {
                token: bearer_token.to_owned(),
                user_id: member.user_id,
                seat: member.seat,
                authority_epoch: member.authority_epoch,
            })
        })
    }

    fn binding(&self, session: &Self::Session) -> ConnectionBinding {
        ConnectionBinding {
            connection_id: format!("connection_{}", session.user_id),
            tenant_id: "tenant_ar07".to_owned(),
            user_id: session.user_id.clone(),
            campaign_id: CAMPAIGN.to_owned(),
            seat: session.seat.clone(),
            authority_mode: "human_kp".to_owned(),
            authority_epoch: session.authority_epoch,
        }
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
            self.reauthorize(session, subscription, now_unix_ms).await?;
            if subscription.kind == RoomKind::Campaign && subscription.room_id != CAMPAIGN {
                return Err(BackendError::Authorization);
            }
            match (cursor, resume_token) {
                (0, None) => Ok(()),
                (_, Some(token))
                    if token
                        == format!(
                            "resume_{}_{}_{}",
                            session.user_id, cursor, session.authority_epoch
                        ) =>
                {
                    Ok(())
                }
                _ => Err(BackendError::ResumeToken),
            }
        })
    }

    fn reauthorize<'a>(
        &'a self,
        session: &'a mut Self::Session,
        subscription: &'a RoomSubscription,
        _now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<ConnectionBinding, BackendError>> {
        Box::pin(async move {
            let member = self
                .state
                .lock()
                .map_err(|_| BackendError::Unavailable)?
                .members
                .get(&session.token)
                .filter(|member| member.active)
                .cloned()
                .ok_or(BackendError::Authorization)?;
            if member.authority_epoch != session.authority_epoch {
                return Err(BackendError::AuthorityChanged);
            }
            if subscription.kind == RoomKind::Group
                && member.seat != "human_kp"
                && !member.groups.contains(&subscription.room_id)
            {
                return Err(BackendError::Authorization);
            }
            session.seat = member.seat;
            Ok(self.binding(session))
        })
    }

    fn replay<'a>(
        &'a self,
        session: &'a Self::Session,
        subscription: &'a RoomSubscription,
        cursor: u64,
        limit: usize,
        _now_unix_ms: u64,
    ) -> RealtimeFuture<'a, Result<ReplayBatch, BackendError>> {
        Box::pin(async move {
            let state = self.state.lock().map_err(|_| BackendError::Unavailable)?;
            let latest = state
                .events
                .last()
                .map(|stored| stored.event.cursor)
                .unwrap_or(cursor);
            if (cursor > 0 && cursor.saturating_add(1) < state.earliest) || cursor > latest {
                return Err(BackendError::Resync(ResyncRequired {
                    reason: "cursor_outside_retained_history",
                    earliest_cursor: state.earliest.saturating_sub(1),
                    latest_cursor: latest,
                }));
            }
            let overflow = state.overflow_user.as_deref() == Some(session.user_id.as_str());
            let selected = state
                .events
                .iter()
                .filter(|stored| stored.event.cursor > cursor)
                .take(if overflow { usize::MAX } else { limit })
                .cloned()
                .collect::<Vec<_>>();
            let source_cursor = selected
                .last()
                .map(|stored| stored.event.cursor)
                .unwrap_or(cursor);
            let member = state
                .members
                .get(&session.token)
                .ok_or(BackendError::Authorization)?;
            let events = selected
                .into_iter()
                .filter(|stored| audience_allows(&stored.audience, member))
                .map(|stored| stored.event)
                .filter(|event| subscription.permits(event))
                .collect();
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
        _now_unix_ms: u64,
    ) -> Result<String, BackendError> {
        Ok(format!(
            "resume_{}_{}_{}",
            session.user_id, cursor, session.authority_epoch
        ))
    }

    fn check_readiness(&self) -> RealtimeFuture<'_, Result<(), BackendError>> {
        Box::pin(async { Ok(()) })
    }
}

fn audience_allows(audience: &Audience, member: &Member) -> bool {
    match audience {
        Audience::Public => true,
        Audience::Keeper => member.seat == "human_kp",
        Audience::Player(user) => member.seat == "human_kp" || member.user_id == *user,
        Audience::Group(group) => member.seat == "human_kp" || member.groups.contains(group),
    }
}

fn limits(max_pending_events: usize) -> RealtimeLimits {
    RealtimeLimits {
        max_connections: 32,
        max_message_bytes: 8 * 1_024,
        max_messages_per_window: 50,
        rate_window: Duration::from_secs(1),
        max_pending_events,
        replay_page_size: max_pending_events,
        heartbeat_interval: Duration::from_secs(5),
        heartbeat_timeout: Duration::from_secs(15),
        durable_poll_interval: Duration::from_millis(30),
        reauthorization_interval: Duration::from_millis(30),
        write_timeout: Duration::from_millis(250),
        subscribe_timeout: Duration::from_secs(1),
    }
}

async fn spawn(
    state: Arc<Mutex<TestState>>,
    limits: RealtimeLimits,
) -> (
    Arc<RealtimeApplication<TestBackend>>,
    SocketAddr,
    JoinHandle<()>,
) {
    let application =
        RealtimeApplication::new(TestBackend { state }, limits).expect("valid test application");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test realtime listener");
    let address = listener.local_addr().expect("test realtime address");
    let router = application.router();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("test realtime server");
    });
    (application, address, server)
}

async fn raw_http(address: SocketAddr, request: &str) -> String {
    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("connect raw HTTP client");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("write raw HTTP request");
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut response))
        .await
        .expect("raw HTTP response timeout")
        .expect("read raw HTTP response");
    String::from_utf8(response).expect("UTF-8 HTTP response")
}

fn campaign_subscription() -> RoomSubscription {
    RoomSubscription::campaign(CAMPAIGN).expect("valid campaign subscription")
}

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

fn event_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

enum Frame {
    Text(String),
    Close(u16),
    Other,
}

struct RawWebSocket {
    stream: tokio::net::TcpStream,
    buffered: Vec<u8>,
    seen_text: String,
}

impl RawWebSocket {
    async fn connect(address: SocketAddr, token: &str) -> Self {
        Self::connect_with_authentication(address, token, false).await
    }

    async fn connect_as_browser(address: SocketAddr, token: &str) -> Self {
        Self::connect_with_authentication(address, token, true).await
    }

    async fn connect_with_authentication(
        address: SocketAddr,
        token: &str,
        browser_protocol: bool,
    ) -> Self {
        let mut stream = tokio::net::TcpStream::connect(address)
            .await
            .expect("connect raw websocket");
        let authentication = if browser_protocol {
            format!("Sec-WebSocket-Protocol: trpg.realtime.v1, trpg.auth.{token}\r\n")
        } else {
            format!("Sec-WebSocket-Protocol: trpg.realtime.v1\r\nAuthorization: Bearer {token}\r\n")
        };
        let request = format!(
            "GET /ws/v1/campaigns/{CAMPAIGN}/rooms/{CAMPAIGN} HTTP/1.1\r\n\
             Host: {address}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Version: 13\r\n\
             {authentication}\r\n"
        );
        stream
            .write_all(request.as_bytes())
            .await
            .expect("write websocket handshake");
        let mut received = Vec::new();
        loop {
            let mut chunk = [0_u8; 1_024];
            let read = stream.read(&mut chunk).await.expect("read handshake");
            assert!(read > 0, "websocket handshake closed");
            received.extend_from_slice(&chunk[..read]);
            if let Some(end) = find_bytes(&received, b"\r\n\r\n") {
                let headers =
                    String::from_utf8(received[..end].to_vec()).expect("utf8 websocket response");
                assert!(
                    headers.starts_with("HTTP/1.1 101"),
                    "upgrade failed: {headers}"
                );
                assert!(
                    headers
                        .to_ascii_lowercase()
                        .contains("sec-websocket-protocol: trpg.realtime.v1"),
                    "subprotocol not negotiated: {headers}"
                );
                assert!(
                    !headers.contains(token),
                    "authentication subprotocol must not be echoed"
                );
                return Self {
                    stream,
                    buffered: received[end + 4..].to_vec(),
                    seen_text: String::new(),
                };
            }
        }
    }

    async fn expect_connected(&mut self) {
        let envelope = self.next_server().await;
        assert!(matches!(
            envelope.message,
            ServerMessage::Connected { ref binding, .. }
                if binding.tenant_id == "tenant_ar07"
                    && binding.campaign_id == CAMPAIGN
                    && binding.authority_epoch == 1
        ));
    }

    async fn subscribe(
        &mut self,
        request_id: &str,
        subscription: RoomSubscription,
        cursor: u64,
        resume_token: Option<String>,
    ) {
        self.send_client(ClientEnvelope {
            version: REALTIME_PROTOCOL_VERSION.to_owned(),
            request_id: request_id.to_owned(),
            message: ClientMessage::Subscribe {
                subscription,
                cursor,
                resume_token,
            },
        })
        .await;
        let subscribed = self.next_server().await;
        assert!(matches!(
            subscribed.message,
            ServerMessage::Subscribed { cursor: subscribed_cursor, .. }
                if subscribed_cursor == cursor
        ));
    }

    async fn pong(&mut self, request_id: &str) {
        self.send_client(ClientEnvelope {
            version: REALTIME_PROTOCOL_VERSION.to_owned(),
            request_id: request_id.to_owned(),
            message: ClientMessage::Pong { nonce: 1 },
        })
        .await;
    }

    async fn ack(&mut self, request_id: &str, cursor: u64) -> String {
        self.send_client(ClientEnvelope {
            version: REALTIME_PROTOCOL_VERSION.to_owned(),
            request_id: request_id.to_owned(),
            message: ClientMessage::Ack { cursor },
        })
        .await;
        loop {
            match self.next_server().await.message {
                ServerMessage::Acked {
                    request_id: ack_request,
                    cursor: ack_cursor,
                    resume_token,
                } if ack_request == request_id && ack_cursor == cursor => return resume_token,
                ServerMessage::Heartbeat { .. } => {}
                other => panic!("unexpected server message before ack: {other:?}"),
            }
        }
    }

    async fn collect_checkpoint(&mut self, expected: u64) -> (BTreeSet<String>, String) {
        let mut events = BTreeSet::new();
        loop {
            let envelope = tokio::time::timeout(Duration::from_secs(2), self.next_server())
                .await
                .expect("checkpoint timeout");
            match envelope.message {
                ServerMessage::Event { event, .. } => {
                    events.insert(event.event_type);
                }
                ServerMessage::Checkpoint {
                    cursor,
                    resume_token,
                } if cursor == expected => return (events, resume_token),
                ServerMessage::Checkpoint { cursor, .. } if cursor < expected => {}
                ServerMessage::Heartbeat { .. } => {}
                other => panic!("unexpected server message before checkpoint: {other:?}"),
            }
        }
    }

    async fn send_client(&mut self, envelope: ClientEnvelope) {
        let text = envelope.to_json().expect("serialize client envelope");
        self.send_text(&text).await;
    }

    async fn next_server(&mut self) -> ServerEnvelope {
        loop {
            match self.read_frame().await {
                Frame::Text(text) => {
                    self.seen_text.push_str(&text);
                    self.seen_text.push('\n');
                    return ServerEnvelope::parse_json(&text)
                        .unwrap_or_else(|error| panic!("server envelope {error}: {text}"));
                }
                Frame::Close(code) => panic!("unexpected websocket close {code}"),
                Frame::Other => {}
            }
        }
    }

    fn assert_never_saw(&self, canary: &str) {
        assert!(
            !self.seen_text.contains(canary),
            "unauthorized canary leaked in a server text frame: {canary}"
        );
    }

    async fn next_close(&mut self) -> u16 {
        loop {
            match self.read_frame().await {
                Frame::Close(code) => return code,
                Frame::Text(_) | Frame::Other => {}
            }
        }
    }

    async fn send_text(&mut self, text: &str) {
        let payload = text.as_bytes();
        let mut frame = Vec::with_capacity(payload.len() + 16);
        frame.push(0x81);
        if payload.len() < 126 {
            frame.push(0x80 | u8::try_from(payload.len()).expect("short payload"));
        } else {
            frame.push(0x80 | 126);
            frame.extend_from_slice(
                &u16::try_from(payload.len())
                    .expect("test websocket payload below 64KiB")
                    .to_be_bytes(),
            );
        }
        let mask = [0x11, 0x22, 0x33, 0x44];
        frame.extend_from_slice(&mask);
        frame.extend(
            payload
                .iter()
                .enumerate()
                .map(|(index, byte)| byte ^ mask[index % mask.len()]),
        );
        self.stream
            .write_all(&frame)
            .await
            .expect("write websocket frame");
    }

    async fn read_frame(&mut self) -> Frame {
        let first = self.read_exact_buffered(2).await;
        let opcode = first[0] & 0x0f;
        let masked = first[1] & 0x80 != 0;
        assert!(!masked, "server frames must not be masked");
        let mut length = u64::from(first[1] & 0x7f);
        if length == 126 {
            let bytes = self.read_exact_buffered(2).await;
            length = u64::from(u16::from_be_bytes([bytes[0], bytes[1]]));
        } else if length == 127 {
            let bytes = self.read_exact_buffered(8).await;
            length = u64::from_be_bytes(bytes.try_into().expect("eight length bytes"));
        }
        let payload = self
            .read_exact_buffered(usize::try_from(length).expect("bounded test frame"))
            .await;
        match opcode {
            0x1 => Frame::Text(String::from_utf8(payload).expect("utf8 websocket text")),
            0x8 => {
                let code = payload
                    .get(..2)
                    .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
                    .unwrap_or(1005);
                Frame::Close(code)
            }
            _ => Frame::Other,
        }
    }

    async fn read_exact_buffered(&mut self, length: usize) -> Vec<u8> {
        while self.buffered.len() < length {
            let mut chunk = vec![0_u8; 4_096];
            let read = self.stream.read(&mut chunk).await.expect("read websocket");
            assert!(read > 0, "websocket closed before frame completed");
            self.buffered.extend_from_slice(&chunk[..read]);
        }
        self.buffered.drain(..length).collect()
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
