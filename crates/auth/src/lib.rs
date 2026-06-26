use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fmt,
    str::FromStr,
    time::{Duration, SystemTime},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct UserId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct RoomId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct RoomInviteId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct RefreshSessionId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EmailAddress(String);

impl EmailAddress {
    pub fn parse(value: impl Into<String>) -> Result<Self, AuthDomainError> {
        let value = value.into().trim().to_ascii_lowercase();
        if value.contains('@') && !value.chars().any(char::is_whitespace) {
            Ok(Self(value))
        } else {
            Err(AuthDomainError::InvalidEmail)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TokenHash(String);

impl TokenHash {
    pub fn new(value: impl Into<String>) -> Result<Self, AuthDomainError> {
        let value = value.into();
        if value.len() >= 16 {
            Ok(Self(value))
        } else {
            Err(AuthDomainError::WeakTokenHash)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct User {
    pub id: UserId,
    pub email: EmailAddress,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoomRole {
    Owner,
    Kp,
    AssistantKp,
    Pl,
    Observer,
    PublicScreen,
}

impl RoomRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Kp => "kp",
            Self::AssistantKp => "assistant_kp",
            Self::Pl => "pl",
            Self::Observer => "observer",
            Self::PublicScreen => "public_screen",
        }
    }

    pub fn can(self, action: RoomAction) -> bool {
        match self {
            Self::Owner => true,
            Self::Kp => matches!(
                action,
                RoomAction::ViewRoom
                    | RoomAction::SubmitPlayerAction
                    | RoomAction::ManageRules
                    | RoomAction::ManageMembers
                    | RoomAction::InviteMember
                    | RoomAction::ControlAgent
                    | RoomAction::ExportKp
                    | RoomAction::ViewAuditLog
                    | RoomAction::ViewKpContent
            ),
            Self::AssistantKp => matches!(
                action,
                RoomAction::ViewRoom
                    | RoomAction::SubmitPlayerAction
                    | RoomAction::ManageRules
                    | RoomAction::ControlAgent
                    | RoomAction::ExportKp
                    | RoomAction::ViewKpContent
            ),
            Self::Pl => matches!(
                action,
                RoomAction::ViewRoom | RoomAction::SubmitPlayerAction
            ),
            Self::Observer => matches!(action, RoomAction::ViewRoom),
            Self::PublicScreen => matches!(action, RoomAction::ViewRoom),
        }
    }
}

impl fmt::Display for RoomRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RoomRole {
    type Err = AuthDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "owner" => Ok(Self::Owner),
            "kp" => Ok(Self::Kp),
            "assistant_kp" => Ok(Self::AssistantKp),
            "pl" => Ok(Self::Pl),
            "observer" => Ok(Self::Observer),
            "public_screen" => Ok(Self::PublicScreen),
            _ => Err(AuthDomainError::UnknownRole(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoomAction {
    ViewRoom,
    ManageRoom,
    ManageMembers,
    InviteMember,
    ManageRules,
    SubmitPlayerAction,
    ControlAgent,
    ExportKp,
    ViewAuditLog,
    ViewKpContent,
    UseCloudProvider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityScope {
    PublicRule,
    RoomRule,
    PlVisibleClue,
    KpOnlyModule,
    KpSecret,
    CharacterPrivate,
    SessionLog,
    MemoryPrivate,
    SystemInternal,
}

impl VisibilityScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PublicRule => "public_rule",
            Self::RoomRule => "room_rule",
            Self::PlVisibleClue => "pl_visible_clue",
            Self::KpOnlyModule => "kp_only_module",
            Self::KpSecret => "kp_secret",
            Self::CharacterPrivate => "character_private",
            Self::SessionLog => "session_log",
            Self::MemoryPrivate => "memory_private",
            Self::SystemInternal => "system_internal",
        }
    }

    pub fn visible_to(self, role: RoomRole, actor_id: UserId, owner_id: Option<UserId>) -> bool {
        match self {
            Self::PublicRule | Self::RoomRule | Self::PlVisibleClue => true,
            Self::CharacterPrivate => {
                matches!(role, RoomRole::Owner | RoomRole::Kp | RoomRole::AssistantKp)
                    || owner_id == Some(actor_id)
            }
            Self::SessionLog => !matches!(role, RoomRole::PublicScreen),
            Self::KpOnlyModule | Self::KpSecret | Self::MemoryPrivate => {
                matches!(role, RoomRole::Owner | RoomRole::Kp | RoomRole::AssistantKp)
            }
            Self::SystemInternal => false,
        }
    }
}

impl fmt::Display for VisibilityScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for VisibilityScope {
    type Err = AuthDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "public_rule" => Ok(Self::PublicRule),
            "room_rule" => Ok(Self::RoomRule),
            "pl_visible_clue" => Ok(Self::PlVisibleClue),
            "kp_only_module" => Ok(Self::KpOnlyModule),
            "kp_secret" => Ok(Self::KpSecret),
            "character_private" => Ok(Self::CharacterPrivate),
            "session_log" => Ok(Self::SessionLog),
            "memory_private" => Ok(Self::MemoryPrivate),
            "system_internal" => Ok(Self::SystemInternal),
            _ => Err(AuthDomainError::UnknownVisibility(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoomPrivacyMode {
    Standard,
    PrivateHybrid,
    LocalOnly,
}

impl RoomPrivacyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::PrivateHybrid => "private_hybrid",
            Self::LocalOnly => "local_only",
        }
    }
}

impl fmt::Display for RoomPrivacyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RoomPrivacyMode {
    type Err = AuthDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "standard" => Ok(Self::Standard),
            "private_hybrid" => Ok(Self::PrivateHybrid),
            "local_only" => Ok(Self::LocalOnly),
            _ => Err(AuthDomainError::UnknownPrivacyMode(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub room_id: Option<Uuid>,
    pub role: RoomRole,
}

impl AuthContext {
    pub fn can_view(&self, scope: VisibilityScope) -> bool {
        scope.visible_to(self.role, UserId(self.user_id), None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Room {
    pub id: RoomId,
    pub owner_id: UserId,
    pub title: String,
    pub system_name: String,
    pub privacy_mode: RoomPrivacyMode,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoomMember {
    pub room_id: RoomId,
    pub user_id: UserId,
    pub role: RoomRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoomWithRole {
    pub room: Room,
    pub role: RoomRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomInvite {
    pub id: RoomInviteId,
    pub room_id: RoomId,
    pub invited_email: EmailAddress,
    pub role: RoomRole,
    pub token_hash: TokenHash,
    pub status: RoomInviteStatus,
    pub invited_by: UserId,
    pub accepted_by: Option<UserId>,
    pub expires_at: SystemTime,
}

impl RoomInvite {
    pub fn accept(&mut self, user_id: UserId, now: SystemTime) -> Result<(), AuthDomainError> {
        if now >= self.expires_at {
            self.status = RoomInviteStatus::Expired;
            return Err(AuthDomainError::InviteExpired);
        }
        if self.status != RoomInviteStatus::Pending {
            return Err(AuthDomainError::InviteNotPending);
        }
        self.status = RoomInviteStatus::Accepted;
        self.accepted_by = Some(user_id);
        Ok(())
    }

    pub fn revoke(&mut self) {
        if self.status == RoomInviteStatus::Pending {
            self.status = RoomInviteStatus::Revoked;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoomInviteStatus {
    Pending,
    Accepted,
    Revoked,
    Expired,
}

impl RoomInviteStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }
}

impl fmt::Display for RoomInviteStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessToken {
    pub token_id: Uuid,
    pub subject: UserId,
    pub issued_at: SystemTime,
    pub expires_at: SystemTime,
}

impl AccessToken {
    pub fn new(subject: UserId, now: SystemTime, ttl: Duration) -> Self {
        Self {
            token_id: Uuid::new_v4(),
            subject,
            issued_at: now,
            expires_at: now + ttl,
        }
    }

    pub fn is_expired(&self, now: SystemTime) -> bool {
        now >= self.expires_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshSession {
    pub id: RefreshSessionId,
    pub user_id: UserId,
    pub session_family_id: Uuid,
    pub current_token_hash: TokenHash,
    pub previous_token_hash: Option<TokenHash>,
    pub status: RefreshSessionStatus,
    pub expires_at: SystemTime,
    pub rotated_at: Option<SystemTime>,
    pub revoked_at: Option<SystemTime>,
}

impl RefreshSession {
    pub fn active(
        user_id: UserId,
        current_token_hash: TokenHash,
        now: SystemTime,
        ttl: Duration,
    ) -> Self {
        Self {
            id: RefreshSessionId(Uuid::new_v4()),
            user_id,
            session_family_id: Uuid::new_v4(),
            current_token_hash,
            previous_token_hash: None,
            status: RefreshSessionStatus::Active,
            expires_at: now + ttl,
            rotated_at: None,
            revoked_at: None,
        }
    }

    pub fn rotate(
        &mut self,
        presented_hash: &TokenHash,
        next_hash: TokenHash,
        now: SystemTime,
    ) -> Result<(), RefreshSessionError> {
        if now >= self.expires_at {
            self.status = RefreshSessionStatus::Expired;
            return Err(RefreshSessionError::Expired);
        }
        if self.status == RefreshSessionStatus::Revoked {
            return Err(RefreshSessionError::Revoked);
        }
        if presented_hash != &self.current_token_hash {
            if self.previous_token_hash.as_ref() == Some(presented_hash) {
                self.revoke(now);
                return Err(RefreshSessionError::ReuseDetected);
            }
            return Err(RefreshSessionError::InvalidToken);
        }

        self.previous_token_hash = Some(self.current_token_hash.clone());
        self.current_token_hash = next_hash;
        self.rotated_at = Some(now);
        Ok(())
    }

    pub fn revoke(&mut self, now: SystemTime) {
        self.status = RefreshSessionStatus::Revoked;
        self.revoked_at = Some(now);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RefreshSessionStatus {
    Active,
    Revoked,
    Expired,
}

impl RefreshSessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }
}

impl fmt::Display for RefreshSessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MagicLinkRequest {
    pub email: EmailAddress,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MagicLinkChallenge {
    pub challenge_id: Uuid,
    pub email: EmailAddress,
    pub token_hash: TokenHash,
    pub expires_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OidcLoginRequest {
    pub provider: String,
    pub authorization_code: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VerifiedIdentity {
    pub provider: IdentityProviderKind,
    pub provider_subject: String,
    pub email: EmailAddress,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdentityProviderKind {
    MagicLink,
    Oidc,
    Development,
}

#[async_trait]
pub trait MagicLinkPort: Send + Sync {
    async fn issue_magic_link(
        &self,
        request: MagicLinkRequest,
        now: SystemTime,
    ) -> Result<MagicLinkChallenge, AuthProviderError>;

    async fn verify_magic_link(
        &self,
        challenge: &MagicLinkChallenge,
        token_hash: &TokenHash,
        now: SystemTime,
    ) -> Result<VerifiedIdentity, AuthProviderError>;
}

#[async_trait]
pub trait OidcPort: Send + Sync {
    async fn exchange_oidc_code(
        &self,
        request: OidcLoginRequest,
    ) -> Result<VerifiedIdentity, AuthProviderError>;
}

#[derive(Debug, Clone, Default)]
pub struct MockAuthProvider;

#[derive(Debug, Clone, Default)]
pub struct DevelopmentAuthProvider;

#[async_trait]
impl MagicLinkPort for MockAuthProvider {
    async fn issue_magic_link(
        &self,
        request: MagicLinkRequest,
        now: SystemTime,
    ) -> Result<MagicLinkChallenge, AuthProviderError> {
        Ok(MagicLinkChallenge {
            challenge_id: Uuid::new_v4(),
            email: request.email,
            token_hash: TokenHash::new("mock_magic_link_hash")?,
            expires_at: now + Duration::from_secs(600),
        })
    }

    async fn verify_magic_link(
        &self,
        challenge: &MagicLinkChallenge,
        token_hash: &TokenHash,
        now: SystemTime,
    ) -> Result<VerifiedIdentity, AuthProviderError> {
        if now >= challenge.expires_at {
            return Err(AuthProviderError::Expired);
        }
        if token_hash != &challenge.token_hash {
            return Err(AuthProviderError::InvalidToken);
        }
        Ok(VerifiedIdentity {
            provider: IdentityProviderKind::MagicLink,
            provider_subject: challenge.email.as_str().to_owned(),
            email: challenge.email.clone(),
            display_name: challenge.email.as_str().to_owned(),
        })
    }
}

#[async_trait]
impl OidcPort for MockAuthProvider {
    async fn exchange_oidc_code(
        &self,
        request: OidcLoginRequest,
    ) -> Result<VerifiedIdentity, AuthProviderError> {
        let email = EmailAddress::parse(format!("{}@mock.oidc", request.authorization_code))?;
        Ok(VerifiedIdentity {
            provider: IdentityProviderKind::Oidc,
            provider_subject: format!("{}:{}", request.provider, request.authorization_code),
            email,
            display_name: "Mock OIDC User".to_owned(),
        })
    }
}

#[async_trait]
impl MagicLinkPort for DevelopmentAuthProvider {
    async fn issue_magic_link(
        &self,
        request: MagicLinkRequest,
        now: SystemTime,
    ) -> Result<MagicLinkChallenge, AuthProviderError> {
        MockAuthProvider.issue_magic_link(request, now).await
    }

    async fn verify_magic_link(
        &self,
        challenge: &MagicLinkChallenge,
        token_hash: &TokenHash,
        now: SystemTime,
    ) -> Result<VerifiedIdentity, AuthProviderError> {
        MockAuthProvider
            .verify_magic_link(challenge, token_hash, now)
            .await
    }
}

#[async_trait]
impl OidcPort for DevelopmentAuthProvider {
    async fn exchange_oidc_code(
        &self,
        request: OidcLoginRequest,
    ) -> Result<VerifiedIdentity, AuthProviderError> {
        MockAuthProvider.exchange_oidc_code(request).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuditLog {
    pub room_id: Option<RoomId>,
    pub actor_id: Option<UserId>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub scope: VisibilityScope,
    pub outcome: AuditOutcome,
    pub payload_json: Value,
    pub request_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Success,
    Failure,
}

impl AuditOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IdempotencyRecord {
    pub scope: String,
    pub key: String,
    pub request_hash: String,
    pub status: IdempotencyStatus,
    pub response_json: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyStatus {
    InProgress,
    Completed,
    Failed,
}

impl IdempotencyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdempotencyCheck {
    Claimed,
    Duplicate(IdempotencyRecord),
    Conflict,
}

pub fn authorize_room_action(
    ctx: &AuthContext,
    action: RoomAction,
    privacy_mode: RoomPrivacyMode,
) -> AuthzDecision {
    if action == RoomAction::UseCloudProvider && privacy_mode == RoomPrivacyMode::LocalOnly {
        return AuthzDecision::Deny;
    }
    if ctx.role.can(action) {
        AuthzDecision::Allow
    } else {
        AuthzDecision::Deny
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthzDecision {
    Allow,
    Deny,
}

#[async_trait]
pub trait IdentityRepository: Send + Sync {
    async fn upsert_user(&self, user: &User) -> Result<(), RepositoryError>;
    async fn find_user_by_email(
        &self,
        email: &EmailAddress,
    ) -> Result<Option<User>, RepositoryError>;
}

#[async_trait]
pub trait RefreshSessionRepository: Send + Sync {
    async fn create_refresh_session(&self, session: &RefreshSession)
        -> Result<(), RepositoryError>;
    async fn save_refresh_session(&self, session: &RefreshSession) -> Result<(), RepositoryError>;
    async fn find_refresh_session_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<RefreshSession>, RepositoryError>;
}

#[async_trait]
pub trait RoomRepository: Send + Sync {
    async fn create_room(&self, room: &Room) -> Result<(), RepositoryError>;
    async fn get_room(&self, room_id: RoomId) -> Result<Option<Room>, RepositoryError>;
    async fn list_rooms_for_user(
        &self,
        user_id: UserId,
    ) -> Result<Vec<RoomWithRole>, RepositoryError>;
    async fn add_room_member(&self, member: &RoomMember) -> Result<(), RepositoryError>;
    async fn get_room_member(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<Option<RoomMember>, RepositoryError>;
    async fn list_room_members(&self, room_id: RoomId) -> Result<Vec<RoomMember>, RepositoryError>;
    async fn create_room_invite(&self, invite: &RoomInvite) -> Result<(), RepositoryError>;
    async fn find_pending_room_invite_by_token_hash(
        &self,
        token_hash: &TokenHash,
    ) -> Result<Option<RoomInvite>, RepositoryError>;
    async fn save_room_invite(&self, invite: &RoomInvite) -> Result<(), RepositoryError>;
    async fn accept_room_invite(
        &self,
        invite: &RoomInvite,
        member: &RoomMember,
    ) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    async fn append_audit_log(&self, log: &AuditLog) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait IdempotencyRepository: Send + Sync {
    async fn claim_idempotency_key(
        &self,
        record: &IdempotencyRecord,
        ttl: Duration,
    ) -> Result<IdempotencyCheck, RepositoryError>;
}

#[async_trait]
pub trait RepositoryTransaction: Send {
    async fn commit(self: Box<Self>) -> Result<(), RepositoryError>;
    async fn rollback(self: Box<Self>) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait TransactionalRepository: Send + Sync {
    async fn begin_transaction(
        &self,
    ) -> Result<Box<dyn RepositoryTransaction + Send + '_>, RepositoryError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AuthDomainError {
    #[error("invalid email address")]
    InvalidEmail,
    #[error("token hash is too short")]
    WeakTokenHash,
    #[error("unknown room role: {0}")]
    UnknownRole(String),
    #[error("unknown visibility scope: {0}")]
    UnknownVisibility(String),
    #[error("unknown privacy mode: {0}")]
    UnknownPrivacyMode(String),
    #[error("invite is expired")]
    InviteExpired,
    #[error("invite is not pending")]
    InviteNotPending,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RefreshSessionError {
    #[error("refresh session is expired")]
    Expired,
    #[error("refresh session is revoked")]
    Revoked,
    #[error("refresh token reuse detected")]
    ReuseDetected,
    #[error("invalid refresh token")]
    InvalidToken,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AuthProviderError {
    #[error("auth provider challenge expired")]
    Expired,
    #[error("invalid auth provider token")]
    InvalidToken,
    #[error("auth provider rejected request: {0}")]
    Rejected(String),
    #[error(transparent)]
    Domain(#[from] AuthDomainError),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    #[error("not found")]
    NotFound,
    #[error("duplicate")]
    Duplicate,
    #[error("forbidden")]
    Forbidden,
    #[error("idempotency conflict")]
    IdempotencyConflict,
    #[error("retryable database error: {0}")]
    RetryableDb(String),
    #[error("database error: {0}")]
    Database(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: u128) -> UserId {
        UserId(Uuid::from_u128(id))
    }

    fn token(value: &str) -> TokenHash {
        TokenHash(value.to_owned())
    }

    #[test]
    fn room_role_permission_matrix() {
        let roles = [
            RoomRole::Owner,
            RoomRole::Kp,
            RoomRole::AssistantKp,
            RoomRole::Pl,
            RoomRole::Observer,
            RoomRole::PublicScreen,
        ];
        let expected = [
            (
                RoomAction::ManageRoom,
                [true, false, false, false, false, false],
            ),
            (
                RoomAction::ManageMembers,
                [true, true, false, false, false, false],
            ),
            (
                RoomAction::ManageRules,
                [true, true, true, false, false, false],
            ),
            (
                RoomAction::SubmitPlayerAction,
                [true, true, true, true, false, false],
            ),
            (
                RoomAction::ViewKpContent,
                [true, true, true, false, false, false],
            ),
            (RoomAction::ViewRoom, [true, true, true, true, true, true]),
        ];

        for (action, grants) in expected {
            for (role, allowed) in roles.into_iter().zip(grants) {
                assert_eq!(role.can(action), allowed, "{role:?} {action:?}");
            }
        }
    }

    #[test]
    fn visibility_scope_projection_matrix() {
        let owner = user(1);
        let actor = user(2);
        let roles = [
            RoomRole::Owner,
            RoomRole::Kp,
            RoomRole::AssistantKp,
            RoomRole::Pl,
            RoomRole::Observer,
            RoomRole::PublicScreen,
        ];
        let expected = [
            (
                VisibilityScope::PublicRule,
                [true, true, true, true, true, true],
            ),
            (
                VisibilityScope::PlVisibleClue,
                [true, true, true, true, true, true],
            ),
            (
                VisibilityScope::KpOnlyModule,
                [true, true, true, false, false, false],
            ),
            (
                VisibilityScope::KpSecret,
                [true, true, true, false, false, false],
            ),
            (
                VisibilityScope::MemoryPrivate,
                [true, true, true, false, false, false],
            ),
            (
                VisibilityScope::SessionLog,
                [true, true, true, true, true, false],
            ),
        ];

        for (scope, grants) in expected {
            for (role, allowed) in roles.into_iter().zip(grants) {
                assert_eq!(
                    scope.visible_to(role, actor, Some(owner)),
                    allowed,
                    "{role:?} {scope:?}"
                );
            }
        }

        assert!(VisibilityScope::CharacterPrivate.visible_to(RoomRole::Pl, owner, Some(owner)));
        assert!(!VisibilityScope::CharacterPrivate.visible_to(RoomRole::Pl, actor, Some(owner)));
    }

    #[test]
    fn refresh_session_rotation() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let mut session = RefreshSession::active(
            user(1),
            token("current_hash_1234"),
            now,
            Duration::from_secs(60),
        );

        let result = session.rotate(
            &token("current_hash_1234"),
            token("next_hash_567890"),
            now + Duration::from_secs(1),
        );

        assert_eq!(result, Ok(()));
        assert_eq!(
            session.previous_token_hash,
            Some(token("current_hash_1234"))
        );
        assert_eq!(session.current_token_hash, token("next_hash_567890"));
        assert_eq!(session.status, RefreshSessionStatus::Active);
    }

    #[test]
    fn refresh_token_reuse_revokes_session() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let mut session = RefreshSession::active(
            user(1),
            token("current_hash_1234"),
            now,
            Duration::from_secs(60),
        );

        let first = session.rotate(
            &token("current_hash_1234"),
            token("next_hash_567890"),
            now + Duration::from_secs(1),
        );
        let second = session.rotate(
            &token("current_hash_1234"),
            token("another_hash_000"),
            now + Duration::from_secs(2),
        );

        assert_eq!(first, Ok(()));
        assert_eq!(second, Err(RefreshSessionError::ReuseDetected));
        assert_eq!(session.status, RefreshSessionStatus::Revoked);
    }

    #[test]
    fn invitation_lifecycle() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let mut invite = RoomInvite {
            id: RoomInviteId(Uuid::nil()),
            room_id: RoomId(Uuid::nil()),
            invited_email: EmailAddress("pl@example.test".to_owned()),
            role: RoomRole::Pl,
            token_hash: token("invite_hash_12345"),
            status: RoomInviteStatus::Pending,
            invited_by: user(1),
            accepted_by: None,
            expires_at: now + Duration::from_secs(60),
        };

        assert_eq!(invite.accept(user(2), now + Duration::from_secs(1)), Ok(()));
        assert_eq!(invite.status, RoomInviteStatus::Accepted);
        assert_eq!(invite.accepted_by, Some(user(2)));

        invite.revoke();
        assert_eq!(invite.status, RoomInviteStatus::Accepted);
    }

    #[test]
    fn local_only_denies_cloud_provider() {
        let ctx = AuthContext {
            user_id: user(1).0,
            room_id: Some(RoomId(Uuid::nil()).0),
            role: RoomRole::Owner,
        };

        assert_eq!(
            authorize_room_action(
                &ctx,
                RoomAction::UseCloudProvider,
                RoomPrivacyMode::LocalOnly
            ),
            AuthzDecision::Deny
        );
        assert_eq!(
            authorize_room_action(
                &ctx,
                RoomAction::UseCloudProvider,
                RoomPrivacyMode::Standard
            ),
            AuthzDecision::Allow
        );
    }

    #[tokio::test]
    async fn mock_magic_link_verifies_without_network() -> Result<(), AuthProviderError> {
        let provider = MockAuthProvider;
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let challenge = provider
            .issue_magic_link(
                MagicLinkRequest {
                    email: EmailAddress("pl@example.test".to_owned()),
                    redirect_uri: "http://localhost/callback".to_owned(),
                },
                now,
            )
            .await?;

        let verified = provider
            .verify_magic_link(
                &challenge,
                &challenge.token_hash,
                now + Duration::from_secs(1),
            )
            .await;

        assert!(matches!(
            verified,
            Ok(VerifiedIdentity {
                provider: IdentityProviderKind::MagicLink,
                ..
            })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn mock_oidc_exchanges_code_without_network() {
        let provider = MockAuthProvider;
        let verified = provider
            .exchange_oidc_code(OidcLoginRequest {
                provider: "mock".to_owned(),
                authorization_code: "alice".to_owned(),
                redirect_uri: "http://localhost/callback".to_owned(),
            })
            .await;

        assert!(matches!(
            verified,
            Ok(VerifiedIdentity {
                provider: IdentityProviderKind::Oidc,
                ..
            })
        ));
    }
}
