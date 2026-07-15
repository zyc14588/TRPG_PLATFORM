pub mod schema;

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, RwLock};

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use hmac::{Hmac, Mac};
use postgres::{Client, GenericClient, NoTls};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use trpg_shared_kernel::{
    Actor, ActorOrigin, ActorRole, AgentClass as KernelAgentClass, AuthorityContract,
    AuthorityContractDraft, AuthorityMode, AuthorityVersionSnapshotDraft, EntityId,
    WorkloadRole as KernelWorkloadRole,
};

type HmacSha256 = Hmac<Sha256>;

const SESSION_TOKEN_BYTES: usize = 32;
const SIGNING_KEY_BYTES: usize = 32;
const INTERNAL_TOKEN_VERSION: &str = "v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpAuthStatus {
    Unauthorized401,
    Forbidden403,
    Conflict409,
    Internal500,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityError {
    InvalidCredentials,
    AuthenticationRequired,
    SessionExpired,
    SessionRevoked,
    SessionNotFound,
    DuplicateLogin,
    AuthorityContractConflict,
    AuthorityContractRequired,
    MembershipRequired,
    MembershipDenied,
    CampaignScopeMismatch,
    InvalidInternalCredential,
    InternalCredentialExpired,
    InvalidSigningKey,
    InvalidIdentityData,
    PasswordHashFailure,
    PersistenceUnavailable,
}

