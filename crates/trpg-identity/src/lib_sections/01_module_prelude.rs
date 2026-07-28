
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use hmac::{Hmac, Mac};
use native_tls::{Certificate, Protocol, TlsConnector};
use postgres::config::{Host as PostgresHost, SslMode as PostgresSslMode};
use postgres::{Client, Config as PostgresConfig, GenericClient, NoTls};
use postgres_native_tls::MakeTlsConnector;
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use trpg_shared_kernel::{
    Actor, ActorOrigin, ActorRole, AgentClass as KernelAgentClass, AuthorityContract,
    AuthorityContractDraft, AuthorityMode, AuthorityVersionSnapshotDraft, EntityId, PrincipalScope,
    Visibility, VisibilityKind, WorkloadRole as KernelWorkloadRole,
};

type HmacSha256 = Hmac<Sha256>;

const SESSION_TOKEN_BYTES: usize = 32;
const SIGNING_KEY_BYTES: usize = 32;
const INTERNAL_TOKEN_VERSION: &str = "v1";
const LOGIN_FAILURE_LIMIT: u32 = 5;
const LOGIN_FAILURE_WINDOW_MS: u64 = 60_000;
const LOGIN_BLOCK_MS: u64 = 60_000;
const DUMMY_PASSWORD: &str = "identity timing equalization password";
const DEFAULT_ARGON2_CONCURRENCY: usize = 2;
const DISTRIBUTED_LOGIN_SCRIPT: &str = r#"
local current = tonumber(redis.call('GET', KEYS[1]) or '0')
local limit = tonumber(ARGV[1])
local window_ms = tonumber(ARGV[2])
local block_ms = tonumber(ARGV[3])
if current >= limit then
    return 0
end
current = redis.call('INCR', KEYS[1])
if current == 1 then
    redis.call('PEXPIRE', KEYS[1], window_ms)
end
if current >= limit then
    redis.call('PEXPIRE', KEYS[1], block_ms)
end
return 1
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpAuthStatus {
    Unauthorized401,
    Forbidden403,
    Conflict409,
    TooManyRequests429,
    Internal500,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityError {
    InvalidCredentials,
    LoginRateLimited,
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
            Self::LoginRateLimited => "LOGIN_RATE_LIMITED",
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
            Self::LoginRateLimited => HttpAuthStatus::TooManyRequests429,
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

/// Independent read connection used by distributed verifiers. It deliberately
/// does not rely on the creating process's replicated maps: every user replay
/// decision checks the durable session, campaign role, and (when relevant)
/// group grant at decision time.
#[derive(Clone)]
struct PersistentVerificationStore {
    database: Arc<Mutex<Client>>,
}

impl fmt::Debug for PersistentVerificationStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PersistentVerificationStore")
            .field("database", &"[POSTGRESQL CONNECTION]")
            .finish()
    }
}
