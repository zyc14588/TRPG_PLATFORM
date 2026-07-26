crate::define_api_realtime_contract_module!(
    "api_contracts",
    "ApiContractsRecorded",
    "api_contracts.event_schema",
    crate::contract_core::ApiRealtimeOperation::RegisterSchema
);

use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use trpg_shared_kernel::{
    ActorOrigin, ActorRole, AuthenticatedCommandContext, AuthorityContract, AuthorityMode,
    CanonicalPolicyAudit, ChangePolicy, EntityId, WorkloadRole,
};

pub type CoreApiFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, CoreApiError>> + Send + 'a>>;

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ApiCommandFields {
    pub command_id: String,
    pub idempotency_key: String,
    pub expected_version: i64,
    pub correlation_id: String,
    pub causation_id: String,
    pub trace_id: String,
}

impl ApiCommandFields {
    fn validate(&self) -> Result<(), CoreApiError> {
        for (field, value) in [
            ("command_id", self.command_id.as_str()),
            ("correlation_id", self.correlation_id.as_str()),
            ("causation_id", self.causation_id.as_str()),
            ("trace_id", self.trace_id.as_str()),
        ] {
            EntityId::new(value).map_err(|_| CoreApiError::InvalidInput(field))?;
        }
        if self.idempotency_key.trim().is_empty()
            || self.idempotency_key.len() > 160
            || self.expected_version < 0
        {
            return Err(CoreApiError::InvalidInput("command_fields"));
        }
        Ok(())
    }
}

/// Server-created authorization context. It is deliberately not
/// Deserialize: clients cannot submit policy decision IDs or authority
/// bindings in a request body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizedCoreApiContext {
    requesting: AuthenticatedCommandContext,
    workflow: AuthenticatedCommandContext,
    authority_mode: AuthorityMode,
    authority_contract_version: i64,
    authentication_reference: String,
    policy_audit: CanonicalPolicyAudit,
}

impl AuthorizedCoreApiContext {
    /// Creates a core API context only from two typed, server-side command
    /// contexts: the authenticated user request and the verified workflow that
    /// owns the formal write. No role, actor id, Campaign binding, Authority
    /// binding, or policy resource can be supplied as a free-form field.
    pub fn from_authenticated_contexts(
        requesting: AuthenticatedCommandContext,
        workflow: AuthenticatedCommandContext,
        authority_contract: &AuthorityContract,
        policy_audit: CanonicalPolicyAudit,
    ) -> Result<Self, CoreApiError> {
        let authentication_reference = match requesting.actor().origin() {
            ActorOrigin::UserSession { session_id } => session_id.to_string(),
            _ => return Err(CoreApiError::InvalidAuthorizationContext),
        };
        let authority_binding = authority_contract
            .binding()
            .map_err(|_| CoreApiError::InvalidAuthorizationContext)?;
        if !matches!(
            workflow.actor().origin(),
            ActorOrigin::Workload {
                role: WorkloadRole::WorkflowEngine
            }
        ) || workflow.actor().role() != &ActorRole::Workflow
            || requesting.resource() != workflow.resource()
            || requesting.authority() != workflow.authority()
            || workflow.authority() != &authority_binding
            || authority_contract.campaign_id() != workflow.resource().campaign_id()
            || !authority_contract.is_locked()
            || authority_contract.change_policy() != ChangePolicy::ForkOnly
            || policy_audit.actor_id != workflow.actor().id().as_str()
            || policy_audit.actor_origin != "workload"
            || policy_audit.authentication_reference != workflow.actor().id().as_str()
            || policy_audit.resource_type != workflow.resource().resource_type().as_str()
            || policy_audit.resource_id != workflow.resource().resource_id().as_str()
            || policy_audit.action != "write_official_state"
            || policy_audit.requested_role != "workflow"
        {
            return Err(CoreApiError::InvalidAuthorizationContext);
        }
        let authority_contract_version = i64::try_from(authority_contract.version())
            .map_err(|_| CoreApiError::InvalidAuthorizationContext)?;
        Ok(Self {
            requesting,
            workflow,
            authority_mode: authority_contract.authority_mode().clone(),
            authority_contract_version,
            authentication_reference,
            policy_audit,
        })
    }

