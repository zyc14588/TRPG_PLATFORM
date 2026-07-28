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