impl IdentityError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::AuthenticationRequired => "AUTHENTICATION_REQUIRED",
            Self::SessionExpired => "SESSION_EXPIRED",
            Self::SessionRevoked => "SESSION_REVOKED",
            Self::SessionNotFound => "SESSION_NOT_FOUND",
            Self::DuplicateLogin => "DUPLICATE_LOGIN",
            Self::AuthorityContractConflict => "AUTHORITY_CONTRACT_VERSION_CONFLICT",
            Self::AuthorityContractRequired => "AUTHORITY_CONTRACT_REQUIRED",
            Self::MembershipRequired => "CAMPAIGN_MEMBERSHIP_REQUIRED",
            Self::MembershipDenied => "CAMPAIGN_MEMBERSHIP_DENIED",
            Self::CampaignScopeMismatch => "CAMPAIGN_SCOPE_MISMATCH",
            Self::InvalidInternalCredential => "INTERNAL_IDENTITY_INVALID",
            Self::InternalCredentialExpired => "INTERNAL_IDENTITY_EXPIRED",
            Self::InvalidSigningKey => "IDENTITY_SIGNING_KEY_INVALID",
            Self::InvalidIdentityData => "IDENTITY_DATA_INVALID",
            Self::PasswordHashFailure => "PASSWORD_HASH_FAILURE",
            Self::PersistenceUnavailable => "IDENTITY_PERSISTENCE_UNAVAILABLE",
        }
    }

    pub const fn http_status(&self) -> HttpAuthStatus {
        match self {
            Self::InvalidCredentials
            | Self::AuthenticationRequired
            | Self::SessionExpired
            | Self::SessionRevoked
            | Self::SessionNotFound
            | Self::InvalidInternalCredential
            | Self::InternalCredentialExpired => HttpAuthStatus::Unauthorized401,
            Self::AuthorityContractRequired
            | Self::MembershipRequired
            | Self::MembershipDenied
            | Self::CampaignScopeMismatch => HttpAuthStatus::Forbidden403,
            Self::DuplicateLogin | Self::AuthorityContractConflict => HttpAuthStatus::Conflict409,
            Self::InvalidSigningKey
            | Self::InvalidIdentityData
            | Self::PasswordHashFailure
            | Self::PersistenceUnavailable => HttpAuthStatus::Internal500,
        }
    }
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for IdentityError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobalRole {
    User,
    Moderator,
    ServerOwner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignRole {
    CampaignOwner,
    HumanKeeper,
    Player,
    Spectator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkloadRole {
    ApiServer,
    RealtimeServer,
    AgentWorker,
    WorkflowEngine,
    RulesEngine,
    AuditWriter,
}

impl WorkloadRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ApiServer => "api_server",
            Self::RealtimeServer => "realtime_server",
            Self::AgentWorker => "agent_worker",
            Self::WorkflowEngine => "workflow_engine",
            Self::RulesEngine => "rules_engine",
            Self::AuditWriter => "audit_writer",
        }
    }

    fn parse(value: &str) -> Result<Self, IdentityError> {
        match value {
            "api_server" => Ok(Self::ApiServer),
            "realtime_server" => Ok(Self::RealtimeServer),
            "agent_worker" => Ok(Self::AgentWorker),
            "workflow_engine" => Ok(Self::WorkflowEngine),
            "rules_engine" => Ok(Self::RulesEngine),
            "audit_writer" => Ok(Self::AuditWriter),
            _ => Err(IdentityError::InvalidInternalCredential),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentClass {
    AiKeeperOrchestrator,
    KeeperCopilot,
    AtmosphereWriter,
    MemoryCurator,
}

impl AgentClass {
    const fn as_str(self) -> &'static str {
        match self {
            Self::AiKeeperOrchestrator => "ai_keeper_orchestrator",
            Self::KeeperCopilot => "keeper_copilot",
            Self::AtmosphereWriter => "atmosphere_writer",
            Self::MemoryCurator => "memory_curator",
        }
    }

    fn parse(value: &str) -> Result<Self, IdentityError> {
        match value {
            "ai_keeper_orchestrator" => Ok(Self::AiKeeperOrchestrator),
            "keeper_copilot" => Ok(Self::KeeperCopilot),
            "atmosphere_writer" => Ok(Self::AtmosphereWriter),
            "memory_curator" => Ok(Self::MemoryCurator),
            _ => Err(IdentityError::InvalidInternalCredential),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrincipalKind {
    UserSession {
        session_id: EntityId,
        global_role: GlobalRole,
    },
    Workload {
        role: WorkloadRole,
    },
    AgentRun {
        run_id: EntityId,
        class: AgentClass,
        campaign_id: EntityId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticationContext {
    subject_id: EntityId,
    kind: PrincipalKind,
    authenticated_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    issuer_fingerprint: [u8; 32],
}

impl AuthenticationContext {
    pub fn subject_id(&self) -> &EntityId {
        &self.subject_id
    }

    pub fn kind(&self) -> &PrincipalKind {
        &self.kind
    }

    pub const fn authenticated_at_unix_ms(&self) -> u64 {
        self.authenticated_at_unix_ms
    }

    pub const fn expires_at_unix_ms(&self) -> u64 {
        self.expires_at_unix_ms
    }

    pub fn require_campaign(&self, campaign_id: &EntityId) -> Result<(), IdentityError> {
        if let PrincipalKind::AgentRun {
            campaign_id: bound_campaign,
            ..
        } = &self.kind
        {
            if bound_campaign != campaign_id {
                return Err(IdentityError::CampaignScopeMismatch);
            }
        }
        Ok(())
    }

    fn to_command_actor(
        &self,
        membership: Option<&CampaignMembership>,
    ) -> Result<Actor, IdentityError> {
        match &self.kind {
            PrincipalKind::UserSession {
                session_id,
                global_role,
            } => {
                if membership.is_some_and(|membership| membership.user_id != self.subject_id) {
                    return Err(IdentityError::MembershipDenied);
                }
                let role = match (global_role, membership.map(CampaignMembership::role)) {
                    (GlobalRole::ServerOwner, _) => ActorRole::ServerOwner,
                    (GlobalRole::Moderator, _) => ActorRole::Moderator,
                    (_, Some(CampaignRole::CampaignOwner)) => ActorRole::CampaignOwner,
                    (_, Some(CampaignRole::HumanKeeper)) => ActorRole::HumanKeeper,
                    (_, Some(CampaignRole::Player)) => ActorRole::Investigator,
                    (_, Some(CampaignRole::Spectator)) => ActorRole::Spectator,
                    (_, None) => return Err(IdentityError::MembershipRequired),
                };
                Actor::authenticated_user(self.subject_id.as_str(), role, session_id.as_str())
            }
            PrincipalKind::Workload { role } => Actor::verified_workload(
                self.subject_id.as_str(),
                match role {
                    WorkloadRole::ApiServer => KernelWorkloadRole::ApiServer,
                    WorkloadRole::RealtimeServer => KernelWorkloadRole::RealtimeServer,
                    WorkloadRole::AgentWorker => KernelWorkloadRole::AgentWorker,
                    WorkloadRole::WorkflowEngine => KernelWorkloadRole::WorkflowEngine,
                    WorkloadRole::RulesEngine => KernelWorkloadRole::RulesEngine,
                    WorkloadRole::AuditWriter => KernelWorkloadRole::AuditWriter,
                },
            ),
            PrincipalKind::AgentRun {
                run_id,
                class,
                campaign_id,
            } => Actor::verified_agent_run(
                self.subject_id.as_str(),
                run_id.as_str(),
                match class {
                    AgentClass::AiKeeperOrchestrator => KernelAgentClass::AiKeeperOrchestrator,
                    AgentClass::KeeperCopilot => KernelAgentClass::KeeperCopilot,
                    AgentClass::AtmosphereWriter => KernelAgentClass::AtmosphereWriter,
                    AgentClass::MemoryCurator => KernelAgentClass::MemoryCurator,
                },
                campaign_id.as_str(),
            ),
        }
        .map_err(|_| IdentityError::InvalidInternalCredential)
    }
}

/// Read-only trust anchor distributed to services that consume authenticated
/// identities. The fingerprint is not a signing secret; contexts remain
/// constructible only by `IdentityService` and are accepted only from the
/// configured issuer.
#[derive(Clone)]
pub struct IdentityVerifier {
    issuer_fingerprint: [u8; 32],
    state: Arc<RwLock<VerificationState>>,
}

impl fmt::Debug for IdentityVerifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IdentityVerifier")
            .field("issuer_fingerprint", &hex_encode(&self.issuer_fingerprint))
            .field("state", &"[LIVE IDENTITY STATE]")
            .finish()
    }
}

impl IdentityVerifier {
    pub fn verify(
        &self,
        authentication: &AuthenticationContext,
        now_unix_ms: u64,
    ) -> Result<(), IdentityError> {
        if authentication.issuer_fingerprint != self.issuer_fingerprint {
            return Err(IdentityError::InvalidInternalCredential);
        }
        if authentication.authenticated_at_unix_ms > now_unix_ms
            || authentication.expires_at_unix_ms <= now_unix_ms
        {
            return Err(IdentityError::InternalCredentialExpired);
        }
        if let PrincipalKind::UserSession { session_id, .. } = authentication.kind() {
            let state = self
                .state
                .read()
                .map_err(|_| IdentityError::PersistenceUnavailable)?;
            let session = state
                .sessions_by_id
                .get(session_id)
                .ok_or(IdentityError::SessionNotFound)?;
            if session.user_id != authentication.subject_id
                || session.issued_at_unix_ms != authentication.authenticated_at_unix_ms
                || session.expires_at_unix_ms != authentication.expires_at_unix_ms
            {
                return Err(IdentityError::InvalidInternalCredential);
            }
            if session.revoked {
                return Err(IdentityError::SessionRevoked);
            }
        }
        Ok(())
    }

    pub fn authority_contract(
        &self,
        campaign_id: &EntityId,
    ) -> Result<AuthorityContract, IdentityError> {
        self.state
            .read()
            .map_err(|_| IdentityError::PersistenceUnavailable)?
            .authorities
            .get(campaign_id)
            .cloned()
            .ok_or(IdentityError::AuthorityContractRequired)
    }

    pub fn verify_actor(
        &self,
        authentication: &AuthenticationContext,
        actor: &Actor,
        campaign_id: &EntityId,
        now_unix_ms: u64,
    ) -> Result<(), IdentityError> {
        self.verify(authentication, now_unix_ms)?;
        authentication.require_campaign(campaign_id)?;
        if actor.id() != authentication.subject_id() {
            return Err(IdentityError::InvalidInternalCredential);
        }
        match (authentication.kind(), actor.origin()) {
            (
                PrincipalKind::UserSession {
                    session_id,
                    global_role,
                },
                ActorOrigin::UserSession {
                    session_id: actor_session_id,
                },
            ) if session_id == actor_session_id => {
                let expected_role = match global_role {
                    GlobalRole::ServerOwner => ActorRole::ServerOwner,
                    GlobalRole::Moderator => ActorRole::Moderator,
                    GlobalRole::User => {
                        let state = self
                            .state
                            .read()
                            .map_err(|_| IdentityError::PersistenceUnavailable)?;
                        match state
                            .memberships
                            .get(&(campaign_id.clone(), authentication.subject_id().clone()))
                            .ok_or(IdentityError::MembershipRequired)?
                        {
                            CampaignRole::CampaignOwner => ActorRole::CampaignOwner,
                            CampaignRole::HumanKeeper => ActorRole::HumanKeeper,
                            CampaignRole::Player => ActorRole::Investigator,
                            CampaignRole::Spectator => ActorRole::Spectator,
                        }
                    }
                };
                if actor.role() != &expected_role {
                    return Err(IdentityError::InvalidInternalCredential);
                }
            }
            (PrincipalKind::Workload { role }, ActorOrigin::Workload { role: actor_role }) => {
                let (expected_origin, expected_role) = match role {
                    WorkloadRole::ApiServer => (KernelWorkloadRole::ApiServer, ActorRole::System),
                    WorkloadRole::RealtimeServer => {
                        (KernelWorkloadRole::RealtimeServer, ActorRole::System)
                    }
                    WorkloadRole::AgentWorker => {
                        (KernelWorkloadRole::AgentWorker, ActorRole::System)
                    }
                    WorkloadRole::WorkflowEngine => {
                        (KernelWorkloadRole::WorkflowEngine, ActorRole::Workflow)
                    }
                    WorkloadRole::RulesEngine => {
                        (KernelWorkloadRole::RulesEngine, ActorRole::RulesEngine)
                    }
                    WorkloadRole::AuditWriter => {
                        (KernelWorkloadRole::AuditWriter, ActorRole::System)
                    }
                };
                if actor_role != &expected_origin || actor.role() != &expected_role {
                    return Err(IdentityError::InvalidInternalCredential);
                }
            }
            (
                PrincipalKind::AgentRun {
                    run_id,
                    class,
                    campaign_id: bound_campaign,
                },
                ActorOrigin::AgentRun {
                    run_id: actor_run_id,
                    class: actor_class,
                    campaign_id: actor_campaign,
                },
            ) => {
                let (expected_class, expected_role) = match class {
                    AgentClass::AiKeeperOrchestrator => {
                        (KernelAgentClass::AiKeeperOrchestrator, ActorRole::AiKeeper)
                    }
                    AgentClass::KeeperCopilot => {
                        (KernelAgentClass::KeeperCopilot, ActorRole::Investigator)
                    }
                    AgentClass::AtmosphereWriter => {
                        (KernelAgentClass::AtmosphereWriter, ActorRole::Investigator)
                    }
                    AgentClass::MemoryCurator => {
                        (KernelAgentClass::MemoryCurator, ActorRole::Investigator)
                    }
                };
                if run_id != actor_run_id
                    || bound_campaign != campaign_id
                    || actor_campaign != campaign_id
                    || actor_class != &expected_class
                    || actor.role() != &expected_role
                {
                    return Err(IdentityError::InvalidInternalCredential);
                }
            }
            _ => return Err(IdentityError::InvalidInternalCredential),
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SessionToken(String);

impl SessionToken {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionToken([REDACTED])")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginSession {
    pub token: SessionToken,
    pub expires_at_unix_ms: u64,
}

#[derive(Clone, Debug)]
struct UserRecord {
    user_id: EntityId,
    password_hash: String,
    global_role: GlobalRole,
}

#[derive(Clone, Debug)]
struct SessionRecord {
    session_id: EntityId,
    user_id: EntityId,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    revoked: bool,
}

#[derive(Clone, Debug)]
struct SessionVerificationRecord {
    user_id: EntityId,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    revoked: bool,
}

#[derive(Debug, Default)]
struct VerificationState {
    sessions_by_id: HashMap<EntityId, SessionVerificationRecord>,
    memberships: HashMap<(EntityId, EntityId), CampaignRole>,
    authorities: HashMap<EntityId, AuthorityContract>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignMembership {
    campaign_id: EntityId,
    user_id: EntityId,
    role: CampaignRole,
}

impl CampaignMembership {
    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn user_id(&self) -> &EntityId {
        &self.user_id
    }

    pub const fn role(&self) -> CampaignRole {
        self.role
    }
}

pub struct IdentityService {
    users_by_login: HashMap<String, UserRecord>,
    users_by_id: HashMap<EntityId, UserRecord>,
    sessions_by_hash: HashMap<[u8; 32], SessionRecord>,
    memberships: HashMap<(EntityId, EntityId), CampaignMembership>,
    authorities: HashMap<EntityId, AuthorityContract>,
    database: Option<Client>,
    verification_state: Arc<RwLock<VerificationState>>,
    signing_key: [u8; SIGNING_KEY_BYTES],
    session_ttl_ms: u64,
}

impl IdentityService {
    pub fn new(signing_key: &[u8], session_ttl_ms: u64) -> Result<Self, IdentityError> {
        if signing_key.len() != SIGNING_KEY_BYTES || session_ttl_ms == 0 {
            return Err(IdentityError::InvalidSigningKey);
        }
        let mut key = [0_u8; SIGNING_KEY_BYTES];
        key.copy_from_slice(signing_key);
        Ok(Self {
            users_by_login: HashMap::new(),
            users_by_id: HashMap::new(),
            sessions_by_hash: HashMap::new(),
            memberships: HashMap::new(),
            authorities: HashMap::new(),
            database: None,
            verification_state: Arc::new(RwLock::new(VerificationState::default())),
            signing_key: key,
            session_ttl_ms,
        })
    }

    pub fn from_postgres(
        database_url: &str,
        signing_key: &[u8],
        session_ttl_ms: u64,
    ) -> Result<Self, IdentityError> {
        if database_url.trim().is_empty() {
            return Err(IdentityError::PersistenceUnavailable);
        }
        let mut client = Client::connect(database_url, NoTls)
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        client
            .batch_execute(schema::IDENTITY_AUTHORIZATION_MIGRATION_SQL)
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        client
            .batch_execute(schema::IDENTITY_AUTHORIZATION_HARDENING_MIGRATION_SQL)
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        let mut service = Self::new(signing_key, session_ttl_ms)?;
        service.database = Some(client);
        service.reload_from_database()?;
        Ok(service)
    }

    pub const fn is_persistent(&self) -> bool {
        self.database.is_some()
    }

    pub fn check_readiness(&mut self) -> Result<(), IdentityError> {
        let database = self
            .database
            .as_mut()
            .ok_or(IdentityError::PersistenceUnavailable)?;
        database
            .check_connection()
            .map_err(|_| IdentityError::PersistenceUnavailable)
    }

    pub fn verifier(&self) -> IdentityVerifier {
        IdentityVerifier {
            issuer_fingerprint: Sha256::digest(self.signing_key).into(),
            state: Arc::clone(&self.verification_state),
        }
    }

    fn publish_verification_state(&self) -> Result<(), IdentityError> {
        let sessions_by_id = self
            .sessions_by_hash
            .values()
            .map(|session| {
                (
                    session.session_id.clone(),
                    SessionVerificationRecord {
                        user_id: session.user_id.clone(),
                        issued_at_unix_ms: session.issued_at_unix_ms,
                        expires_at_unix_ms: session.expires_at_unix_ms,
                        revoked: session.revoked,
                    },
                )
            })
            .collect();
        let mut state = self
            .verification_state
            .write()
            .map_err(|_| IdentityError::PersistenceUnavailable)?;
        state.sessions_by_id = sessions_by_id;
        state.memberships = self
            .memberships
            .iter()
            .map(|(key, membership)| (key.clone(), membership.role))
            .collect();
        state.authorities = self.authorities.clone();
        Ok(())
    }

    fn reload_from_database(&mut self) -> Result<(), IdentityError> {
        let database = self
            .database
            .as_mut()
            .ok_or(IdentityError::PersistenceUnavailable)?;

        let mut users_by_login = HashMap::new();
        let mut users_by_id = HashMap::new();
        for row in database
            .query(
                "SELECT user_id, login_normalized, password_hash, global_role \
                   FROM users WHERE disabled_at IS NULL",
                &[],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
        {
            let record = UserRecord {
                user_id: EntityId::new(row.get::<_, String>(0))
                    .map_err(|_| IdentityError::InvalidIdentityData)?,
                password_hash: row.get(2),
                global_role: parse_global_role(row.get::<_, String>(3).as_str())?,
            };
            users_by_login.insert(row.get(1), record.clone());
            users_by_id.insert(record.user_id.clone(), record);
        }

        let mut sessions_by_hash = HashMap::new();
        for row in database
            .query(
                "SELECT session_id, user_id, token_hash, \
                        (extract(epoch FROM issued_at) * 1000)::bigint, \
                        (extract(epoch FROM expires_at) * 1000)::bigint, \
                        revoked_at IS NOT NULL \
                   FROM sessions",
                &[],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
        {
            let token_hash = row.get::<_, Vec<u8>>(2);
            let token_hash: [u8; 32] = token_hash
                .try_into()
                .map_err(|_| IdentityError::InvalidIdentityData)?;
            sessions_by_hash.insert(
                token_hash,
                SessionRecord {
                    session_id: EntityId::new(row.get::<_, String>(0))
                        .map_err(|_| IdentityError::InvalidIdentityData)?,
                    user_id: EntityId::new(row.get::<_, String>(1))
                        .map_err(|_| IdentityError::InvalidIdentityData)?,
                    issued_at_unix_ms: timestamp_from_i64(row.get(3))?,
                    expires_at_unix_ms: timestamp_from_i64(row.get(4))?,
                    revoked: row.get(5),
                },
            );
        }

        let mut memberships = HashMap::new();
        for row in database
            .query(
                "SELECT campaign_id, user_id, role \
                   FROM campaign_memberships WHERE revoked_at IS NULL",
                &[],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
        {
            let membership = CampaignMembership {
                campaign_id: EntityId::new(row.get::<_, String>(0))
                    .map_err(|_| IdentityError::InvalidIdentityData)?,
                user_id: EntityId::new(row.get::<_, String>(1))
                    .map_err(|_| IdentityError::InvalidIdentityData)?,
                role: parse_campaign_role(row.get::<_, String>(2).as_str())?,
            };
            memberships.insert(
                (membership.campaign_id.clone(), membership.user_id.clone()),
                membership,
            );
        }

        let mut authorities = HashMap::new();
        for row in database
            .query(
                "SELECT contract_id, campaign_id, authority_mode, authority_owner, \
                        contract_version, ruleset_version, house_rules_version, \
                        scenario_version, prompt_version, agent_pack_version, \
                        tool_schema_version, safety_profile_version, ai_provider_snapshot, \
                        model_route_snapshot, character_sheet_template_version, \
                        (extract(epoch FROM created_at) * 1000)::bigint \
                   FROM authority_contracts",
                &[],
            )
            .map_err(|_| IdentityError::PersistenceUnavailable)?
        {
            let version = u64::try_from(row.get::<_, i64>(4))
                .map_err(|_| IdentityError::InvalidIdentityData)?;
            let contract = AuthorityContract::new_locked(AuthorityContractDraft {
                contract_id: row.get(0),
                campaign_id: row.get(1),
                mode: parse_authority_mode(row.get::<_, String>(2).as_str())?,
                authority_owner: row.get(3),
                version,
                snapshot: AuthorityVersionSnapshotDraft {
                    ruleset_version: row.get(5),
                    house_rules_version: row.get(6),
                    scenario_version: row.get(7),
                    prompt_version: row.get(8),
                    agent_pack_version: row.get(9),
                    tool_schema_version: row.get(10),
                    safety_profile_version: row.get(11),
                    ai_provider_snapshot: row.get(12),
                    model_route_snapshot: row.get(13),
                    character_sheet_template_version: row.get(14),
                },
                created_at_unix_ms: timestamp_from_i64(row.get(15))?,
            })
            .map_err(|_| IdentityError::InvalidIdentityData)?;
            authorities.insert(contract.campaign_id().clone(), contract);
        }

        self.users_by_login = users_by_login;
        self.users_by_id = users_by_id;
        self.sessions_by_hash = sessions_by_hash;
        self.memberships = memberships;
        self.authorities = authorities;
        self.publish_verification_state()
    }

    fn sync_if_persistent(&mut self) -> Result<(), IdentityError> {
        if self.database.is_some() {
            self.reload_from_database()?;
        }
        Ok(())
    }

    pub fn create_user(
        &mut self,
        user_id: impl Into<String>,
        login: &str,
        password: &str,
        global_role: GlobalRole,
    ) -> Result<(), IdentityError> {
        self.sync_if_persistent()?;
        let normalized_login = normalize_login(login)?;
        if self.users_by_login.contains_key(&normalized_login) {
            return Err(IdentityError::DuplicateLogin);
        }
        validate_password(password)?;
        let user_id = EntityId::new(user_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| IdentityError::PasswordHashFailure)?
            .to_string();
        let record = UserRecord {
            user_id: user_id.clone(),
            password_hash,
            global_role,
        };
        if let Some(database) = self.database.as_mut() {
            database
                .execute(
                    "INSERT INTO users \
                        (user_id, login_normalized, password_hash, global_role) \
                     VALUES ($1, $2, $3, $4)",
                    &[
                        &record.user_id.as_str(),
                        &normalized_login,
                        &record.password_hash,
                        &global_role_name(record.global_role),
                    ],
                )
                .map_err(map_postgres_error)?;
        }
        self.users_by_login.insert(normalized_login, record.clone());
        self.users_by_id.insert(user_id, record);
        Ok(())
    }

    pub fn login(
        &mut self,
        login: &str,
        password: &str,
        now_unix_ms: u64,
    ) -> Result<LoginSession, IdentityError> {
        self.sync_if_persistent()?;
        let normalized_login =
            normalize_login(login).map_err(|_| IdentityError::InvalidCredentials)?;
        let user = self
            .users_by_login
            .get(&normalized_login)
            .ok_or(IdentityError::InvalidCredentials)?;
        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|_| IdentityError::PasswordHashFailure)?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| IdentityError::InvalidCredentials)?;

        self.issue_session(user.user_id.clone(), now_unix_ms)
    }

    pub fn authenticate_session(
        &mut self,
        token: Option<&str>,
        now_unix_ms: u64,
    ) -> Result<AuthenticationContext, IdentityError> {
        self.sync_if_persistent()?;
        let token = token.ok_or(IdentityError::AuthenticationRequired)?;
        let token_hash = hash_token(token);
        let session = self
            .sessions_by_hash
            .get(&token_hash)
            .ok_or(IdentityError::SessionNotFound)?;
        if session.revoked {
            return Err(IdentityError::SessionRevoked);
        }
        if now_unix_ms >= session.expires_at_unix_ms {
            return Err(IdentityError::SessionExpired);
        }
        let user = self
            .users_by_id
            .get(&session.user_id)
            .ok_or(IdentityError::InvalidIdentityData)?;
        Ok(AuthenticationContext {
            subject_id: user.user_id.clone(),
            kind: PrincipalKind::UserSession {
                session_id: session.session_id.clone(),
                global_role: user.global_role,
            },
            authenticated_at_unix_ms: session.issued_at_unix_ms,
            expires_at_unix_ms: session.expires_at_unix_ms,
            issuer_fingerprint: self.verifier().issuer_fingerprint,
        })
    }

    pub fn refresh_session(
        &mut self,
        token: &str,
        now_unix_ms: u64,
    ) -> Result<LoginSession, IdentityError> {
        self.sync_if_persistent()?;
        let token_hash = hash_token(token);
        let session = self
            .sessions_by_hash
            .get(&token_hash)
            .ok_or(IdentityError::SessionNotFound)?;
        if session.revoked {
            return Err(IdentityError::SessionRevoked);
        }
        if now_unix_ms >= session.expires_at_unix_ms {
            return Err(IdentityError::SessionExpired);
        }
        let user_id = session.user_id.clone();
        let rotated_from_session_id = session.session_id.clone();
        let (replacement_hash, replacement_record, replacement) =
            self.generate_session(user_id, now_unix_ms)?;
        if let Some(database) = self.database.as_mut() {
            let mut transaction = database
                .transaction()
                .map_err(|_| IdentityError::PersistenceUnavailable)?;
            let updated = transaction
                .execute(
                    "UPDATE sessions SET revoked_at = now() \
                     WHERE token_hash = $1 AND revoked_at IS NULL \
                       AND expires_at > to_timestamp($2::bigint / 1000.0)",
                    &[
                        &&token_hash[..],
                        &i64::try_from(now_unix_ms)
                            .map_err(|_| IdentityError::InvalidIdentityData)?,
                    ],
                )
                .map_err(|_| IdentityError::PersistenceUnavailable)?;
            if updated != 1 {
                return Err(IdentityError::SessionRevoked);
            }
            persist_session(
                &mut transaction,
                replacement_hash,
                &replacement_record,
                Some(&rotated_from_session_id),
            )?;
            transaction
                .commit()
                .map_err(|_| IdentityError::PersistenceUnavailable)?;
        }
        self.sessions_by_hash
            .get_mut(&token_hash)
            .ok_or(IdentityError::SessionNotFound)?
            .revoked = true;
        self.sessions_by_hash
            .insert(replacement_hash, replacement_record);
        self.publish_verification_state()?;
        Ok(replacement)
    }

    pub fn logout(&mut self, token: &str) -> Result<(), IdentityError> {
        self.sync_if_persistent()?;
        let token_hash = hash_token(token);
        let session = self
            .sessions_by_hash
            .get(&token_hash)
            .ok_or(IdentityError::SessionNotFound)?;
        if session.revoked {
            return Err(IdentityError::SessionRevoked);
        }
        if let Some(database) = self.database.as_mut() {
            let updated = database
                .execute(
                    "UPDATE sessions SET revoked_at = now() \
                     WHERE token_hash = $1 AND revoked_at IS NULL",
                    &[&&token_hash[..]],
                )
                .map_err(|_| IdentityError::PersistenceUnavailable)?;
            if updated != 1 {
                return Err(IdentityError::SessionRevoked);
            }
        }
        self.sessions_by_hash
            .get_mut(&token_hash)
            .ok_or(IdentityError::SessionNotFound)?
            .revoked = true;
        self.publish_verification_state()
    }

    pub fn grant_membership(
        &mut self,
        actor: &AuthenticationContext,
        campaign_id: impl Into<String>,
        user_id: impl Into<String>,
        role: CampaignRole,
        now_unix_ms: u64,
    ) -> Result<CampaignMembership, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        let campaign_id =
            EntityId::new(campaign_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let user_id = EntityId::new(user_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        if !self.users_by_id.contains_key(&user_id) {
            return Err(IdentityError::InvalidIdentityData);
        }
        let server_owner = matches!(
            actor.kind,
            PrincipalKind::UserSession {
                global_role: GlobalRole::ServerOwner,
                ..
            }
        );
        let campaign_owner = self
            .memberships
            .get(&(campaign_id.clone(), actor.subject_id.clone()))
            .is_some_and(|membership| membership.role == CampaignRole::CampaignOwner);
        if !server_owner && !campaign_owner {
            return Err(IdentityError::MembershipDenied);
        }
        if !server_owner
            && matches!(
                role,
                CampaignRole::CampaignOwner | CampaignRole::HumanKeeper
            )
        {
            return Err(IdentityError::MembershipDenied);
        }
        if role == CampaignRole::HumanKeeper
            && self.memberships.values().any(|membership| {
                membership.campaign_id == campaign_id
                    && membership.user_id != user_id
                    && membership.role == CampaignRole::HumanKeeper
            })
        {
            return Err(IdentityError::MembershipDenied);
        }
        if let Some(contract) = self.authorities.get(&campaign_id) {
            if (role == CampaignRole::HumanKeeper
                && (contract.mode() != &AuthorityMode::HumanKp
                    || contract.authority_owner() != &user_id))
                || (contract.mode() == &AuthorityMode::HumanKp
                    && contract.authority_owner() == &user_id
                    && role != CampaignRole::HumanKeeper)
            {
                return Err(IdentityError::MembershipDenied);
            }
        }
        let membership = CampaignMembership {
            campaign_id: campaign_id.clone(),
            user_id: user_id.clone(),
            role,
        };
        if let Some(database) = self.database.as_mut() {
            database
                .execute(
                    "INSERT INTO campaign_memberships \
                        (campaign_id, user_id, role, granted_by, revoked_at) \
                     VALUES ($1, $2, $3, $4, NULL) \
                     ON CONFLICT (campaign_id, user_id) DO UPDATE SET \
                        role = EXCLUDED.role, granted_by = EXCLUDED.granted_by, \
                        granted_at = now(), revoked_at = NULL",
                    &[
                        &membership.campaign_id.as_str(),
                        &membership.user_id.as_str(),
                        &campaign_role_name(membership.role),
                        &actor.subject_id.as_str(),
                    ],
                )
                .map_err(map_postgres_error)?;
        }
        self.memberships
            .insert((campaign_id, user_id), membership.clone());
        Ok(membership)
    }

    pub fn require_membership(
        &mut self,
        actor: &AuthenticationContext,
        campaign_id: &EntityId,
        allowed: &[CampaignRole],
        now_unix_ms: u64,
    ) -> Result<CampaignMembership, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        actor.require_campaign(campaign_id)?;
        let membership = self
            .memberships
            .get(&(campaign_id.clone(), actor.subject_id.clone()))
            .ok_or(IdentityError::MembershipRequired)?;
        if !allowed.contains(&membership.role) {
            return Err(IdentityError::MembershipDenied);
        }
        Ok(membership.clone())
    }

    pub fn membership_for(
        &mut self,
        actor: &AuthenticationContext,
        campaign_id: &EntityId,
        now_unix_ms: u64,
    ) -> Result<Option<CampaignMembership>, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        actor.require_campaign(campaign_id)?;
        Ok(self
            .memberships
            .get(&(campaign_id.clone(), actor.subject_id.clone()))
            .cloned())
    }

    pub fn require_membership_manager(
        &mut self,
        actor: &AuthenticationContext,
        campaign_id: &EntityId,
        now_unix_ms: u64,
    ) -> Result<Option<CampaignMembership>, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        if matches!(
            actor.kind,
            PrincipalKind::UserSession {
                global_role: GlobalRole::ServerOwner,
                ..
            }
        ) {
            return Ok(None);
        }
        let membership = self
            .memberships
            .get(&(campaign_id.clone(), actor.subject_id.clone()))
            .filter(|membership| membership.role == CampaignRole::CampaignOwner)
            .cloned()
            .ok_or(IdentityError::MembershipDenied)?;
        Ok(Some(membership))
    }

    pub fn command_actor(
        &mut self,
        authentication: &AuthenticationContext,
        campaign_id: &EntityId,
        now_unix_ms: u64,
    ) -> Result<Actor, IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(authentication, now_unix_ms)?;
        authentication.require_campaign(campaign_id)?;
        let membership = match authentication.kind() {
            PrincipalKind::UserSession {
                global_role: GlobalRole::User,
                ..
            } => Some(
                self.memberships
                    .get(&(campaign_id.clone(), authentication.subject_id.clone()))
                    .ok_or(IdentityError::MembershipRequired)?,
            ),
            PrincipalKind::UserSession { .. }
            | PrincipalKind::Workload { .. }
            | PrincipalKind::AgentRun { .. } => None,
        };
        authentication.to_command_actor(membership)
    }

    pub fn register_authority_contract(
        &mut self,
        actor: &AuthenticationContext,
        contract: AuthorityContract,
        now_unix_ms: u64,
    ) -> Result<(), IdentityError> {
        self.sync_if_persistent()?;
        self.verifier().verify(actor, now_unix_ms)?;
        let server_owner = matches!(
            actor.kind,
            PrincipalKind::UserSession {
                global_role: GlobalRole::ServerOwner,
                ..
            }
        );
        let campaign_owner = self
            .memberships
            .get(&(contract.campaign_id().clone(), actor.subject_id.clone()))
            .is_some_and(|membership| membership.role == CampaignRole::CampaignOwner);
        if !server_owner && !campaign_owner {
            return Err(IdentityError::MembershipDenied);
        }
        if self.authorities.contains_key(contract.campaign_id()) {
            return Err(IdentityError::AuthorityContractConflict);
        }
        if contract.mode() == &AuthorityMode::HumanKp {
            let owner_membership = self.memberships.get(&(
                contract.campaign_id().clone(),
                contract.authority_owner().clone(),
            ));
            if !owner_membership
                .is_some_and(|membership| membership.role == CampaignRole::HumanKeeper)
            {
                return Err(IdentityError::MembershipDenied);
            }
            if self.memberships.values().any(|membership| {
                membership.campaign_id == *contract.campaign_id()
                    && membership.user_id != *contract.authority_owner()
                    && membership.role == CampaignRole::HumanKeeper
            }) {
                return Err(IdentityError::MembershipDenied);
            }
        } else if self.memberships.values().any(|membership| {
            membership.campaign_id == *contract.campaign_id()
                && membership.role == CampaignRole::HumanKeeper
        }) {
            return Err(IdentityError::MembershipDenied);
        }

        if let Some(database) = self.database.as_mut() {
            let version = i64::try_from(contract.version())
                .map_err(|_| IdentityError::InvalidIdentityData)?;
            let created_at = i64::try_from(contract.created_at_unix_ms())
                .map_err(|_| IdentityError::InvalidIdentityData)?;
            let snapshot = contract.snapshot();
            database
                .execute(
                    "INSERT INTO authority_contracts (\
                        contract_id, campaign_id, authority_mode, authority_owner, \
                        contract_version, ruleset_version, house_rules_version, \
                        scenario_version, prompt_version, agent_pack_version, \
                        tool_schema_version, safety_profile_version, ai_provider_snapshot, \
                        model_route_snapshot, character_sheet_template_version, created_at\
                     ) VALUES (\
                        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, \
                        to_timestamp($16::bigint / 1000.0)\
                     )",
                    &[
                        &contract.contract_id().as_str(),
                        &contract.campaign_id().as_str(),
                        &authority_mode_name(contract.mode()),
                        &contract.authority_owner().as_str(),
                        &version,
                        &snapshot.ruleset_version().as_str(),
                        &snapshot.house_rules_version().as_str(),
                        &snapshot.scenario_version().as_str(),
                        &snapshot.prompt_version().as_str(),
                        &snapshot.agent_pack_version().as_str(),
                        &snapshot.tool_schema_version().as_str(),
                        &snapshot.safety_profile_version().as_str(),
                        &snapshot.ai_provider_snapshot().as_str(),
                        &snapshot.model_route_snapshot().as_str(),
                        &snapshot.character_sheet_template_version().as_str(),
                        &created_at,
                    ],
                )
                .map_err(map_postgres_error)?;
        }
        self.authorities
            .insert(contract.campaign_id().clone(), contract);
        self.publish_verification_state()
    }

    pub fn authority_contract(
        &mut self,
        campaign_id: &EntityId,
    ) -> Result<Option<AuthorityContract>, IdentityError> {
        self.sync_if_persistent()?;
        Ok(self.authorities.get(campaign_id).cloned())
    }

    pub fn issue_workload_credential(
        &self,
        workload_id: &str,
        role: WorkloadRole,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<String, IdentityError> {
        let workload_id =
            EntityId::new(workload_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        if expires_at_unix_ms <= issued_at_unix_ms {
            return Err(IdentityError::InvalidIdentityData);
        }
        let claims = format!(
            "{}|workload|{}|{}|{}|{}",
            INTERNAL_TOKEN_VERSION,
            workload_id,
            role.as_str(),
            issued_at_unix_ms,
            expires_at_unix_ms
        );
        Ok(self.sign_claims(&claims))
    }

    pub fn authenticate_workload(
        &self,
        credential: &str,
        now_unix_ms: u64,
    ) -> Result<AuthenticationContext, IdentityError> {
        let claims = self.verify_signed_credential(credential)?;
        let fields = claims.split('|').collect::<Vec<_>>();
        if fields.len() != 6 || fields[0] != INTERNAL_TOKEN_VERSION || fields[1] != "workload" {
            return Err(IdentityError::InvalidInternalCredential);
        }
        let subject_id =
            EntityId::new(fields[2]).map_err(|_| IdentityError::InvalidInternalCredential)?;
        let role = WorkloadRole::parse(fields[3])?;
        let issued_at_unix_ms = parse_timestamp(fields[4])?;
        let expires_at_unix_ms = parse_timestamp(fields[5])?;
        ensure_internal_time(now_unix_ms, issued_at_unix_ms, expires_at_unix_ms)?;
        Ok(AuthenticationContext {
            subject_id,
            kind: PrincipalKind::Workload { role },
            authenticated_at_unix_ms: issued_at_unix_ms,
            expires_at_unix_ms,
            issuer_fingerprint: self.verifier().issuer_fingerprint,
        })
    }

    pub fn issue_agent_run_credential(
        &self,
        run_id: &str,
        agent_id: &str,
        campaign_id: &str,
        class: AgentClass,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<String, IdentityError> {
        let run_id = EntityId::new(run_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let agent_id = EntityId::new(agent_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        let campaign_id =
            EntityId::new(campaign_id).map_err(|_| IdentityError::InvalidIdentityData)?;
        if expires_at_unix_ms <= issued_at_unix_ms {
            return Err(IdentityError::InvalidIdentityData);
        }
        let claims = format!(
            "{}|agent|{}|{}|{}|{}|{}|{}",
            INTERNAL_TOKEN_VERSION,
            run_id,
            agent_id,
            campaign_id,
            class.as_str(),
            issued_at_unix_ms,
            expires_at_unix_ms
        );
        Ok(self.sign_claims(&claims))
    }

    pub fn authenticate_agent_run(
        &self,
        credential: &str,
        now_unix_ms: u64,
    ) -> Result<AuthenticationContext, IdentityError> {
        let claims = self.verify_signed_credential(credential)?;
        let fields = claims.split('|').collect::<Vec<_>>();
        if fields.len() != 8 || fields[0] != INTERNAL_TOKEN_VERSION || fields[1] != "agent" {
            return Err(IdentityError::InvalidInternalCredential);
        }
        let run_id =
            EntityId::new(fields[2]).map_err(|_| IdentityError::InvalidInternalCredential)?;
        let subject_id =
            EntityId::new(fields[3]).map_err(|_| IdentityError::InvalidInternalCredential)?;
        let campaign_id =
            EntityId::new(fields[4]).map_err(|_| IdentityError::InvalidInternalCredential)?;
        let class = AgentClass::parse(fields[5])?;
        let issued_at_unix_ms = parse_timestamp(fields[6])?;
        let expires_at_unix_ms = parse_timestamp(fields[7])?;
        ensure_internal_time(now_unix_ms, issued_at_unix_ms, expires_at_unix_ms)?;
        Ok(AuthenticationContext {
            subject_id,
            kind: PrincipalKind::AgentRun {
                run_id,
                class,
                campaign_id,
            },
            authenticated_at_unix_ms: issued_at_unix_ms,
            expires_at_unix_ms,
            issuer_fingerprint: self.verifier().issuer_fingerprint,
        })
    }

    fn issue_session(
        &mut self,
        user_id: EntityId,
        now_unix_ms: u64,
    ) -> Result<LoginSession, IdentityError> {
        let (token_hash, record, session) = self.generate_session(user_id, now_unix_ms)?;
        if let Some(database) = self.database.as_mut() {
            persist_session(database, token_hash, &record, None)?;
        }
        self.sessions_by_hash.insert(token_hash, record);
        self.publish_verification_state()?;
        Ok(session)
    }

    fn generate_session(
        &self,
        user_id: EntityId,
        now_unix_ms: u64,
    ) -> Result<([u8; 32], SessionRecord, LoginSession), IdentityError> {
        let mut raw_token = [0_u8; SESSION_TOKEN_BYTES];
        OsRng.fill_bytes(&mut raw_token);
        let token = SessionToken(hex_encode(&raw_token));
        let token_hash = hash_token(token.expose());
        let mut raw_session_id = [0_u8; 16];
        OsRng.fill_bytes(&mut raw_session_id);
        let session_id = EntityId::new(format!("session_{}", hex_encode(&raw_session_id)))
            .map_err(|_| IdentityError::InvalidIdentityData)?;
        let expires_at_unix_ms = now_unix_ms
            .checked_add(self.session_ttl_ms)
            .ok_or(IdentityError::InvalidIdentityData)?;
        let record = SessionRecord {
            session_id,
            user_id,
            issued_at_unix_ms: now_unix_ms,
            expires_at_unix_ms,
            revoked: false,
        };
        let session = LoginSession {
            token,
            expires_at_unix_ms,
        };
        Ok((token_hash, record, session))
    }

    fn sign_claims(&self, claims: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.signing_key)
            .expect("identity key length was validated at construction");
        mac.update(claims.as_bytes());
        format!("{claims}.{}", hex_encode(&mac.finalize().into_bytes()))
    }

    fn verify_signed_credential<'a>(&self, credential: &'a str) -> Result<&'a str, IdentityError> {
        let (claims, supplied_signature) = credential
            .rsplit_once('.')
            .ok_or(IdentityError::InvalidInternalCredential)?;
        let supplied_signature = hex_decode_32(supplied_signature)?;
        let mut mac = HmacSha256::new_from_slice(&self.signing_key)
            .map_err(|_| IdentityError::InvalidSigningKey)?;
        mac.update(claims.as_bytes());
        mac.verify_slice(&supplied_signature)
            .map_err(|_| IdentityError::InvalidInternalCredential)?;
        Ok(claims)
    }
}

fn normalize_login(login: &str) -> Result<String, IdentityError> {
    let normalized = login.trim().to_ascii_lowercase();
    if normalized.len() < 3
        || normalized.len() > 254
        || !normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "@._-+".contains(character))
    {
        return Err(IdentityError::InvalidIdentityData);
    }
    Ok(normalized)
}

fn global_role_name(role: GlobalRole) -> &'static str {
    match role {
        GlobalRole::User => "USER",
        GlobalRole::Moderator => "MODERATOR",
        GlobalRole::ServerOwner => "SERVER_OWNER",
    }
}

fn parse_global_role(value: &str) -> Result<GlobalRole, IdentityError> {
    match value {
        "USER" => Ok(GlobalRole::User),
        "MODERATOR" => Ok(GlobalRole::Moderator),
        "SERVER_OWNER" => Ok(GlobalRole::ServerOwner),
        _ => Err(IdentityError::InvalidIdentityData),
    }
}

fn campaign_role_name(role: CampaignRole) -> &'static str {
    match role {
        CampaignRole::CampaignOwner => "CAMPAIGN_OWNER",
        CampaignRole::HumanKeeper => "HUMAN_KEEPER",
        CampaignRole::Player => "PLAYER",
        CampaignRole::Spectator => "SPECTATOR",
    }
}

fn parse_campaign_role(value: &str) -> Result<CampaignRole, IdentityError> {
    match value {
        "CAMPAIGN_OWNER" => Ok(CampaignRole::CampaignOwner),
        "HUMAN_KEEPER" => Ok(CampaignRole::HumanKeeper),
        "PLAYER" => Ok(CampaignRole::Player),
        "SPECTATOR" => Ok(CampaignRole::Spectator),
        _ => Err(IdentityError::InvalidIdentityData),
    }
}

fn authority_mode_name(mode: &AuthorityMode) -> &'static str {
    match mode {
        AuthorityMode::HumanKp => "HUMAN_KP",
        AuthorityMode::AiKp => "AI_KP",
    }
}

fn parse_authority_mode(value: &str) -> Result<AuthorityMode, IdentityError> {
    match value {
        "HUMAN_KP" => Ok(AuthorityMode::HumanKp),
        "AI_KP" => Ok(AuthorityMode::AiKp),
        _ => Err(IdentityError::InvalidIdentityData),
    }
}

fn timestamp_from_i64(value: i64) -> Result<u64, IdentityError> {
    u64::try_from(value).map_err(|_| IdentityError::InvalidIdentityData)
}

fn map_postgres_error(_error: postgres::Error) -> IdentityError {
    IdentityError::PersistenceUnavailable
}

fn persist_session(
    database: &mut impl GenericClient,
    token_hash: [u8; 32],
    record: &SessionRecord,
    rotated_from: Option<&EntityId>,
) -> Result<(), IdentityError> {
    let issued_at =
        i64::try_from(record.issued_at_unix_ms).map_err(|_| IdentityError::InvalidIdentityData)?;
    let expires_at =
        i64::try_from(record.expires_at_unix_ms).map_err(|_| IdentityError::InvalidIdentityData)?;
    let rotated_from = rotated_from.map(EntityId::as_str);
    database
        .execute(
            "INSERT INTO sessions (\
                session_id, user_id, token_hash, issued_at, expires_at, rotated_from_session_id\
             ) VALUES (\
                $1, $2, $3, to_timestamp($4::bigint / 1000.0), \
                to_timestamp($5::bigint / 1000.0), $6\
             )",
            &[
                &record.session_id.as_str(),
                &record.user_id.as_str(),
                &&token_hash[..],
                &issued_at,
                &expires_at,
                &rotated_from,
            ],
        )
        .map_err(map_postgres_error)?;
    Ok(())
}

fn validate_password(password: &str) -> Result<(), IdentityError> {
    if password.len() < 12 || password.len() > 1024 {
        return Err(IdentityError::InvalidIdentityData);
    }
    Ok(())
}

fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn parse_timestamp(value: &str) -> Result<u64, IdentityError> {
    value
        .parse::<u64>()
        .map_err(|_| IdentityError::InvalidInternalCredential)
}

fn ensure_internal_time(
    now_unix_ms: u64,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
) -> Result<(), IdentityError> {
    if issued_at_unix_ms > now_unix_ms || expires_at_unix_ms <= now_unix_ms {
        return Err(IdentityError::InternalCredentialExpired);
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn hex_decode_32(value: &str) -> Result<[u8; 32], IdentityError> {
    if value.len() != 64 {
        return Err(IdentityError::InvalidInternalCredential);
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (hex_value(pair[0])? << 4) | hex_value(pair[1])?;
    }
    Ok(output)
}

fn hex_value(value: u8) -> Result<u8, IdentityError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(IdentityError::InvalidInternalCredential),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7; 32];

    fn service() -> IdentityService {
        IdentityService::new(&KEY, 60_000).expect("valid identity service")
    }

    #[test]
    fn password_session_rotation_and_logout_are_enforced() {
        let mut service = service();
        service
            .create_user(
                "user_owner",
                "Owner@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        assert_eq!(
            service.login("owner@example.test", "wrong password!!", 1_000),
            Err(IdentityError::InvalidCredentials)
        );

        let session = service
            .login("owner@example.test", "correct horse battery staple", 1_000)
            .unwrap();
        let context = service
            .authenticate_session(Some(session.token.expose()), 1_001)
            .unwrap();
        assert_eq!(context.subject_id().as_str(), "user_owner");

        let replacement = service
            .refresh_session(session.token.expose(), 2_000)
            .unwrap();
        assert_eq!(
            service.authenticate_session(Some(session.token.expose()), 2_001),
            Err(IdentityError::SessionRevoked)
        );
        service.logout(replacement.token.expose()).unwrap();
        assert_eq!(
            service.authenticate_session(Some(replacement.token.expose()), 2_002),
            Err(IdentityError::SessionRevoked)
        );
    }

    #[test]
    fn verifier_rejects_a_context_after_its_session_is_logged_out() {
        let mut service = service();
        service
            .create_user(
                "user_owner",
                "owner@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        let session = service
            .login("owner@example.test", "correct horse battery staple", 1_000)
            .unwrap();
        let context = service
            .authenticate_session(Some(session.token.expose()), 1_001)
            .unwrap();
        let verifier = service.verifier();
        verifier.verify(&context, 1_002).unwrap();

        service.logout(session.token.expose()).unwrap();

        assert_eq!(
            verifier.verify(&context, 1_003),
            Err(IdentityError::SessionRevoked)
        );
    }

    #[test]
    fn logged_out_context_cannot_manage_memberships() {
        let mut service = service();
        service
            .create_user(
                "user_owner",
                "owner@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        service
            .create_user(
                "user_player",
                "player@example.test",
                "another correct horse battery",
                GlobalRole::User,
            )
            .unwrap();
        let session = service
            .login("owner@example.test", "correct horse battery staple", 1_000)
            .unwrap();
        let context = service
            .authenticate_session(Some(session.token.expose()), 1_001)
            .unwrap();
        service.logout(session.token.expose()).unwrap();

        assert_eq!(
            service.grant_membership(
                &context,
                "campaign_a",
                "user_player",
                CampaignRole::Player,
                1_002,
            ),
            Err(IdentityError::SessionRevoked)
        );
    }

    #[test]
    fn campaign_owner_cannot_grant_privileged_roles_and_human_keeper_is_unique() {
        let mut service = service();
        for (id, login, role) in [
            (
                "server_owner",
                "server@example.test",
                GlobalRole::ServerOwner,
            ),
            ("campaign_owner", "campaign@example.test", GlobalRole::User),
            ("keeper_a", "keeper-a@example.test", GlobalRole::User),
            ("keeper_b", "keeper-b@example.test", GlobalRole::User),
        ] {
            service
                .create_user(id, login, "correct horse battery staple", role)
                .unwrap();
        }
        let server_session = service
            .login("server@example.test", "correct horse battery staple", 1_000)
            .unwrap();
        let server = service
            .authenticate_session(Some(server_session.token.expose()), 1_001)
            .unwrap();
        service
            .grant_membership(
                &server,
                "campaign_a",
                "campaign_owner",
                CampaignRole::CampaignOwner,
                1_002,
            )
            .unwrap();
        service
            .grant_membership(
                &server,
                "campaign_a",
                "keeper_a",
                CampaignRole::HumanKeeper,
                1_002,
            )
            .unwrap();
        assert_eq!(
            service.grant_membership(
                &server,
                "campaign_a",
                "keeper_b",
                CampaignRole::HumanKeeper,
                1_003,
            ),
            Err(IdentityError::MembershipDenied)
        );

        let owner_session = service
            .login(
                "campaign@example.test",
                "correct horse battery staple",
                1_000,
            )
            .unwrap();
        let owner = service
            .authenticate_session(Some(owner_session.token.expose()), 1_001)
            .unwrap();
        for privileged_role in [CampaignRole::CampaignOwner, CampaignRole::HumanKeeper] {
            assert_eq!(
                service.grant_membership(&owner, "campaign_a", "keeper_b", privileged_role, 1_003,),
                Err(IdentityError::MembershipDenied)
            );
        }
        service
            .grant_membership(
                &owner,
                "campaign_a",
                "keeper_b",
                CampaignRole::Player,
                1_003,
            )
            .unwrap();
    }

    #[test]
    fn membership_is_resource_scoped_and_fail_closed() {
        let mut service = service();
        service
            .create_user(
                "user_owner",
                "owner@example.test",
                "correct horse battery staple",
                GlobalRole::ServerOwner,
            )
            .unwrap();
        service
            .create_user(
                "user_player",
                "player@example.test",
                "another correct horse battery",
                GlobalRole::User,
            )
            .unwrap();
        let owner_session = service
            .login("owner@example.test", "correct horse battery staple", 1_000)
            .unwrap();
        let owner = service
            .authenticate_session(Some(owner_session.token.expose()), 1_001)
            .unwrap();
        service
            .grant_membership(
                &owner,
                "campaign_a",
                "user_player",
                CampaignRole::Player,
                1_002,
            )
            .unwrap();

        let player_session = service
            .login(
                "player@example.test",
                "another correct horse battery",
                1_000,
            )
            .unwrap();
        let player = service
            .authenticate_session(Some(player_session.token.expose()), 1_001)
            .unwrap();
        service
            .require_membership(
                &player,
                &EntityId::new("campaign_a").unwrap(),
                &[CampaignRole::Player],
                1_002,
            )
            .unwrap();
        let campaign_a = EntityId::new("campaign_a").unwrap();
        let command_actor = service.command_actor(&player, &campaign_a, 1_002).unwrap();
        assert_eq!(command_actor.role(), &ActorRole::Investigator);
        assert_eq!(
            service.require_membership(
                &player,
                &EntityId::new("campaign_b").unwrap(),
                &[CampaignRole::Player],
                1_002,
            ),
            Err(IdentityError::MembershipRequired)
        );
        assert_eq!(
            service.command_actor(&player, &EntityId::new("campaign_b").unwrap(), 1_002),
            Err(IdentityError::MembershipRequired)
        );
    }

    #[test]
    fn forged_workload_and_cross_campaign_agent_tokens_are_rejected() {
        let service = service();
        let workload = service
            .issue_workload_credential("workflow_1", WorkloadRole::WorkflowEngine, 1_000, 2_000)
            .unwrap();
        let mut forged = workload.clone();
        forged.replace_range(3..4, "x");
        assert_eq!(
            service.authenticate_workload(&forged, 1_500),
            Err(IdentityError::InvalidInternalCredential)
        );
        assert!(service.authenticate_workload(&workload, 1_500).is_ok());
        let workload_context = service.authenticate_workload(&workload, 1_500).unwrap();
        let campaign = EntityId::new("campaign_a").unwrap();
        let verifier = service.verifier();
        verifier
            .verify_actor(
                &workload_context,
                &Actor::verified_workload("workflow_1", KernelWorkloadRole::WorkflowEngine)
                    .unwrap(),
                &campaign,
                1_500,
            )
            .unwrap();
        assert_eq!(
            verifier.verify_actor(
                &workload_context,
                &Actor::verified_workload("workflow_1", KernelWorkloadRole::RulesEngine).unwrap(),
                &campaign,
                1_500,
            ),
            Err(IdentityError::InvalidInternalCredential)
        );

        let agent = service
            .issue_agent_run_credential(
                "run_1",
                "agent_keeper",
                "campaign_a",
                AgentClass::AiKeeperOrchestrator,
                1_000,
                2_000,
            )
            .unwrap();
        let context = service.authenticate_agent_run(&agent, 1_500).unwrap();
        assert_eq!(
            context.require_campaign(&EntityId::new("campaign_b").unwrap()),
            Err(IdentityError::CampaignScopeMismatch)
        );
    }
}
