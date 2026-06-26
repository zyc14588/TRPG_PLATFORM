use async_trait::async_trait;
use auth::{
    AuditLog, AuditOutcome, EmailAddress, IdempotencyCheck, IdempotencyRecord,
    IdempotencyRepository, IdempotencyStatus, IdentityRepository, MagicLinkChallenge,
    OidcLoginRequest, OidcPort, RefreshSession, RefreshSessionRepository, Room, RoomId, RoomInvite,
    RoomInviteId, RoomMember, RoomPrivacyMode, RoomRepository, RoomRole, RoomWithRole, TokenHash,
    User, UserId, VisibilityScope,
};
use axum::{
    extract::{Path, Query, State},
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE, COOKIE, SET_COOKIE},
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use observability::prometheus_bootstrap_metrics;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    env, fs,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

const REFRESH_COOKIE: &str = "trpg_refresh";
const CSRF_COOKIE: &str = "trpg_csrf";
const CSRF_HEADER: &str = "x-csrf-token";
const ROOM_INVITE_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const IDEMPOTENCY_TTL_SECS: u64 = 24 * 60 * 60;
pub const OPENAPI_JSON: &str = include_str!("../../../schemas/openapi.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    Development,
    Test,
    Production,
}

impl FromStr for AuthMode {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "development" => Ok(Self::Development),
            "test" => Ok(Self::Test),
            "production" => Ok(Self::Production),
            _ => Err(anyhow::anyhow!("unknown auth mode: {value}")),
        }
    }
}

impl AuthMode {
    fn allows_development_defaults(self) -> bool {
        matches!(self, Self::Development | Self::Test)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub bind_addr: String,
    pub decision_baseline: String,
    pub region_id: String,
    pub auth_mode: AuthMode,
    pub auth_secret: String,
    pub access_token_ttl_secs: u64,
    pub refresh_token_ttl_secs: u64,
    pub magic_link_ttl_secs: u64,
    pub cookie_secure: bool,
    pub cookie_same_site: String,
}

#[derive(Debug, Deserialize, Default)]
struct ConfigFile {
    project: Option<ProjectSection>,
}

#[derive(Debug, Deserialize)]
struct ProjectSection {
    decision_baseline: Option<String>,
    region_id: Option<String>,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let path =
            env::var("TRPG_CONFIG_PATH").unwrap_or_else(|_| "config/default.toml".to_owned());
        let parsed = read_config_file(PathBuf::from(path))?;
        let auth_mode = load_auth_mode(env::var("TRPG_AUTH_MODE").ok())?;
        let auth_secret = env::var("TRPG_AUTH_SECRET").unwrap_or_else(|_| {
            if auth_mode.allows_development_defaults() {
                "development-secret-do-not-use".to_owned()
            } else {
                String::new()
            }
        });
        validate_auth_secret(auth_mode, &auth_secret)?;
        let cookie_secure = env_bool("TRPG_COOKIE_SECURE", auth_mode == AuthMode::Production)?;
        let cookie_same_site = env_same_site(cookie_secure)?;