    pub fn actor_id(&self) -> &str {
        self.requesting.actor().id().as_str()
    }

    pub fn actor_role(&self) -> &'static str {
        self.requesting.actor().canonical_role_name()
    }

    pub fn authentication_reference(&self) -> &str {
        &self.authentication_reference
    }

    pub fn workflow_actor_id(&self) -> &str {
        self.workflow.actor().id().as_str()
    }

    pub fn workflow_actor_role(&self) -> &'static str {
        self.workflow.actor().canonical_role_name()
    }

    pub fn campaign_id(&self) -> &str {
        self.workflow.resource().campaign_id().as_str()
    }

    pub fn authority_contract_id(&self) -> &str {
        self.workflow.authority().contract_id().as_str()
    }

    pub const fn authority_mode(&self) -> &'static str {
        match self.authority_mode {
            AuthorityMode::HumanKp => "human_kp",
            AuthorityMode::AiKp => "ai_kp",
        }
    }

    pub fn authority_owner(&self) -> &str {
        self.workflow.authority().authority_owner().as_str()
    }

    pub const fn authority_contract_version(&self) -> i64 {
        self.authority_contract_version
    }

    pub fn policy_audit(&self) -> &CanonicalPolicyAudit {
        &self.policy_audit
    }

    fn validate(
        &self,
        campaign_id: &str,
        resource_type: &str,
        resource_id: &str,
    ) -> Result<(), CoreApiError> {
        let required = [
            self.actor_id(),
            self.actor_role(),
            self.authentication_reference(),
            self.workflow_actor_id(),
            self.workflow_actor_role(),
            self.campaign_id(),
            self.authority_contract_id(),
            self.authority_mode(),
            self.authority_owner(),
            self.policy_audit.actor_id.as_str(),
            self.policy_audit.actor_origin.as_str(),
            self.policy_audit.authentication_reference.as_str(),
            self.policy_audit.resource_type.as_str(),
            self.policy_audit.resource_id.as_str(),
            self.policy_audit.action.as_str(),
            self.policy_audit.requested_role.as_str(),
            self.policy_audit.openfga_decision_id.as_str(),
            self.policy_audit.openfga_policy_revision.as_str(),
            self.policy_audit.opa_decision_id.as_str(),
            self.policy_audit.opa_policy_revision.as_str(),
        ];
        if required.iter().any(|value| value.trim().is_empty())
            || self.campaign_id() != campaign_id
            || self.authority_contract_version <= 0
            || !matches!(self.authority_mode(), "human_kp" | "ai_kp")
            || self.workflow_actor_role() != "workflow"
            || self.policy_audit.actor_id != self.workflow_actor_id()
            || self.policy_audit.actor_origin != "workload"
            || self.policy_audit.authentication_reference != self.workflow_actor_id()
            || self.policy_audit.resource_type != resource_type
            || self.policy_audit.resource_id != resource_id
            || self.policy_audit.action != "write_official_state"
            || self.policy_audit.requested_role != "workflow"
        {
            return Err(CoreApiError::InvalidAuthorizationContext);
        }
        EntityId::new(self.actor_id())
            .and_then(|_| EntityId::new(campaign_id))
            .and_then(|_| EntityId::new(self.authority_contract_id()))
            .and_then(|_| EntityId::new(self.workflow_actor_id()))
            .and_then(|_| EntityId::new(resource_id))
            .map_err(|_| CoreApiError::InvalidAuthorizationContext)?;
        Ok(())
    }

    fn require_keeper(&self) -> Result<(), CoreApiError> {
        if matches!(
            self.requesting.actor().role(),
            ActorRole::HumanKeeper | ActorRole::CampaignOwner | ActorRole::ServerOwner
        ) {
            Ok(())
        } else {
            Err(CoreApiError::Forbidden)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct AuthoritySnapshotApiRequest {
    pub contract_id: String,
    pub authority_mode: String,
    pub authority_owner: String,
    pub ruleset_version: String,
    pub house_rules_version: String,
    pub scenario_version: String,
    pub prompt_version: String,
    pub agent_pack_version: String,
    pub tool_schema_version: String,
    pub safety_profile_version: String,
    pub ai_provider_snapshot: String,
    pub model_route_snapshot: String,
    pub character_sheet_template_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CreateCampaignApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub owner_user_id: String,
    pub title: String,
    pub room_id: String,
    pub room_name: String,
    pub created_at_unix_ms: u64,
    pub authority: AuthoritySnapshotApiRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct IssueInviteApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub invite_id: String,
    pub invited_user_id: String,
    pub role: String,
    pub expires_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptInviteApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub invite_id: String,
    pub accepting_user_id: String,
    pub raw_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CreateCharacterApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub character_id: String,
    pub owner_user_id: String,
    pub display_name: String,
    pub sheet_version_id: String,
    pub sheet_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CharacterTransitionApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub character_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CoreApiCommitReceipt {
    pub last_event_sequence: i64,
    pub aggregate_version: i64,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
pub struct IssuedInviteApiResponse {
    pub invite_id: String,
    pub raw_token: String,
    pub expires_at_unix_ms: u64,
    pub receipt: CoreApiCommitReceipt,
}

impl fmt::Debug for IssuedInviteApiResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuedInviteApiResponse")
            .field("invite_id", &self.invite_id)
            .field("raw_token", &"[REDACTED]")
            .field("expires_at_unix_ms", &self.expires_at_unix_ms)
            .field("receipt", &self.receipt)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreApiError {
    InvalidInput(&'static str),
    InvalidAuthorizationContext,
    Forbidden,
    Conflict(&'static str),
    Unavailable(&'static str),
}

impl CoreApiError {
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::InvalidInput(_) => 400,
            Self::InvalidAuthorizationContext | Self::Forbidden => 403,
            Self::Conflict(_) => 409,
            Self::Unavailable(_) => 503,
        }
    }
}

impl fmt::Display for CoreApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(field) => write!(formatter, "CORE_API_INPUT_INVALID:{field}"),
            Self::InvalidAuthorizationContext => {
                formatter.write_str("CORE_API_AUTHORIZATION_CONTEXT_INVALID")
            }
            Self::Forbidden => formatter.write_str("CORE_API_FORBIDDEN"),
            Self::Conflict(reason) => write!(formatter, "CORE_API_CONFLICT:{reason}"),
            Self::Unavailable(reason) => write!(formatter, "CORE_API_UNAVAILABLE:{reason}"),
        }
    }
}

impl Error for CoreApiError {}

pub trait CampaignCharacterCommandPort: Send + Sync {
    fn create_campaign<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CreateCampaignApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn issue_invite<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a IssueInviteApiRequest,
    ) -> CoreApiFuture<'a, IssuedInviteApiResponse>;

    fn accept_invite<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a AcceptInviteApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn create_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CreateCharacterApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn submit_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CharacterTransitionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn review_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CharacterTransitionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;
}

#[derive(Clone)]
pub struct CampaignCharacterApi<P> {
    port: Arc<P>,
}

impl<P> CampaignCharacterApi<P>
where
    P: CampaignCharacterCommandPort,
{
    pub fn new(port: Arc<P>) -> Self {
        Self { port }
    }

    pub async fn create_campaign(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CreateCampaignApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "campaign", &request.campaign_id)?;
        context.require_keeper()?;
        if request.command.expected_version != 0
            || request.owner_user_id != context.actor_id()
            || request.authority.contract_id != context.authority_contract_id()
            || request.authority.authority_owner != context.authority_owner()
            || request.authority.authority_mode.to_ascii_lowercase() != context.authority_mode()
            || request.title.trim().is_empty()
            || request.room_name.trim().is_empty()
            || request.created_at_unix_ms == 0
        {
            return Err(CoreApiError::InvalidInput("campaign"));
        }
        for value in [
            request.campaign_id.as_str(),
            request.owner_user_id.as_str(),
            request.room_id.as_str(),
            request.authority.contract_id.as_str(),
        ] {
            EntityId::new(value).map_err(|_| CoreApiError::InvalidInput("campaign_id"))?;
        }
        self.port.create_campaign(context, request).await
    }

    pub async fn issue_invite(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &IssueInviteApiRequest,
    ) -> Result<IssuedInviteApiResponse, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "campaign_invite", &request.invite_id)?;
        context.require_keeper()?;
        if request.command.expected_version != 0
            || !matches!(request.role.as_str(), "PLAYER" | "SPECTATOR")
            || request.expires_at_unix_ms == 0
        {
            return Err(CoreApiError::InvalidInput("invite"));
        }
        for value in [request.invite_id.as_str(), request.invited_user_id.as_str()] {
            EntityId::new(value).map_err(|_| CoreApiError::InvalidInput("invite_id"))?;
        }
        self.port.issue_invite(context, request).await
    }

    pub async fn accept_invite(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &AcceptInviteApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "campaign_invite", &request.invite_id)?;
        if request.command.expected_version != 1
            || request.accepting_user_id != context.actor_id()
            || request.raw_token.is_empty()
            || request.raw_token.len() > 256
        {
            return Err(CoreApiError::InvalidInput("invite_accept"));
        }
        self.port.accept_invite(context, request).await
    }

    pub async fn create_character(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CreateCharacterApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "character", &request.character_id)?;
        if request.command.expected_version != 0
            || request.owner_user_id != context.actor_id()
            || request.display_name.trim().is_empty()
        {
            return Err(CoreApiError::InvalidInput("character"));
        }
        let sheet: serde_json::Value = serde_json::from_str(&request.sheet_json)
            .map_err(|_| CoreApiError::InvalidInput("character_sheet"))?;
        if !sheet.is_object() {
            return Err(CoreApiError::InvalidInput("character_sheet"));
        }
        self.port.create_character(context, request).await
    }

    pub async fn submit_character(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CharacterTransitionApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "character", &request.character_id)?;
        self.port.submit_character(context, request).await
    }

    pub async fn review_character(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CharacterTransitionApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "character", &request.character_id)?;
        context.require_keeper()?;
        self.port.review_character(context, request).await
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum PlayerActionIntentApiRequest {
    Investigation {
        skill_name: String,
        clue_id: String,
        clue_importance: String,
        adjustment: String,
    },
    SanityCheck {
        success_loss: u8,
        failure_loss: u8,
        day_key: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitPlayerActionApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub action_id: String,
    pub character_id: String,
    pub scene_id: String,
    pub submitted_at_unix_ms: u64,
    pub intent: PlayerActionIntentApiRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmPlayerActionApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub action_id: String,
    pub resolved_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PlayerActionApiReceipt {
    pub first_event_sequence: i64,
    pub last_event_sequence: i64,
    pub aggregate_version: i64,
    pub state: String,
    pub realtime_delta_id: String,
}

pub trait PlayerActionCommandPort: Send + Sync {
    fn submit_player_action<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a SubmitPlayerActionApiRequest,
    ) -> CoreApiFuture<'a, PlayerActionApiReceipt>;

    fn confirm_player_action<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ConfirmPlayerActionApiRequest,
    ) -> CoreApiFuture<'a, PlayerActionApiReceipt>;
}

