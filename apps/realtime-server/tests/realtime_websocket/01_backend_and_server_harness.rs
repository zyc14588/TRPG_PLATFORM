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