        Ok(Self {
            bind_addr: env::var("TRPG_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_owned()),
            decision_baseline: parsed
                .project
                .as_ref()
                .and_then(|project| project.decision_baseline.clone())
                .unwrap_or_else(|| "2026-06-25-final".to_owned()),
            region_id: parsed
                .project
                .and_then(|project| project.region_id)
                .unwrap_or_else(|| "local-1".to_owned()),
            auth_mode,
            auth_secret,
            access_token_ttl_secs: env_u64("TRPG_ACCESS_TOKEN_TTL_SECS", 900),
            refresh_token_ttl_secs: env_u64("TRPG_REFRESH_TOKEN_TTL_SECS", 2_592_000),
            magic_link_ttl_secs: env_u64("TRPG_MAGIC_LINK_TTL_SECS", 600),
            cookie_secure,
            cookie_same_site,
        })
    }
}

fn load_auth_mode(value: Option<String>) -> anyhow::Result<AuthMode> {
    value
        .unwrap_or_else(|| "production".to_owned())
        .parse::<AuthMode>()
}

fn validate_auth_secret(auth_mode: AuthMode, auth_secret: &str) -> anyhow::Result<()> {
    if auth_mode != AuthMode::Production {
        return Ok(());
    }
    if auth_secret.is_empty() {
        return Err(anyhow::anyhow!(
            "TRPG_AUTH_SECRET is required when TRPG_AUTH_MODE=production"
        ));
    }
    if auth_secret == "development-secret-do-not-use" {
        return Err(anyhow::anyhow!(
            "TRPG_AUTH_SECRET must not use the development default in production"
        ));
    }
    if auth_secret.as_bytes().len() < 32 {
        return Err(anyhow::anyhow!(
            "TRPG_AUTH_SECRET must be at least 32 bytes when TRPG_AUTH_MODE=production"
        ));
    }
    Ok(())
}

fn env_bool(name: &str, fallback: bool) -> anyhow::Result<bool> {
    let value = match env::var(name) {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(err) => return Err(anyhow::anyhow!("{name} is invalid: {err}")),
    };
    parse_bool(name, value, fallback)
}

fn parse_bool(name: &str, value: Option<String>, fallback: bool) -> anyhow::Result<bool> {
    value.map_or(Ok(fallback), |value| {
        value
            .parse::<bool>()
            .map_err(|_| anyhow::anyhow!("{name} must be either true or false"))
    })
}

fn env_same_site(cookie_secure: bool) -> anyhow::Result<String> {
    let value = match env::var("TRPG_COOKIE_SAME_SITE") {
        Ok(value) => value,
        Err(env::VarError::NotPresent) => "Strict".to_owned(),
        Err(err) => return Err(anyhow::anyhow!("TRPG_COOKIE_SAME_SITE is invalid: {err}")),
    };
    validate_cookie_same_site(&value, cookie_secure)
}

fn validate_cookie_same_site(value: &str, cookie_secure: bool) -> anyhow::Result<String> {
    match value {
        "Strict" | "Lax" => Ok(value.to_owned()),
        "None" if cookie_secure => Ok(value.to_owned()),
        "None" => Err(anyhow::anyhow!(
            "TRPG_COOKIE_SAME_SITE=None requires TRPG_COOKIE_SECURE=true"
        )),
        _ => Err(anyhow::anyhow!(
            "TRPG_COOKIE_SAME_SITE must be one of Strict, Lax, or None"
        )),
    }
}

fn env_u64(name: &str, fallback: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

pub fn database_url_or_in_memory_from_env(config: &AppConfig) -> anyhow::Result<Option<String>> {
    database_url_or_in_memory(
        config,
        env::var("DATABASE_URL").ok(),
        env::var("TRPG_ALLOW_IN_MEMORY_STORE").ok(),
    )
}

fn database_url_or_in_memory(
    config: &AppConfig,
    database_url: Option<String>,
    allow_in_memory: Option<String>,
) -> anyhow::Result<Option<String>> {
    if let Some(database_url) = database_url.filter(|value| !value.is_empty()) {
        return Ok(Some(database_url));
    }
    if config.auth_mode == AuthMode::Production {
        return Err(anyhow::anyhow!(
            "DATABASE_URL is required when TRPG_AUTH_MODE=production"
        ));
    }
    if parse_bool("TRPG_ALLOW_IN_MEMORY_STORE", allow_in_memory, false)? {
        return Ok(None);
    }
    Err(anyhow::anyhow!(
        "InMemoryAuthStore requires TRPG_ALLOW_IN_MEMORY_STORE=true in development or test"
    ))
}

fn read_config_file(path: PathBuf) -> anyhow::Result<ConfigFile> {
    if !path.exists() {
        return Ok(ConfigFile::default());
    }

    let contents = fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    auth_store: Arc<dyn AuthStore>,
}

#[async_trait]
pub trait AuthStore: Send + Sync {
    async fn upsert_user(&self, user: &User) -> Result<(), ApiError>;
    async fn find_user_by_email(&self, email: &EmailAddress) -> Result<Option<User>, ApiError>;
    async fn find_user_by_id(&self, user_id: UserId) -> Result<Option<User>, ApiError>;
    async fn create_magic_link_challenge(
        &self,
        challenge: &MagicLinkChallenge,
    ) -> Result<(), ApiError>;
    async fn find_pending_magic_link_challenge_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<MagicLinkChallenge>, ApiError>;
    async fn consume_magic_link_challenge(&self, challenge_id: Uuid) -> Result<bool, ApiError>;
    async fn create_refresh_session(&self, session: &RefreshSession) -> Result<(), ApiError>;
    async fn save_refresh_session(&self, session: &RefreshSession) -> Result<(), ApiError>;
    async fn find_refresh_session_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<RefreshSession>, ApiError>;
    async fn create_room(&self, room: &Room) -> Result<(), ApiError>;
    async fn get_room(&self, room_id: RoomId) -> Result<Option<Room>, ApiError>;
    async fn get_room_as_member(
        &self,
        room_id: RoomId,
        member: &RoomMember,
    ) -> Result<Option<Room>, ApiError>;
    async fn get_room_for_invite(
        &self,
        room_id: RoomId,
        email: &EmailAddress,
        user_id: UserId,
    ) -> Result<Option<Room>, ApiError>;
    async fn list_rooms_for_user(&self, user_id: UserId) -> Result<Vec<RoomWithRole>, ApiError>;
    async fn add_room_member(&self, member: &RoomMember) -> Result<(), ApiError>;
    async fn get_room_member(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<Option<RoomMember>, ApiError>;
    async fn list_room_members_as_member(
        &self,
        room_id: RoomId,
        member: &RoomMember,
    ) -> Result<Vec<RoomMember>, ApiError>;
    async fn create_room_invite(&self, invite: &RoomInvite) -> Result<(), ApiError>;
    async fn find_pending_room_invite_for_user(
        &self,
        token_hash: &TokenHash,
        email: &EmailAddress,
        user_id: UserId,
    ) -> Result<Option<RoomInvite>, ApiError>;
    async fn accept_room_invite(
        &self,
        invite: &RoomInvite,
        member: &RoomMember,
    ) -> Result<(), ApiError>;
    async fn claim_idempotency_key(
        &self,
        record: &IdempotencyRecord,
        ttl: Duration,
    ) -> Result<IdempotencyCheck, ApiError>;
    async fn append_audit_log(&self, log: &AuditLog) -> Result<(), ApiError>;
}

#[async_trait]
impl AuthStore for storage::PostgresRepositories {
    async fn upsert_user(&self, user: &User) -> Result<(), ApiError> {
        IdentityRepository::upsert_user(self, user)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn find_user_by_email(&self, email: &EmailAddress) -> Result<Option<User>, ApiError> {
        IdentityRepository::find_user_by_email(self, email)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn find_user_by_id(&self, user_id: UserId) -> Result<Option<User>, ApiError> {
        self.find_user_by_id(user_id)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn create_magic_link_challenge(
        &self,
        challenge: &MagicLinkChallenge,
    ) -> Result<(), ApiError> {
        self.create_magic_link_challenge(challenge)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn find_pending_magic_link_challenge_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<MagicLinkChallenge>, ApiError> {
        self.find_pending_magic_link_challenge_by_token_hash(token_hash)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn consume_magic_link_challenge(&self, challenge_id: Uuid) -> Result<bool, ApiError> {
        self.consume_magic_link_challenge(challenge_id)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn create_refresh_session(&self, session: &RefreshSession) -> Result<(), ApiError> {
        RefreshSessionRepository::create_refresh_session(self, session)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn save_refresh_session(&self, session: &RefreshSession) -> Result<(), ApiError> {
        RefreshSessionRepository::save_refresh_session(self, session)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn find_refresh_session_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<RefreshSession>, ApiError> {
        RefreshSessionRepository::find_refresh_session_by_token_hash(self, token_hash)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn create_room(&self, room: &Room) -> Result<(), ApiError> {
        self.create_room_with_rls(room)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn get_room(&self, room_id: RoomId) -> Result<Option<Room>, ApiError> {
        RoomRepository::get_room(self, room_id)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn get_room_as_member(
        &self,
        room_id: RoomId,
        member: &RoomMember,
    ) -> Result<Option<Room>, ApiError> {
        self.get_room_with_rls(room_id, member)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn get_room_for_invite(
        &self,
        room_id: RoomId,
        email: &EmailAddress,
        user_id: UserId,
    ) -> Result<Option<Room>, ApiError> {
        self.get_room_for_invite_with_rls(room_id, email, user_id)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn list_rooms_for_user(&self, user_id: UserId) -> Result<Vec<RoomWithRole>, ApiError> {
        self.list_rooms_for_user_with_rls(user_id)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn add_room_member(&self, member: &RoomMember) -> Result<(), ApiError> {
        RoomRepository::add_room_member(self, member)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn get_room_member(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<Option<RoomMember>, ApiError> {
        self.get_room_member_with_rls(room_id, user_id)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn list_room_members_as_member(
        &self,
        room_id: RoomId,
        member: &RoomMember,
    ) -> Result<Vec<RoomMember>, ApiError> {
        self.list_room_members_with_rls(room_id, member)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn create_room_invite(&self, invite: &RoomInvite) -> Result<(), ApiError> {
        self.create_room_invite_with_rls(invite)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn find_pending_room_invite_for_user(
        &self,
        token_hash: &TokenHash,
        email: &EmailAddress,
        user_id: UserId,
    ) -> Result<Option<RoomInvite>, ApiError> {
        self.find_pending_room_invite_for_user_with_rls(token_hash, email, user_id)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn accept_room_invite(
        &self,
        invite: &RoomInvite,
        member: &RoomMember,
    ) -> Result<(), ApiError> {
        self.accept_room_invite_with_rls(invite, member)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn claim_idempotency_key(
        &self,
        record: &IdempotencyRecord,
        ttl: Duration,
    ) -> Result<IdempotencyCheck, ApiError> {
        IdempotencyRepository::claim_idempotency_key(self, record, ttl)
            .await
            .map_err(ApiError::from_repository)
    }

    async fn append_audit_log(&self, log: &AuditLog) -> Result<(), ApiError> {
        self.append_audit_log_with_rls(log)
            .await
            .map_err(ApiError::from_repository)
    }
}

#[derive(Default)]
pub struct InMemoryAuthStore {
    inner: Mutex<InMemoryAuthData>,
}

#[derive(Default)]
struct InMemoryAuthData {
    users_by_email: HashMap<String, User>,
    users_by_id: HashMap<Uuid, User>,
    magic_challenges_by_hash: HashMap<String, MagicLinkChallenge>,
    consumed_magic_challenges: HashMap<Uuid, ()>,
    refresh_sessions_by_id: HashMap<Uuid, RefreshSession>,
    rooms_by_id: HashMap<Uuid, Room>,
    room_members: HashMap<(Uuid, Uuid), RoomMember>,
    room_invites_by_hash: HashMap<String, RoomInvite>,
    idempotency_records: HashMap<(String, String), IdempotencyRecord>,
    audit_logs: Vec<AuditLog>,
}

#[async_trait]
impl AuthStore for InMemoryAuthStore {
    async fn upsert_user(&self, user: &User) -> Result<(), ApiError> {
        let mut inner = self.lock()?;
        inner
            .users_by_email
            .insert(user.email.as_str().to_owned(), user.clone());
        inner.users_by_id.insert(user.id.0, user.clone());
        Ok(())
    }

    async fn find_user_by_email(&self, email: &EmailAddress) -> Result<Option<User>, ApiError> {
        Ok(self.lock()?.users_by_email.get(email.as_str()).cloned())
    }

    async fn find_user_by_id(&self, user_id: UserId) -> Result<Option<User>, ApiError> {
        Ok(self.lock()?.users_by_id.get(&user_id.0).cloned())
    }

    async fn create_magic_link_challenge(
        &self,
        challenge: &MagicLinkChallenge,
    ) -> Result<(), ApiError> {
        self.lock()?
            .magic_challenges_by_hash
            .insert(challenge.token_hash.as_str().to_owned(), challenge.clone());
        Ok(())
    }

    async fn find_pending_magic_link_challenge_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<MagicLinkChallenge>, ApiError> {
        let inner = self.lock()?;
        let Some(challenge) = inner.magic_challenges_by_hash.get(token_hash.as_str()) else {
            return Ok(None);
        };
        if inner
            .consumed_magic_challenges
            .contains_key(&challenge.challenge_id)
        {
            return Ok(None);
        }
        Ok(Some(challenge.clone()))
    }

    async fn consume_magic_link_challenge(&self, challenge_id: Uuid) -> Result<bool, ApiError> {
        let mut inner = self.lock()?;
        if inner.consumed_magic_challenges.contains_key(&challenge_id) {
            return Ok(false);
        }
        inner.consumed_magic_challenges.insert(challenge_id, ());
        Ok(true)
    }

    async fn create_refresh_session(&self, session: &RefreshSession) -> Result<(), ApiError> {
        self.lock()?
            .refresh_sessions_by_id
            .insert(session.id.0, session.clone());
        Ok(())
    }

    async fn save_refresh_session(&self, session: &RefreshSession) -> Result<(), ApiError> {
        self.lock()?
            .refresh_sessions_by_id
            .insert(session.id.0, session.clone());
        Ok(())
    }

    async fn find_refresh_session_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<RefreshSession>, ApiError> {
        Ok(self
            .lock()?
            .refresh_sessions_by_id
            .values()
            .find(|session| {
                session.current_token_hash == *token_hash
                    || session.previous_token_hash.as_ref() == Some(token_hash)
            })
            .cloned())
    }

    async fn create_room(&self, room: &Room) -> Result<(), ApiError> {
        let mut inner = self.lock()?;
        inner.rooms_by_id.insert(room.id.0, room.clone());
        inner.room_members.insert(
            (room.id.0, room.owner_id.0),
            RoomMember {
                room_id: room.id,
                user_id: room.owner_id,
                role: RoomRole::Owner,
            },
        );
        Ok(())
    }

    async fn get_room(&self, room_id: RoomId) -> Result<Option<Room>, ApiError> {
        Ok(self.lock()?.rooms_by_id.get(&room_id.0).cloned())
    }

    async fn get_room_as_member(
        &self,
        room_id: RoomId,
        member: &RoomMember,
    ) -> Result<Option<Room>, ApiError> {
        if self
            .lock()?
            .room_members
            .contains_key(&(room_id.0, member.user_id.0))
        {
            self.get_room(room_id).await
        } else {
            Ok(None)
        }
    }

    async fn get_room_for_invite(
        &self,
        room_id: RoomId,
        email: &EmailAddress,
        _user_id: UserId,
    ) -> Result<Option<Room>, ApiError> {
        let inner = self.lock()?;
        let has_invite = inner.room_invites_by_hash.values().any(|invite| {
            invite.room_id == room_id
                && invite.invited_email == *email
                && invite.status == auth::RoomInviteStatus::Pending
        });
        if has_invite {
            Ok(inner.rooms_by_id.get(&room_id.0).cloned())
        } else {
            Ok(None)
        }
    }

    async fn list_rooms_for_user(&self, user_id: UserId) -> Result<Vec<RoomWithRole>, ApiError> {
        let inner = self.lock()?;
        let mut rooms = Vec::new();
        for member in inner.room_members.values() {
            if member.user_id != user_id {
                continue;
            }
            if let Some(room) = inner.rooms_by_id.get(&member.room_id.0) {
                rooms.push(RoomWithRole {
                    room: room.clone(),
                    role: member.role,
                });
            }
        }
        rooms.sort_by(|left, right| left.room.title.cmp(&right.room.title));
        Ok(rooms)
    }

    async fn add_room_member(&self, member: &RoomMember) -> Result<(), ApiError> {
        self.lock()?
            .room_members
            .insert((member.room_id.0, member.user_id.0), member.clone());
        Ok(())
    }

    async fn get_room_member(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<Option<RoomMember>, ApiError> {
        Ok(self
            .lock()?
            .room_members
            .get(&(room_id.0, user_id.0))
            .cloned())
    }

    async fn list_room_members_as_member(
        &self,
        room_id: RoomId,
        member: &RoomMember,
    ) -> Result<Vec<RoomMember>, ApiError> {
        if !matches!(
            member.role,
            RoomRole::Owner | RoomRole::Kp | RoomRole::AssistantKp
        ) {
            return Ok(vec![member.clone()]);
        }
        let mut members = self
            .lock()?
            .room_members
            .values()
            .filter(|member| member.room_id == room_id)
            .cloned()
            .collect::<Vec<_>>();
        members.sort_by_key(|member| member.user_id.0);
        Ok(members)
    }

    async fn create_room_invite(&self, invite: &RoomInvite) -> Result<(), ApiError> {
        self.lock()?
            .room_invites_by_hash
            .insert(invite.token_hash.as_str().to_owned(), invite.clone());
        Ok(())
    }

    async fn find_pending_room_invite_for_user(
        &self,
        token_hash: &TokenHash,
        email: &EmailAddress,
        _user_id: UserId,
    ) -> Result<Option<RoomInvite>, ApiError> {
        let Some(invite) = self
            .lock()?
            .room_invites_by_hash
            .get(token_hash.as_str())
            .cloned()
        else {
            return Ok(None);
        };
        if invite.status == auth::RoomInviteStatus::Pending && invite.invited_email == *email {
            Ok(Some(invite))
        } else {
            Ok(None)
        }
    }

    async fn accept_room_invite(
        &self,
        invite: &RoomInvite,
        member: &RoomMember,
    ) -> Result<(), ApiError> {
        let mut inner = self.lock()?;
        inner
            .room_invites_by_hash
            .insert(invite.token_hash.as_str().to_owned(), invite.clone());
        inner
            .room_members
            .insert((member.room_id.0, member.user_id.0), member.clone());
        Ok(())
    }

    async fn claim_idempotency_key(
        &self,
        record: &IdempotencyRecord,
        _ttl: Duration,
    ) -> Result<IdempotencyCheck, ApiError> {
        let mut inner = self.lock()?;
        let key = (record.scope.clone(), record.key.clone());
        let Some(existing) = inner.idempotency_records.get(&key) else {
            inner.idempotency_records.insert(key, record.clone());
            return Ok(IdempotencyCheck::Claimed);
        };
        if existing.request_hash == record.request_hash {
            Ok(IdempotencyCheck::Duplicate(existing.clone()))
        } else {
            Ok(IdempotencyCheck::Conflict)
        }
    }

    async fn append_audit_log(&self, log: &AuditLog) -> Result<(), ApiError> {
        self.lock()?.audit_logs.push(log.clone());
        Ok(())
    }
}

impl InMemoryAuthStore {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, InMemoryAuthData>, ApiError> {
        self.inner
            .lock()
            .map_err(|_| ApiError::Internal("auth store lock poisoned".to_owned()))
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub decision_baseline: String,
    pub region_id: String,
}

#[derive(Debug, Serialize)]
pub struct ReadyResponse {
    pub status: &'static str,
    pub checks: BTreeMap<&'static str, &'static str>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct MagicLinkRequestBody {
    pub email: String,
    pub redirect_uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MagicLinkRequestResponse {
    pub status: String,
    pub challenge_id: Uuid,
    pub expires_at_unix: u64,
    pub development_magic_link: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MagicLinkVerifyRequest {
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthSessionResponse {
    pub access_token: String,
    pub token_type: String,
    pub access_token_expires_at_unix: u64,
    pub csrf_token: String,
    pub user: UserDto,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OidcStartResponse {
    pub provider: String,
    pub authorization_url: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct OidcCallbackQuery {
    pub code: String,
    pub state: Option<String>,
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MeResponse {
    pub user: UserDto,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RoomDto {
    pub id: Uuid,
    pub title: String,
    pub system_name: String,
    pub privacy_mode: RoomPrivacyMode,
    pub version: i64,
    pub my_role: RoomRole,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRoomRequest {
    pub title: String,
    pub system_name: String,
    pub privacy_mode: RoomPrivacyMode,
    pub idempotency_key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RoomResponse {
    pub room: RoomDto,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ListRoomsResponse {
    pub rooms: Vec<RoomDto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateInvitationRequest {
    pub email: String,
    pub role: RoomRole,
    pub idempotency_key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CreateInvitationResponse {
    pub room_id: Uuid,
    pub invited_email: String,
    pub role: RoomRole,
    pub expires_at_unix: u64,
    pub token: String,
    pub invitation_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AcceptInvitationRequest {
    pub idempotency_key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RoomMemberDto {
    pub user_id: Uuid,
    pub role: RoomRole,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ListRoomMembersResponse {
    pub members: Vec<RoomMemberDto>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Internal(String),
}

impl ApiError {
    fn from_repository(error: auth::RepositoryError) -> Self {
        match error {
            auth::RepositoryError::Forbidden => Self::Forbidden("forbidden".to_owned()),
            auth::RepositoryError::NotFound => Self::NotFound("not found".to_owned()),
            auth::RepositoryError::Duplicate => Self::Conflict("duplicate".to_owned()),
            auth::RepositoryError::IdempotencyConflict => {
                Self::Conflict("idempotency key conflict".to_owned())
            }
            _ => Self::Internal("repository error".to_owned()),
        }
    }

    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error) = self.status_and_code();
        (
            status,
            Json(ErrorBody {
                error,
                message: self.to_string(),
            }),
        )
            .into_response()
    }
}

pub fn router(config: AppConfig) -> Router {
    assert!(
        config.auth_mode.allows_development_defaults(),
        "InMemoryAuthStore is only allowed in development or test"
    );
    router_with_auth_store(config, Arc::new(InMemoryAuthStore::default()))
}

pub fn router_with_auth_store(config: AppConfig, auth_store: Arc<dyn AuthStore>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .route("/openapi.json", get(openapi_json))
        .route("/api/auth/magic-link/request", post(magic_link_request))
        .route("/api/auth/magic-link/verify", post(magic_link_verify))
        .route("/api/auth/oidc/{provider}/start", get(oidc_start))
        .route("/api/auth/oidc/{provider}/callback", get(oidc_callback))
        .route("/api/auth/refresh", post(refresh))
        .route("/api/auth/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/rooms", post(create_room).get(list_rooms))
        .route("/api/rooms/{room_id}", get(get_room))
        .route(
            "/api/rooms/{room_id}/invitations",
            post(create_room_invitation),
        )
        .route(
            "/api/room-invitations/{token}/accept",
            post(accept_room_invitation),
        )
        .route("/api/rooms/{room_id}/members", get(list_room_members))
        .with_state(AppState { config, auth_store })
}

async fn healthz(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "trpg-platform-api",
        decision_baseline: state.config.decision_baseline,
        region_id: state.config.region_id,
    })
}

async fn readyz() -> Json<ReadyResponse> {
    let mut checks = BTreeMap::new();
    checks.insert("database", "not_checked_phase_1b_auth");
    checks.insert("redis", "not_checked_phase_1b_auth");
    checks.insert("object_storage", "not_checked_phase_1b_auth");

    Json(ReadyResponse {
        status: "ready_phase_1b_auth",
        checks,
    })
}

async fn metrics() -> &'static str {
    prometheus_bootstrap_metrics()
}

async fn openapi_json() -> Response {
    let mut response = OPENAPI_JSON.into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

async fn magic_link_request(
    State(state): State<AppState>,
    Json(request): Json<MagicLinkRequestBody>,
) -> Result<Json<MagicLinkRequestResponse>, ApiError> {
    let email = EmailAddress::parse(request.email)
        .map_err(|_| ApiError::BadRequest("email must be a valid address".to_owned()))?;
    if request.redirect_uri.trim().is_empty() {
        return Err(ApiError::BadRequest("redirect_uri is required".to_owned()));
    }

    let now = SystemTime::now();
    let token = generate_secret_token("ml");
    let token_hash = token_hash(&state.config, &token)?;
    let expires_at = now + Duration::from_secs(state.config.magic_link_ttl_secs);
    let challenge = MagicLinkChallenge {
        challenge_id: Uuid::new_v4(),
        email,
        token_hash,
        expires_at,
    };
    state
        .auth_store
        .create_magic_link_challenge(&challenge)
        .await?;

    Ok(Json(MagicLinkRequestResponse {
        status: "sent".to_owned(),
        challenge_id: challenge.challenge_id,
        expires_at_unix: system_time_unix(expires_at)?,
        development_magic_link: if state.config.auth_mode == AuthMode::Development {
            Some(append_query_token(&request.redirect_uri, &token))
        } else {
            None
        },
    }))
}

async fn magic_link_verify(
    State(state): State<AppState>,
    Json(request): Json<MagicLinkVerifyRequest>,
) -> Result<Response, ApiError> {
    let token_hash = token_hash(&state.config, &request.token)?;
    let Some(challenge) = state
        .auth_store
        .find_pending_magic_link_challenge_by_token_hash(&token_hash)
        .await?
    else {
        return Err(ApiError::Unauthorized("invalid magic link".to_owned()));
    };
    if SystemTime::now() >= challenge.expires_at {
        return Err(ApiError::Unauthorized("magic link expired".to_owned()));
    }
    if !state
        .auth_store
        .consume_magic_link_challenge(challenge.challenge_id)
        .await?
    {
        return Err(ApiError::Unauthorized("magic link already used".to_owned()));
    }
    let identity = auth::VerifiedIdentity {
        provider: auth::IdentityProviderKind::MagicLink,
        provider_subject: challenge.email.as_str().to_owned(),
        email: challenge.email,
        display_name: "TRPG Player".to_owned(),
    };
    issue_session_response(&state, identity).await
}

async fn oidc_start(
    State(state): State<AppState>,
    Path(provider): Path<String>,
) -> Result<Json<OidcStartResponse>, ApiError> {
    if state.config.auth_mode != AuthMode::Development {
        return Err(ApiError::BadRequest(
            "real OIDC providers are not configured in this phase".to_owned(),
        ));
    }
    let state_token = generate_secret_token("oidc_state");
    Ok(Json(OidcStartResponse {
        provider: provider.clone(),
        authorization_url: format!(
            "/api/auth/oidc/{provider}/callback?code=dev-user&state={state_token}"
        ),
        state: state_token,
    }))
}

async fn oidc_callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(query): Query<OidcCallbackQuery>,
) -> Result<Response, ApiError> {
    if state.config.auth_mode != AuthMode::Development {
        return Err(ApiError::BadRequest(
            "real OIDC providers are not configured in this phase".to_owned(),
        ));
    }
    let identity = auth::DevelopmentAuthProvider
        .exchange_oidc_code(OidcLoginRequest {
            provider,
            authorization_code: query.code,
            redirect_uri: query
                .redirect_uri
                .unwrap_or_else(|| "/auth/callback".to_owned()),
        })
        .await
        .map_err(|_| ApiError::Unauthorized("oidc login failed".to_owned()))?;
    issue_session_response(&state, identity).await
}

async fn refresh(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_csrf(&headers)?;
    let refresh_token = cookie_value(&headers, REFRESH_COOKIE)
        .ok_or_else(|| ApiError::Unauthorized("refresh session is required".to_owned()))?;
    let presented_hash = token_hash(&state.config, &refresh_token)?;
    let Some(mut session) = state
        .auth_store
        .find_refresh_session_by_token_hash(&presented_hash)
        .await?
    else {
        return Err(ApiError::Unauthorized(
            "refresh session is invalid".to_owned(),
        ));
    };

    let next_refresh_token = generate_secret_token("rt");
    let next_hash = token_hash(&state.config, &next_refresh_token)?;
    let now = SystemTime::now();
    if let Err(error) = session.rotate(&presented_hash, next_hash, now) {
        if !matches!(error, auth::RefreshSessionError::InvalidToken) {
            state.auth_store.save_refresh_session(&session).await?;
        }
        return Err(ApiError::Unauthorized(
            "refresh session is invalid".to_owned(),
        ));
    }
    state.auth_store.save_refresh_session(&session).await?;
    let user = state
        .auth_store
        .find_user_by_id(session.user_id)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("user no longer exists".to_owned()))?;
    let (body, csrf_token) = session_body(&state.config, &user)?;
    Ok(json_with_cookies(
        StatusCode::OK,
        &body,
        vec![
            refresh_cookie(
                &state.config,
                &next_refresh_token,
                Some(state.config.refresh_token_ttl_secs),
            ),
            csrf_cookie(
                &state.config,
                &csrf_token,
                Some(state.config.refresh_token_ttl_secs),
            ),
        ],
    ))
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_csrf(&headers)?;
    let refresh_token = cookie_value(&headers, REFRESH_COOKIE)
        .ok_or_else(|| ApiError::Unauthorized("refresh session is required".to_owned()))?;
    let presented_hash = token_hash(&state.config, &refresh_token)?;
    let Some(mut session) = state
        .auth_store
        .find_refresh_session_by_token_hash(&presented_hash)
        .await?
    else {
        return Err(ApiError::Unauthorized(
            "refresh session is invalid".to_owned(),
        ));
    };
    session.revoke(SystemTime::now());
    state.auth_store.save_refresh_session(&session).await?;

    Ok(json_with_cookies(
        StatusCode::OK,
        &serde_json::json!({ "status": "logged_out" }),
        vec![
            expired_cookie(&state.config, REFRESH_COOKIE, true),
            expired_cookie(&state.config, CSRF_COOKIE, false),
        ],
    ))
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, ApiError> {
    let user = authenticated_user(&state, &headers).await?;
    Ok(Json(MeResponse {
        user: user_dto(&user),
    }))
}

async fn create_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateRoomRequest>,
) -> Result<Json<RoomResponse>, ApiError> {
    require_idempotency_key(&request.idempotency_key)?;
    let user = authenticated_user(&state, &headers).await?;
    let title = non_empty_trimmed(&request.title, "title")?;
    let system_name = non_empty_trimmed(&request.system_name, "system_name")?;
    let room = Room {
        id: RoomId(Uuid::new_v4()),
        owner_id: user.id,
        title,
        system_name,
        privacy_mode: request.privacy_mode,
        version: 0,
    };
    let response = RoomResponse {
        room: room_dto(&room, RoomRole::Owner),
    };
    let request_hash = hash_json(&request)?;
    if let Some(duplicate) = claim_idempotent_response(
        &state,
        format!("user:{}:create_room", user.id.0),
        &request.idempotency_key,
        request_hash,
        &response,
    )
    .await?
    {
        return Ok(Json(duplicate));
    }

    state.auth_store.create_room(&room).await?;
    audit_success(
        &state,
        Some(room.id),
        Some(user.id),
        "room.create",
        "room",
        Some(room.id.0),
        serde_json::json!({
            "privacy_mode": room.privacy_mode,
            "system_name": room.system_name,
        }),
    )
    .await?;
    Ok(Json(response))
}

async fn list_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ListRoomsResponse>, ApiError> {
    let user = authenticated_user(&state, &headers).await?;
    let rooms = state
        .auth_store
        .list_rooms_for_user(user.id)
        .await?
        .into_iter()
        .map(|room| room_dto(&room.room, room.role))
        .collect();
    Ok(Json(ListRoomsResponse { rooms }))
}

async fn get_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<Json<RoomResponse>, ApiError> {
    let user = authenticated_user(&state, &headers).await?;
    let (room, member) = audited_room_for_member(&state, RoomId(room_id), user.id).await?;
    Ok(Json(RoomResponse {
        room: room_dto(&room, member.role),
    }))
}

async fn create_room_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    Json(request): Json<CreateInvitationRequest>,
) -> Result<Json<CreateInvitationResponse>, ApiError> {
    require_idempotency_key(&request.idempotency_key)?;
    let user = authenticated_user(&state, &headers).await?;
    let room_id = RoomId(room_id);
    let (_room, member) = audited_room_for_member(&state, room_id, user.id).await?;
    if member.role != RoomRole::Owner {
        audit_failure(
            &state,
            Some(room_id),
            Some(user.id),
            "access.denied",
            "room_invitation",
            Some(room_id.0),
            serde_json::json!({ "reason": "only_room_owner_can_invite" }),
        )
        .await?;
        return Err(ApiError::Forbidden("only room owner can invite".to_owned()));
    }
    if request.role == RoomRole::Owner {
        return Err(ApiError::BadRequest(
            "invitation role cannot be owner".to_owned(),
        ));
    }

    let invited_email = EmailAddress::parse(&request.email)
        .map_err(|_| ApiError::BadRequest("email must be a valid address".to_owned()))?;
    let token = generate_secret_token("ri");
    let expires_at = SystemTime::now() + Duration::from_secs(ROOM_INVITE_TTL_SECS);
    let invite = RoomInvite {
        id: RoomInviteId(Uuid::new_v4()),
        room_id,
        invited_email: invited_email.clone(),
        role: request.role,
        token_hash: token_hash(&state.config, &token)?,
        status: auth::RoomInviteStatus::Pending,
        invited_by: user.id,
        accepted_by: None,
        expires_at,
    };
    let response = CreateInvitationResponse {
        room_id: room_id.0,
        invited_email: invited_email.as_str().to_owned(),
        role: request.role,
        expires_at_unix: system_time_unix(expires_at)?,
        token: token.clone(),
        invitation_url: format!("/rooms/join?token={token}"),
    };
    let request_hash = hash_json(&request)?;
    if let Some(duplicate) = claim_idempotent_response(
        &state,
        format!("room:{}:create_invitation", room_id.0),
        &request.idempotency_key,
        request_hash,
        &response,
    )
    .await?
    {
        return Ok(Json(duplicate));
    }

    state.auth_store.create_room_invite(&invite).await?;
    audit_success(
        &state,
        Some(room_id),
        Some(user.id),
        "room.invite.create",
        "room_invite",
        Some(invite.id.0),
        serde_json::json!({
            "invited_email": invited_email.as_str(),
            "role": request.role,
        }),
    )
    .await?;
    Ok(Json(response))
}

async fn accept_room_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(token): Path<String>,
    Json(request): Json<AcceptInvitationRequest>,
) -> Result<Json<RoomResponse>, ApiError> {
    require_idempotency_key(&request.idempotency_key)?;
    let user = authenticated_user(&state, &headers).await?;
    let token_hash = token_hash(&state.config, &token)?;
    let Some(mut invite) = state
        .auth_store
        .find_pending_room_invite_for_user(&token_hash, &user.email, user.id)
        .await?
    else {
        audit_failure(
            &state,
            None,
            Some(user.id),
            "access.denied",
            "room_invite",
            None,
            serde_json::json!({ "reason": "invitation_not_found" }),
        )
        .await?;
        return Err(ApiError::NotFound("invitation not found".to_owned()));
    };
    if invite.invited_email != user.email {
        audit_failure(
            &state,
            Some(invite.room_id),
            Some(user.id),
            "access.denied",
            "room_invite",
            Some(invite.id.0),
            serde_json::json!({ "reason": "wrong_invited_user" }),
        )
        .await?;
        return Err(ApiError::Forbidden(
            "invitation is not for this user".to_owned(),
        ));
    }
    let room = state
        .auth_store
        .get_room_for_invite(invite.room_id, &user.email, user.id)
        .await?
        .ok_or_else(|| ApiError::NotFound("invitation not found".to_owned()))?;
    let response = RoomResponse {
        room: room_dto(&room, invite.role),
    };
    let request_hash = hash_json(&request)?;
    if let Some(duplicate) = claim_idempotent_response(
        &state,
        format!("invite:{}:accept", invite.token_hash.as_str()),
        &request.idempotency_key,
        request_hash,
        &response,
    )
    .await?
    {
        return Ok(Json(duplicate));
    }

    invite
        .accept(user.id, SystemTime::now())
        .map_err(|err| ApiError::BadRequest(err.to_string()))?;
    state
        .auth_store
        .accept_room_invite(
            &invite,
            &RoomMember {
                room_id: invite.room_id,
                user_id: user.id,
                role: invite.role,
            },
        )
        .await?;
    audit_success(
        &state,
        Some(invite.room_id),
        Some(user.id),
        "room.invite.accept",
        "room_invite",
        Some(invite.id.0),
        serde_json::json!({ "role": invite.role }),
    )
    .await?;
    Ok(Json(response))
}

async fn list_room_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<Json<ListRoomMembersResponse>, ApiError> {
    let user = authenticated_user(&state, &headers).await?;
    let room_id = RoomId(room_id);
    let (_room, member) = audited_room_for_member(&state, room_id, user.id).await?;
    let members = state
        .auth_store
        .list_room_members_as_member(room_id, &member)
        .await?
        .into_iter()
        .map(|member| RoomMemberDto {
            user_id: member.user_id.0,
            role: member.role,
        })
        .collect();
    Ok(Json(ListRoomMembersResponse { members }))
}

async fn issue_session_response(
    state: &AppState,
    identity: auth::VerifiedIdentity,
) -> Result<Response, ApiError> {
    let user = match state.auth_store.find_user_by_email(&identity.email).await? {
        Some(mut existing) => {
            existing.display_name = identity.display_name;
            existing
        }
        None => User {
            id: UserId(Uuid::new_v4()),
            email: identity.email,
            display_name: identity.display_name,
        },
    };
    state.auth_store.upsert_user(&user).await?;

    let refresh_token = generate_secret_token("rt");
    let refresh_hash = token_hash(&state.config, &refresh_token)?;
    let session = RefreshSession::active(
        user.id,
        refresh_hash,
        SystemTime::now(),
        Duration::from_secs(state.config.refresh_token_ttl_secs),
    );
    state.auth_store.create_refresh_session(&session).await?;
    audit_success(
        state,
        None,
        Some(user.id),
        "auth.login",
        "user",
        Some(user.id.0),
        serde_json::json!({ "provider": identity.provider }),
    )
    .await?;

    let (body, csrf_token) = session_body(&state.config, &user)?;
    Ok(json_with_cookies(
        StatusCode::OK,
        &body,
        vec![
            refresh_cookie(
                &state.config,
                &refresh_token,
                Some(state.config.refresh_token_ttl_secs),
            ),
            csrf_cookie(
                &state.config,
                &csrf_token,
                Some(state.config.refresh_token_ttl_secs),
            ),
        ],
    ))
}

fn session_body(
    config: &AppConfig,
    user: &User,
) -> Result<(AuthSessionResponse, String), ApiError> {
    let csrf_token = generate_secret_token("csrf");
    let access_token_expires_at =
        SystemTime::now() + Duration::from_secs(config.access_token_ttl_secs);
    Ok((
        AuthSessionResponse {
            access_token: sign_access_token(config, user.id, access_token_expires_at)?,
            token_type: "Bearer".to_owned(),
            access_token_expires_at_unix: system_time_unix(access_token_expires_at)?,
            csrf_token: csrf_token.clone(),
            user: user_dto(user),
        },
        csrf_token,
    ))
}

fn user_dto(user: &User) -> UserDto {
    UserDto {
        id: user.id.0,
        email: user.email.as_str().to_owned(),
        display_name: user.display_name.clone(),
    }
}

fn room_dto(room: &Room, my_role: RoomRole) -> RoomDto {
    RoomDto {
        id: room.id.0,
        title: room.title.clone(),
        system_name: room.system_name.clone(),
        privacy_mode: room.privacy_mode,
        version: room.version,
        my_role,
    }
}

async fn authenticated_user(state: &AppState, headers: &HeaderMap) -> Result<User, ApiError> {
    let claims = require_access_token(&state.config, headers)?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map(UserId)
        .map_err(|_| ApiError::Unauthorized("access token is invalid".to_owned()))?;
    state
        .auth_store
        .find_user_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("user no longer exists".to_owned()))
}

async fn room_for_member(
    state: &AppState,
    room_id: RoomId,
    user_id: UserId,
) -> Result<(Room, RoomMember), ApiError> {
    let member = state
        .auth_store
        .get_room_member(room_id, user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("room not found".to_owned()))?;
    let room = state
        .auth_store
        .get_room_as_member(room_id, &member)
        .await?
        .ok_or_else(|| ApiError::NotFound("room not found".to_owned()))?;
    Ok((room, member))
}

async fn audited_room_for_member(
    state: &AppState,
    room_id: RoomId,
    user_id: UserId,
) -> Result<(Room, RoomMember), ApiError> {
    match room_for_member(state, room_id, user_id).await {
        Ok(value) => Ok(value),
        Err(ApiError::NotFound(message)) => {
            audit_failure(
                state,
                Some(room_id),
                Some(user_id),
                "access.denied",
                "room",
                Some(room_id.0),
                serde_json::json!({ "reason": "not_room_member" }),
            )
            .await?;
            Err(ApiError::NotFound(message))
        }
        Err(error) => Err(error),
    }
}

async fn audit_success(
    state: &AppState,
    room_id: Option<RoomId>,
    actor_id: Option<UserId>,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    payload_json: serde_json::Value,
) -> Result<(), ApiError> {
    write_audit_log(
        state,
        audit_log(
            room_id,
            actor_id,
            action,
            target_type,
            target_id,
            AuditOutcome::Success,
            payload_json,
        ),
    )
    .await
}

async fn audit_failure(
    state: &AppState,
    room_id: Option<RoomId>,
    actor_id: Option<UserId>,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    payload_json: serde_json::Value,
) -> Result<(), ApiError> {
    write_audit_log(
        state,
        audit_log(
            room_id,
            actor_id,
            action,
            target_type,
            target_id,
            AuditOutcome::Failure,
            payload_json,
        ),
    )
    .await
}

fn audit_log(
    room_id: Option<RoomId>,
    actor_id: Option<UserId>,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    outcome: AuditOutcome,
    payload_json: serde_json::Value,
) -> AuditLog {
    AuditLog {
        room_id,
        actor_id,
        action: action.to_owned(),
        target_type: target_type.to_owned(),
        target_id,
        scope: VisibilityScope::SystemInternal,
        outcome,
        payload_json,
        request_id: Some(Uuid::new_v4()),
    }
}

async fn write_audit_log(state: &AppState, log: AuditLog) -> Result<(), ApiError> {
    state.auth_store.append_audit_log(&log).await
}

fn non_empty_trimmed(value: &str, field: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        Err(ApiError::BadRequest(format!("{field} is required")))
    } else {
        Ok(value.to_owned())
    }
}

fn require_idempotency_key(value: &str) -> Result<(), ApiError> {
    if value.trim().is_empty() {
        Err(ApiError::BadRequest(
            "idempotency_key is required".to_owned(),
        ))
    } else {
        Ok(())
    }
}

fn hash_json<T: Serialize>(value: &T) -> Result<String, ApiError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| ApiError::Internal("request hash failed".to_owned()))?;
    Ok(hex(&Sha256::digest(bytes)))
}

async fn claim_idempotent_response<T>(
    state: &AppState,
    scope: String,
    key: &str,
    request_hash: String,
    response: &T,
) -> Result<Option<T>, ApiError>
where
    T: Serialize + DeserializeOwned,
{
    // ponytail: stop point 2 stores the completed payload before the write;
    // move claim+write into one DB transaction when the repository exposes it.
    let response_json = serde_json::to_value(response)
        .map_err(|_| ApiError::Internal("idempotency response encode failed".to_owned()))?;
    let record = IdempotencyRecord {
        scope,
        key: key.to_owned(),
        request_hash,
        status: IdempotencyStatus::Completed,
        response_json: Some(response_json),
    };
    match state
        .auth_store
        .claim_idempotency_key(&record, Duration::from_secs(IDEMPOTENCY_TTL_SECS))
        .await?
    {
        IdempotencyCheck::Claimed => Ok(None),
        IdempotencyCheck::Duplicate(existing) => {
            let value = existing.response_json.ok_or_else(|| {
                ApiError::Conflict("idempotency response is unavailable".to_owned())
            })?;
            serde_json::from_value(value)
                .map(Some)
                .map_err(|_| ApiError::Internal("idempotency response decode failed".to_owned()))
        }
        IdempotencyCheck::Conflict => {
            Err(ApiError::Conflict("idempotency key conflict".to_owned()))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct AccessTokenClaims {
    sub: String,
    exp: u64,
    iat: u64,
    jti: String,
}

fn sign_access_token(
    config: &AppConfig,
    user_id: UserId,
    expires_at: SystemTime,
) -> Result<String, ApiError> {
    let claims = AccessTokenClaims {
        sub: user_id.0.to_string(),
        exp: system_time_unix(expires_at)?,
        iat: system_time_unix(SystemTime::now())?,
        jti: Uuid::new_v4().to_string(),
    };
    let payload = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&claims)
            .map_err(|_| ApiError::Internal("token encode failed".to_owned()))?,
    );
    let signature = sign_bytes(config, payload.as_bytes())?;
    Ok(format!("{payload}.{}", URL_SAFE_NO_PAD.encode(signature)))
}

fn require_access_token(
    config: &AppConfig,
    headers: &HeaderMap,
) -> Result<AccessTokenClaims, ApiError> {
    let header = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("access token is required".to_owned()))?;
    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("access token is required".to_owned()))?;
    verify_access_token(config, token)
}

fn verify_access_token(config: &AppConfig, token: &str) -> Result<AccessTokenClaims, ApiError> {
    let Some((payload, signature)) = token.split_once('.') else {
        return Err(ApiError::Unauthorized("access token is invalid".to_owned()));
    };
    let actual = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| ApiError::Unauthorized("access token is invalid".to_owned()))?;
    verify_signature(config, payload.as_bytes(), &actual)?;
    let claims: AccessTokenClaims = decode_json_b64(payload)?;
    if system_time_unix(SystemTime::now())? >= claims.exp {
        return Err(ApiError::Unauthorized("access token expired".to_owned()));
    }
    Ok(claims)
}

fn decode_json_b64<T: DeserializeOwned>(value: &str) -> Result<T, ApiError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ApiError::Unauthorized("access token is invalid".to_owned()))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::Unauthorized("access token is invalid".to_owned()))
}

fn sign_bytes(config: &AppConfig, bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(config.auth_secret.as_bytes())
        .map_err(|_| ApiError::Internal("token signing failed".to_owned()))?;
    mac.update(bytes);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn verify_signature(config: &AppConfig, bytes: &[u8], signature: &[u8]) -> Result<(), ApiError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(config.auth_secret.as_bytes())
        .map_err(|_| ApiError::Internal("token verification failed".to_owned()))?;
    mac.update(bytes);
    mac.verify_slice(signature)
        .map_err(|_| ApiError::Unauthorized("access token is invalid".to_owned()))
}

fn token_hash(config: &AppConfig, token: &str) -> Result<TokenHash, ApiError> {
    let mut hasher = Sha256::new();
    hasher.update(config.auth_secret.as_bytes());
    hasher.update(b":");
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    TokenHash::new(hex(&digest)).map_err(|_| ApiError::Internal("token hash failed".to_owned()))
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn generate_secret_token(prefix: &str) -> String {
    format!(
        "{prefix}_{}_{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn require_csrf(headers: &HeaderMap) -> Result<(), ApiError> {
    let header = headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::Forbidden("csrf token is required".to_owned()))?;
    let cookie = cookie_value(headers, CSRF_COOKIE)
        .ok_or_else(|| ApiError::Forbidden("csrf token is required".to_owned()))?;
    if header.is_empty() || header != cookie {
        return Err(ApiError::Forbidden("csrf token is invalid".to_owned()));
    }
    Ok(())
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    for value in headers.get_all(COOKIE) {
        let Ok(value) = value.to_str() else {
            continue;
        };
        for pair in value.split(';') {
            let Some((cookie_name, cookie_value)) = pair.trim().split_once('=') else {
                continue;
            };
            if cookie_name == name {
                return Some(cookie_value.to_owned());
            }
        }
    }
    None
}

fn refresh_cookie(config: &AppConfig, token: &str, max_age_secs: Option<u64>) -> String {
    cookie(config, REFRESH_COOKIE, token, true, max_age_secs)
}

fn csrf_cookie(config: &AppConfig, token: &str, max_age_secs: Option<u64>) -> String {
    cookie(config, CSRF_COOKIE, token, false, max_age_secs)
}

fn expired_cookie(config: &AppConfig, name: &str, http_only: bool) -> String {
    let mut value = cookie(config, name, "", http_only, Some(0));
    value.push_str("; Expires=Thu, 01 Jan 1970 00:00:00 GMT");
    value
}

fn cookie(
    config: &AppConfig,
    name: &str,
    value: &str,
    http_only: bool,
    max_age_secs: Option<u64>,
) -> String {
    let mut cookie = format!(
        "{name}={value}; Path=/; SameSite={}",
        config.cookie_same_site
    );
    if let Some(max_age_secs) = max_age_secs {
        cookie.push_str(&format!("; Max-Age={max_age_secs}"));
    }
    if http_only {
        cookie.push_str("; HttpOnly");
    }
    if config.cookie_secure {
        cookie.push_str("; Secure");
    }
    cookie
}

fn json_with_cookies<T: Serialize>(status: StatusCode, body: &T, cookies: Vec<String>) -> Response {
    let mut response = (status, Json(body)).into_response();
    for cookie in cookies {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(SET_COOKIE, value);
        }
    }
    response
}

fn append_query_token(redirect_uri: &str, token: &str) -> String {
    let separator = if redirect_uri.contains('?') { '&' } else { '?' };
    format!("{redirect_uri}{separator}token={token}")
}

fn system_time_unix(value: SystemTime) -> Result<u64, ApiError> {
    value
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::Internal("time conversion failed".to_owned()))
        .map(|duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use http::Request;
    use serde_json::json;
    use sqlx::Executor;
    use std::collections::BTreeSet;
    use tower::ServiceExt;

    fn test_config() -> AppConfig {
        AppConfig {
            bind_addr: "127.0.0.1:0".to_owned(),
            decision_baseline: "test".to_owned(),
            region_id: "local-test".to_owned(),
            auth_mode: AuthMode::Development,
            auth_secret: "test-secret".to_owned(),
            access_token_ttl_secs: 900,
            refresh_token_ttl_secs: 3_600,
            magic_link_ttl_secs: 600,
            cookie_secure: false,
            cookie_same_site: "Strict".to_owned(),
        }
    }

    async fn json_request<T: Serialize>(
        app: Router,
        method: &str,
        uri: &str,
        body: T,
        headers: Vec<(&str, String)>,
    ) -> axum::response::Response {
        let mut builder = Request::builder().method(method).uri(uri);
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        app.oneshot(
            builder
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).expect("json body")))
                .expect("request should build"),
        )
        .await
        .expect("router should respond")
    }

    async fn empty_request(
        app: Router,
        method: &str,
        uri: &str,
        headers: Vec<(&str, String)>,
    ) -> axum::response::Response {
        let mut builder = Request::builder().method(method).uri(uri);
        for (name, value) in headers {
            builder = builder.header(name, value);
        }
        app.oneshot(builder.body(Body::empty()).expect("request should build"))
            .await
            .expect("router should respond")
    }

    async fn read_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        serde_json::from_slice(&bytes).expect("body should be json")
    }

    fn set_cookie_headers(response: &axum::response::Response) -> Vec<String> {
        response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .map(ToOwned::to_owned)
            .collect()
    }

    fn cookie_header(set_cookies: &[String]) -> String {
        set_cookies
            .iter()
            .filter_map(|cookie| cookie.split(';').next())
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn named_cookie_pair(set_cookies: &[String], name: &str) -> String {
        set_cookies
            .iter()
            .filter_map(|cookie| cookie.split(';').next())
            .find(|pair| pair.starts_with(&format!("{name}=")))
            .expect("cookie should exist")
            .to_owned()
    }

    fn token_from_link(link: &str) -> String {
        link.split("token=")
            .nth(1)
            .expect("dev link should include token")
            .to_owned()
    }

    fn openapi_path_methods(document: &serde_json::Value) -> BTreeSet<(String, String)> {
        let paths = document["paths"].as_object().expect("openapi paths");
        let methods = ["get", "post", "put", "patch", "delete"];
        let mut routes = BTreeSet::new();
        for (path, item) in paths {
            let item = item.as_object().expect("path item");
            for method in methods {
                if item.contains_key(method) {
                    routes.insert((path.clone(), method.to_ascii_uppercase()));
                }
            }
        }
        routes
    }

    fn expected_route_contract() -> BTreeSet<(String, String)> {
        [
            ("/healthz", "GET"),
            ("/readyz", "GET"),
            ("/metrics", "GET"),
            ("/openapi.json", "GET"),
            ("/api/auth/magic-link/request", "POST"),
            ("/api/auth/magic-link/verify", "POST"),
            ("/api/auth/oidc/{provider}/start", "GET"),
            ("/api/auth/oidc/{provider}/callback", "GET"),
            ("/api/auth/refresh", "POST"),
            ("/api/auth/logout", "POST"),
            ("/api/me", "GET"),
            ("/api/rooms", "GET"),
            ("/api/rooms", "POST"),
            ("/api/rooms/{room_id}", "GET"),
            ("/api/rooms/{room_id}/invitations", "POST"),
            ("/api/room-invitations/{token}/accept", "POST"),
            ("/api/rooms/{room_id}/members", "GET"),
        ]
        .into_iter()
        .map(|(path, method)| (path.to_owned(), method.to_owned()))
        .collect()
    }

    fn concrete_route(path: &str) -> String {
        match path {
            "/api/auth/oidc/{provider}/callback" => {
                "/api/auth/oidc/dev/callback?code=dev-user".to_owned()
            }
            _ => path
                .replace("{provider}", "dev")
                .replace("{room_id}", "00000000-0000-0000-0000-000000000001")
                .replace("{token}", "invite-token"),
        }
    }

    async fn login(app: Router, email: &str) -> (AuthSessionResponse, Vec<String>) {
        let request_response = json_request(
            app.clone(),
            "POST",
            "/api/auth/magic-link/request",
            json!({ "email": email, "redirect_uri": "http://localhost/auth/callback" }),
            vec![],
        )
        .await;
        assert_eq!(request_response.status(), StatusCode::OK);
        let request_body: MagicLinkRequestResponse = read_json(request_response).await;
        let token = token_from_link(
            request_body
                .development_magic_link
                .as_deref()
                .expect("dev link"),
        );

        let verify_response = json_request(
            app,
            "POST",
            "/api/auth/magic-link/verify",
            json!({ "token": token }),
            vec![],
        )
        .await;
        assert_eq!(verify_response.status(), StatusCode::OK);
        let cookies = set_cookie_headers(&verify_response);
        let body: AuthSessionResponse = read_json(verify_response).await;
        (body, cookies)
    }

    fn auth_header(session: &AuthSessionResponse) -> Vec<(&str, String)> {
        vec![("authorization", format!("Bearer {}", session.access_token))]
    }

    fn audit_logs(store: &InMemoryAuthStore) -> Vec<AuditLog> {
        store.lock().expect("audit store lock").audit_logs.clone()
    }

    async fn postgres_router_with_rls_role() -> Option<(Router, storage::PostgresRepositories)> {
        let database_url = std::env::var("DATABASE_URL").ok()?;
        let repo = storage::PostgresRepositories::connect(&database_url)
            .await
            .ok()?;
        storage::MIGRATOR.run(repo.pool()).await.ok()?;
        ensure_rls_test_role(&repo).await.ok()?;
        let app_repo = repo.clone().with_rls_role("trpg_rls_test").ok()?;
        Some((
            router_with_auth_store(test_config(), Arc::new(app_repo)),
            repo,
        ))
    }

    async fn ensure_rls_test_role(repo: &storage::PostgresRepositories) -> Result<(), sqlx::Error> {
        repo.pool()
            .execute(
                r#"
                DO $$
                BEGIN
                    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'trpg_rls_test') THEN
                        CREATE ROLE trpg_rls_test;
                    END IF;
                END
                $$;
                "#,
            )
            .await?;
        repo.pool()
            .execute("GRANT USAGE ON SCHEMA public, app TO trpg_rls_test")
            .await?;
        repo.pool()
            .execute(
                "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO trpg_rls_test",
            )
            .await?;
        repo.pool()
            .execute("GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA app TO trpg_rls_test")
            .await?;
        Ok(())
    }

    async fn create_test_room(
        app: Router,
        session: &AuthSessionResponse,
        title: &str,
        idempotency_key: &str,
    ) -> RoomResponse {
        let response = json_request(
            app,
            "POST",
            "/api/rooms",
            json!({
                "title": title,
                "system_name": "generic_percentile",
                "privacy_mode": "private_hybrid",
                "idempotency_key": idempotency_key
            }),
            auth_header(session),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        read_json(response).await
    }

    async fn invite_user(
        app: Router,
        owner: &AuthSessionResponse,
        room_id: Uuid,
        email: &str,
        idempotency_key: &str,
    ) -> CreateInvitationResponse {
        let response = json_request(
            app,
            "POST",
            &format!("/api/rooms/{room_id}/invitations"),
            json!({
                "email": email,
                "role": "pl",
                "idempotency_key": idempotency_key
            }),
            auth_header(owner),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        read_json(response).await
    }

    async fn accept_invite(
        app: Router,
        session: &AuthSessionResponse,
        token: &str,
        idempotency_key: &str,
    ) -> RoomResponse {
        let response = json_request(
            app,
            "POST",
            &format!("/api/room-invitations/{token}/accept"),
            json!({ "idempotency_key": idempotency_key }),
            auth_header(session),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        read_json(response).await
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        let app = router(test_config());
        let response = empty_request(app, "GET", "/healthz", vec![]).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn missing_auth_mode_defaults_to_production() {
        assert_eq!(
            load_auth_mode(None).expect("auth mode"),
            AuthMode::Production
        );
    }

    #[test]
    fn production_without_database_url_fails() {
        let mut config = test_config();
        config.auth_mode = AuthMode::Production;

        let error = database_url_or_in_memory(&config, None, Some("true".to_owned()))
            .expect_err("production must require DATABASE_URL");

        assert!(error.to_string().contains("DATABASE_URL is required"));
    }

    #[test]
    fn production_rejects_short_auth_secret() {
        let error = validate_auth_secret(AuthMode::Production, "short")
            .expect_err("production secret must be long enough");

        assert!(error.to_string().contains("at least 32 bytes"));
    }

    #[test]
    fn production_rejects_development_secret() {
        let error = validate_auth_secret(AuthMode::Production, "development-secret-do-not-use")
            .expect_err("production must reject development default");

        assert!(error.to_string().contains("development default"));
    }

    #[test]
    fn production_rejects_insecure_samesite_none() {
        let error = validate_cookie_same_site("None", false)
            .expect_err("SameSite=None must require Secure");

        assert!(error
            .to_string()
            .contains("requires TRPG_COOKIE_SECURE=true"));
    }

    #[test]
    fn invalid_bool_env_is_rejected() {
        let error = parse_bool("TRPG_COOKIE_SECURE", Some("yes".to_owned()), false)
            .expect_err("bool env parser must reject non-bool values");

        assert!(error.to_string().contains("either true or false"));
    }

    #[test]
    fn development_can_use_in_memory_only_when_explicit() {
        let config = test_config();
        let error = database_url_or_in_memory(&config, None, None)
            .expect_err("development in-memory store must be explicit");
        assert!(error
            .to_string()
            .contains("TRPG_ALLOW_IN_MEMORY_STORE=true"));

        assert_eq!(
            database_url_or_in_memory(&config, None, Some("true".to_owned()))
                .expect("explicit development in-memory store"),
            None
        );
    }

    #[tokio::test]
    async fn openapi_contract_is_readable_and_matches_registered_routes() {
        let document: serde_json::Value =
            serde_json::from_str(OPENAPI_JSON).expect("openapi json should parse");
        assert_eq!(document["openapi"], "3.1.0");
        assert_eq!(openapi_path_methods(&document), expected_route_contract());

        let app = router(test_config());
        let served_response = empty_request(app.clone(), "GET", "/openapi.json", vec![]).await;
        assert_eq!(served_response.status(), StatusCode::OK);
        let served_document: serde_json::Value = read_json(served_response).await;
        assert_eq!(served_document, document);

        for (path, method) in expected_route_contract() {
            let response =
                empty_request(app.clone(), &method, &concrete_route(&path), vec![]).await;
            assert_ne!(
                response.status(),
                StatusCode::NOT_FOUND,
                "{method} {path} is documented but not registered"
            );
            assert_ne!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{method} {path} is documented with the wrong method"
            );
        }
    }

    #[tokio::test]
    async fn magic_link_is_single_use() {
        let app = router(test_config());
        let request_response = json_request(
            app.clone(),
            "POST",
            "/api/auth/magic-link/request",
            json!({ "email": "owner@example.test", "redirect_uri": "http://localhost/auth/callback" }),
            vec![],
        )
        .await;
        let request_body: MagicLinkRequestResponse = read_json(request_response).await;
        let token = token_from_link(
            request_body
                .development_magic_link
                .as_deref()
                .expect("dev link"),
        );

        let first = json_request(
            app.clone(),
            "POST",
            "/api/auth/magic-link/verify",
            json!({ "token": token }),
            vec![],
        )
        .await;
        assert_eq!(first.status(), StatusCode::OK);
        let second = json_request(
            app,
            "POST",
            "/api/auth/magic-link/verify",
            json!({ "token": token }),
            vec![],
        )
        .await;
        assert_eq!(second.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn magic_link_expires() {
        let mut config = test_config();
        config.magic_link_ttl_secs = 0;
        let app = router(config);
        let request_response = json_request(
            app.clone(),
            "POST",
            "/api/auth/magic-link/request",
            json!({ "email": "owner@example.test", "redirect_uri": "http://localhost/auth/callback" }),
            vec![],
        )
        .await;
        let request_body: MagicLinkRequestResponse = read_json(request_response).await;
        let token = token_from_link(
            request_body
                .development_magic_link
                .as_deref()
                .expect("dev link"),
        );
        let verify = json_request(
            app,
            "POST",
            "/api/auth/magic-link/verify",
            json!({ "token": token }),
            vec![],
        )
        .await;
        assert_eq!(verify.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn production_magic_link_response_does_not_expose_token() {
        let mut config = test_config();
        config.auth_mode = AuthMode::Production;
        config.cookie_secure = true;
        let app = router_with_auth_store(config, Arc::new(InMemoryAuthStore::default()));
        let response = json_request(
            app,
            "POST",
            "/api/auth/magic-link/request",
            json!({ "email": "owner@example.test", "redirect_uri": "https://app.example/auth/callback" }),
            vec![],
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: MagicLinkRequestResponse = read_json(response).await;
        assert!(body.development_magic_link.is_none());
    }

    #[tokio::test]
    async fn refresh_rotates_refresh_cookie_and_rejects_reuse() {
        let app = router(test_config());
        let (login_body, login_cookies) = login(app.clone(), "owner@example.test").await;
        let old_refresh = named_cookie_pair(&login_cookies, REFRESH_COOKIE);
        assert!(login_cookies
            .iter()
            .any(|cookie| cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict")));

        let refresh_response = empty_request(
            app.clone(),
            "POST",
            "/api/auth/refresh",
            vec![
                ("cookie", cookie_header(&login_cookies)),
                (CSRF_HEADER, login_body.csrf_token.clone()),
            ],
        )
        .await;
        assert_eq!(refresh_response.status(), StatusCode::OK);
        let new_cookies = set_cookie_headers(&refresh_response);
        let new_refresh = named_cookie_pair(&new_cookies, REFRESH_COOKIE);
        assert_ne!(old_refresh, new_refresh);

        let reused = empty_request(
            app,
            "POST",
            "/api/auth/refresh",
            vec![
                ("cookie", cookie_header(&login_cookies)),
                (CSRF_HEADER, login_body.csrf_token),
            ],
        )
        .await;
        assert_eq!(reused.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn logout_revokes_current_refresh_session() {
        let app = router(test_config());
        let (login_body, login_cookies) = login(app.clone(), "owner@example.test").await;
        let logout_response = empty_request(
            app.clone(),
            "POST",
            "/api/auth/logout",
            vec![
                ("cookie", cookie_header(&login_cookies)),
                (CSRF_HEADER, login_body.csrf_token.clone()),
            ],
        )
        .await;
        assert_eq!(logout_response.status(), StatusCode::OK);

        let refresh_response = empty_request(
            app,
            "POST",
            "/api/auth/refresh",
            vec![
                ("cookie", cookie_header(&login_cookies)),
                (CSRF_HEADER, login_body.csrf_token),
            ],
        )
        .await;
        assert_eq!(refresh_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn csrf_is_required_for_refresh() {
        let app = router(test_config());
        let (_login_body, login_cookies) = login(app.clone(), "owner@example.test").await;
        let response = empty_request(
            app,
            "POST",
            "/api/auth/refresh",
            vec![("cookie", cookie_header(&login_cookies))],
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn me_rejects_unauthenticated_access() {
        let app = router(test_config());
        let response = empty_request(app, "GET", "/api/me", vec![]).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn me_returns_user_for_access_token() {
        let app = router(test_config());
        let (login_body, _login_cookies) = login(app.clone(), "owner@example.test").await;
        let response = empty_request(
            app,
            "GET",
            "/api/me",
            vec![(
                "authorization",
                format!("Bearer {}", login_body.access_token),
            )],
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: MeResponse = read_json(response).await;
        assert_eq!(body.user.email, "owner@example.test");
    }

    #[tokio::test]
    async fn room_creation_rejects_unauthenticated_access() {
        let app = router(test_config());
        let response = json_request(
            app,
            "POST",
            "/api/rooms",
            json!({
                "title": "Nope",
                "system_name": "generic_percentile",
                "privacy_mode": "standard",
                "idempotency_key": "create-no-auth"
            }),
            vec![],
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn room_creation_list_and_safe_projection_work() {
        let app = router(test_config());
        let (owner, _cookies) = login(app.clone(), "owner@example.test").await;
        let created = create_test_room(app.clone(), &owner, "Friday Game", "create-room-1").await;

        assert_eq!(created.room.title, "Friday Game");
        assert_eq!(created.room.my_role, RoomRole::Owner);
        let raw: serde_json::Value = serde_json::to_value(&created).expect("room response json");
        assert!(raw["room"].get("kp_only_notes").is_none());
        assert!(raw["room"].get("private_kp_notes").is_none());

        let list_response =
            empty_request(app.clone(), "GET", "/api/rooms", auth_header(&owner)).await;
        assert_eq!(list_response.status(), StatusCode::OK);
        let list: ListRoomsResponse = read_json(list_response).await;
        assert_eq!(list.rooms.len(), 1);
        assert_eq!(list.rooms[0].id, created.room.id);

        let get_response = empty_request(
            app,
            "GET",
            &format!("/api/rooms/{}", created.room.id),
            auth_header(&owner),
        )
        .await;
        assert_eq!(get_response.status(), StatusCode::OK);
        let fetched: RoomResponse = read_json(get_response).await;
        assert_eq!(fetched, created);
    }

    #[tokio::test]
    async fn duplicate_room_create_idempotency_key_returns_original_room() {
        let app = router(test_config());
        let (owner, _cookies) = login(app.clone(), "owner@example.test").await;
        let first = create_test_room(app.clone(), &owner, "Same Room", "same-create-key").await;
        let second = create_test_room(app.clone(), &owner, "Same Room", "same-create-key").await;
        assert_eq!(first, second);

        let list_response = empty_request(app, "GET", "/api/rooms", auth_header(&owner)).await;
        let list: ListRoomsResponse = read_json(list_response).await;
        assert_eq!(list.rooms.len(), 1);
    }

    #[tokio::test]
    async fn owner_invites_member_and_invited_user_joins() {
        let app = router(test_config());
        let (owner, _owner_cookies) = login(app.clone(), "owner@example.test").await;
        let room = create_test_room(app.clone(), &owner, "Invite Room", "invite-room").await;
        let invite = invite_user(
            app.clone(),
            &owner,
            room.room.id,
            "player@example.test",
            "invite-player",
        )
        .await;
        assert_eq!(invite.role, RoomRole::Pl);
        assert!(!invite.token.is_empty());

        let (player, _player_cookies) = login(app.clone(), "player@example.test").await;
        let joined = accept_invite(app.clone(), &player, &invite.token, "accept-player").await;
        assert_eq!(joined.room.id, room.room.id);
        assert_eq!(joined.room.my_role, RoomRole::Pl);

        let get_response = empty_request(
            app.clone(),
            "GET",
            &format!("/api/rooms/{}", room.room.id),
            auth_header(&player),
        )
        .await;
        assert_eq!(get_response.status(), StatusCode::OK);
        let player_view: RoomResponse = read_json(get_response).await;
        assert_eq!(player_view.room.my_role, RoomRole::Pl);

        let members_response = empty_request(
            app,
            "GET",
            &format!("/api/rooms/{}/members", room.room.id),
            auth_header(&owner),
        )
        .await;
        assert_eq!(members_response.status(), StatusCode::OK);
        let members: ListRoomMembersResponse = read_json(members_response).await;
        assert_eq!(members.members.len(), 2);
        assert!(members
            .members
            .iter()
            .any(|member| member.role == RoomRole::Owner));
        assert!(members
            .members
            .iter()
            .any(|member| member.role == RoomRole::Pl));
    }

    #[tokio::test]
    async fn non_owner_member_cannot_invite() {
        let app = router(test_config());
        let (owner, _owner_cookies) = login(app.clone(), "owner@example.test").await;
        let room = create_test_room(app.clone(), &owner, "No Invite", "no-invite-room").await;
        let invite = invite_user(
            app.clone(),
            &owner,
            room.room.id,
            "player@example.test",
            "owner-invites-player",
        )
        .await;
        let (player, _player_cookies) = login(app.clone(), "player@example.test").await;
        accept_invite(app.clone(), &player, &invite.token, "player-accepts").await;

        let response = json_request(
            app,
            "POST",
            &format!("/api/rooms/{}/invitations", room.room.id),
            json!({
                "email": "third@example.test",
                "role": "pl",
                "idempotency_key": "player-tries-invite"
            }),
            auth_header(&player),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn third_party_and_cross_room_access_do_not_leak_private_rooms() {
        let app = router(test_config());
        let (owner_a, _cookies_a) = login(app.clone(), "a@example.test").await;
        let (owner_b, _cookies_b) = login(app.clone(), "b@example.test").await;
        let room_a = create_test_room(app.clone(), &owner_a, "Room A", "room-a").await;
        let room_b = create_test_room(app.clone(), &owner_b, "Room B", "room-b").await;

        let forbidden_room = empty_request(
            app.clone(),
            "GET",
            &format!("/api/rooms/{}", room_a.room.id),
            auth_header(&owner_b),
        )
        .await;
        assert_eq!(forbidden_room.status(), StatusCode::NOT_FOUND);

        let forbidden_members = empty_request(
            app.clone(),
            "GET",
            &format!("/api/rooms/{}/members", room_a.room.id),
            auth_header(&owner_b),
        )
        .await;
        assert_eq!(forbidden_members.status(), StatusCode::NOT_FOUND);

        let list_a = empty_request(app.clone(), "GET", "/api/rooms", auth_header(&owner_a)).await;
        let rooms_a: ListRoomsResponse = read_json(list_a).await;
        assert_eq!(rooms_a.rooms.len(), 1);
        assert_eq!(rooms_a.rooms[0].id, room_a.room.id);

        let list_b = empty_request(app, "GET", "/api/rooms", auth_header(&owner_b)).await;
        let rooms_b: ListRoomsResponse = read_json(list_b).await;
        assert_eq!(rooms_b.rooms.len(), 1);
        assert_eq!(rooms_b.rooms[0].id, room_b.room.id);
    }

    #[tokio::test]
    async fn http_vertical_flow_writes_required_audit_rows() {
        let store = Arc::new(InMemoryAuthStore::default());
        let app = router_with_auth_store(test_config(), store.clone());
        let (owner, _owner_cookies) = login(app.clone(), "audit-owner@example.test").await;
        let room = create_test_room(app.clone(), &owner, "Audit Room", "audit-room").await;
        let invite = invite_user(
            app.clone(),
            &owner,
            room.room.id,
            "audit-player@example.test",
            "audit-invite",
        )
        .await;
        let (player, _player_cookies) = login(app.clone(), "audit-player@example.test").await;
        accept_invite(app.clone(), &player, &invite.token, "audit-accept").await;
        let (outsider, _outsider_cookies) = login(app.clone(), "audit-outsider@example.test").await;

        let denied = empty_request(
            app,
            "GET",
            &format!("/api/rooms/{}", room.room.id),
            auth_header(&outsider),
        )
        .await;
        assert_eq!(denied.status(), StatusCode::NOT_FOUND);

        let logs = audit_logs(&store);
        for action in [
            "auth.login",
            "room.create",
            "room.invite.create",
            "room.invite.accept",
            "access.denied",
        ] {
            assert!(
                logs.iter().any(|log| log.action == action),
                "missing audit action {action}"
            );
        }
        assert!(logs
            .iter()
            .any(|log| log.action == "access.denied" && log.outcome == AuditOutcome::Failure));
    }

    #[tokio::test]
    async fn postgres_http_room_flow_uses_rls_context_and_writes_audit() {
        let Some((app, repo)) = postgres_router_with_rls_role().await else {
            return;
        };
        let suffix = Uuid::new_v4().simple();
        let owner_email = format!("owner-{suffix}@example.test");
        let player_email = format!("player-{suffix}@example.test");
        let outsider_email = format!("outsider-{suffix}@example.test");

        let (owner, _owner_cookies) = login(app.clone(), &owner_email).await;
        let room = create_test_room(
            app.clone(),
            &owner,
            "Postgres RLS Room",
            &format!("pg-create-{suffix}"),
        )
        .await;
        let invite = invite_user(
            app.clone(),
            &owner,
            room.room.id,
            &player_email,
            &format!("pg-invite-{suffix}"),
        )
        .await;
        let (player, _player_cookies) = login(app.clone(), &player_email).await;
        accept_invite(
            app.clone(),
            &player,
            &invite.token,
            &format!("pg-accept-{suffix}"),
        )
        .await;

        let members_response = empty_request(
            app.clone(),
            "GET",
            &format!("/api/rooms/{}/members", room.room.id),
            auth_header(&owner),
        )
        .await;
        assert_eq!(members_response.status(), StatusCode::OK);
        let members: ListRoomMembersResponse = read_json(members_response).await;
        assert_eq!(members.members.len(), 2);

        let (outsider, _outsider_cookies) = login(app.clone(), &outsider_email).await;
        let denied = empty_request(
            app,
            "GET",
            &format!("/api/rooms/{}", room.room.id),
            auth_header(&outsider),
        )
        .await;
        assert_eq!(denied.status(), StatusCode::NOT_FOUND);

        let audit_count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*)
            FROM audit_logs
            WHERE action = ANY($1)
              AND actor_id = ANY($2)
            "#,
        )
        .bind(vec![
            "auth.login",
            "room.create",
            "room.invite.create",
            "room.invite.accept",
            "access.denied",
        ])
        .bind(vec![owner.user.id, player.user.id, outsider.user.id])
        .fetch_one(repo.pool())
        .await
        .expect("audit rows should query");

        assert!(audit_count >= 7);
    }
}