#[derive(Clone)]
pub struct PlayerActionApi<P> {
    port: Arc<P>,
}

impl<P> PlayerActionApi<P>
where
    P: PlayerActionCommandPort,
{
    pub fn new(port: Arc<P>) -> Self {
        Self { port }
    }

    pub async fn submit(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &SubmitPlayerActionApiRequest,
    ) -> Result<PlayerActionApiReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "player_action", &request.action_id)?;
        for value in [
            request.campaign_id.as_str(),
            request.action_id.as_str(),
            request.character_id.as_str(),
            request.scene_id.as_str(),
        ] {
            EntityId::new(value).map_err(|_| CoreApiError::InvalidInput("player_action_id"))?;
        }
        if context.authority_mode() != "human_kp"
            || context.actor_role() != "investigator"
            || request.command.expected_version != 0
            || request.submitted_at_unix_ms == 0
        {
            return Err(CoreApiError::Forbidden);
        }
        match &request.intent {
            PlayerActionIntentApiRequest::Investigation {
                skill_name,
                clue_id,
                clue_importance,
                adjustment,
            } => {
                if skill_name.trim().is_empty()
                    || skill_name.len() > 128
                    || EntityId::new(clue_id).is_err()
                    || !matches!(clue_importance.as_str(), "CORE" | "OPTIONAL")
                    || !matches!(adjustment.as_str(), "NONE" | "BONUS" | "PENALTY")
                {
                    return Err(CoreApiError::InvalidInput("investigation_intent"));
                }
            }
            PlayerActionIntentApiRequest::SanityCheck {
                success_loss,
                failure_loss,
                day_key,
            } => {
                if day_key.trim().is_empty()
                    || day_key.len() > 128
                    || success_loss > failure_loss
                    || *failure_loss > 99
                {
                    return Err(CoreApiError::InvalidInput("sanity_intent"));
                }
            }
        }
        self.port.submit_player_action(context, request).await
    }

    pub async fn confirm(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &ConfirmPlayerActionApiRequest,
    ) -> Result<PlayerActionApiReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "player_action", &request.action_id)?;
        context.require_keeper()?;
        if context.authority_mode() != "human_kp"
            || context.actor_id() != context.authority_owner()
            || request.command.expected_version != 1
            || request.resolved_at_unix_ms == 0
        {
            return Err(CoreApiError::Forbidden);
        }
        self.port.confirm_player_action(context, request).await
    }
}

