use trpg_shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};

pub const PLATFORM_AUTHORIZATION_GRANTED_EVENT: &str =
    "platform.policy_authz.authorization_granted";
pub const POLICY_AUTHZ_METRIC_MODULE: &str = "policy_authz";
pub const POLICY_AUTHZ_REQUIRED_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_projection_lag_events",
    "trpg_visibility_redaction_total",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyGateDecision {
    Permit,
    Deny,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluatePlatformAuthorization {
    pub principal: String,
    pub resource: String,
    pub action: String,
    pub openfga_decision: PolicyGateDecision,
    pub opa_decision: PolicyGateDecision,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum PolicyAuthzEvent {
    PlatformAuthorizationGranted {
        principal: String,
        resource: String,
        action: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyAuthzError {
    PrincipalRequired,
    ResourceRequired,
    ActionRequired,
    PolicyDenied,
}

impl From<PolicyAuthzError> for TrpgError {
    fn from(error: PolicyAuthzError) -> Self {
        match error {
            PolicyAuthzError::PrincipalRequired => {
                TrpgError::InvalidConfiguration("principal_required")
            }
            PolicyAuthzError::ResourceRequired => {
                TrpgError::InvalidConfiguration("resource_required")
            }
            PolicyAuthzError::ActionRequired => TrpgError::InvalidConfiguration("action_required"),
            PolicyAuthzError::PolicyDenied => TrpgError::PolicyDenied,
        }
    }
}

pub type PolicyAuthzEventEnvelope = EventEnvelope<PolicyAuthzEvent>;
pub type PolicyAuthzRepository = EventStore<PolicyAuthzEvent>;

pub struct PolicyAuthzService;

impl PolicyAuthzService {
    pub fn evaluate_platform_authorization(
        repository: &mut PolicyAuthzRepository,
        command: &CommandEnvelope<EvaluatePlatformAuthorization>,
    ) -> KernelResult<PolicyAuthzEventEnvelope> {
        if command.payload.principal.trim().is_empty() {
            return Err(PolicyAuthzError::PrincipalRequired.into());
        }
        if command.payload.resource.trim().is_empty() {
            return Err(PolicyAuthzError::ResourceRequired.into());
        }
        if command.payload.action.trim().is_empty() {
            return Err(PolicyAuthzError::ActionRequired.into());
        }
        if command.payload.openfga_decision == PolicyGateDecision::Deny
            || command.payload.opa_decision == PolicyGateDecision::Deny
        {
            return Err(PolicyAuthzError::PolicyDenied.into());
        }

        repository.append(
            command,
            PLATFORM_AUTHORIZATION_GRANTED_EVENT,
            PolicyAuthzEvent::PlatformAuthorizationGranted {
                principal: command.payload.principal.clone(),
                resource: command.payload.resource.clone(),
                action: command.payload.action.clone(),
            },
        )
    }
}

pub fn evaluate_platform_authorization(
    repository: &mut PolicyAuthzRepository,
    command: &CommandEnvelope<EvaluatePlatformAuthorization>,
) -> KernelResult<PolicyAuthzEventEnvelope> {
    PolicyAuthzService::evaluate_platform_authorization(repository, command)
}