#[cfg(test)]
mod tests {
    use super::{AcceptInviteApiRequest, IssueInviteApiRequest};

    #[test]
    fn invite_api_rejects_client_supplied_clock_fields() {
        let command = serde_json::json!({
            "command_id": "command_invite_clock",
            "idempotency_key": "idempotency_invite_clock",
            "expected_version": 0,
            "correlation_id": "correlation_invite_clock",
            "causation_id": "causation_invite_clock",
            "trace_id": "trace_invite_clock"
        });
        let issue = serde_json::json!({
            "command": command,
            "campaign_id": "campaign_invite_clock",
            "invite_id": "invite_clock",
            "invited_user_id": "player_invite_clock",
            "role": "PLAYER",
            "expires_at_unix_ms": 2_000_000_000_000_u64,
            "now_unix_ms": 1
        });
        assert!(serde_json::from_value::<IssueInviteApiRequest>(issue).is_err());

        let accept = serde_json::json!({
            "command": {
                "command_id": "command_accept_clock",
                "idempotency_key": "idempotency_accept_clock",
                "expected_version": 1,
                "correlation_id": "correlation_accept_clock",
                "causation_id": "causation_accept_clock",
                "trace_id": "trace_accept_clock"
            },
            "campaign_id": "campaign_invite_clock",
            "invite_id": "invite_clock",
            "accepting_user_id": "player_invite_clock",
            "raw_token": "opaque-token",
            "accepted_at_unix_ms": 1
        });
        assert!(serde_json::from_value::<AcceptInviteApiRequest>(accept).is_err());
    }
}
