use crate::deployment_ops::{validate_provider_boundary, DeploymentEnvironment, ProviderEndpoint};
use trpg_shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};

pub const DEPLOYMENT_OPERATION_APPLIED_EVENT: &str =
    "platform.deployment_ops_impl.operation_applied";
pub const DEPLOYMENT_OPS_IMPL_METRIC_MODULE: &str = "deployment_ops_impl";
pub const DEPLOYMENT_OPS_IMPL_REQUIRED_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_projection_lag_events",
    "trpg_visibility_redaction_total",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyDeploymentOperation {
    pub deployment_id: String,
    pub environment: DeploymentEnvironment,
    pub endpoint: ProviderEndpoint,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum DeploymentOpsEvent {
    DeploymentOperationApplied {
        deployment_id: String,
        environment: String,
        provider: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeploymentOpsError {
    DeploymentIdRequired,
}

impl From<DeploymentOpsError> for TrpgError {
    fn from(error: DeploymentOpsError) -> Self {
        match error {
            DeploymentOpsError::DeploymentIdRequired => {
                TrpgError::InvalidConfiguration("deployment_id_required")
            }
        }
    }
}

pub type DeploymentOpsEventEnvelope = EventEnvelope<DeploymentOpsEvent>;
pub type DeploymentOpsRepository = EventStore<DeploymentOpsEvent>;

pub struct DeploymentOpsService;

impl DeploymentOpsService {
    pub fn apply_deployment_operation(
        repository: &mut DeploymentOpsRepository,
        command: &CommandEnvelope<ApplyDeploymentOperation>,
    ) -> KernelResult<DeploymentOpsEventEnvelope> {
        if command.payload.deployment_id.trim().is_empty() {
            return Err(DeploymentOpsError::DeploymentIdRequired.into());
        }
        validate_provider_boundary(&command.payload.environment, &command.payload.endpoint)?;

        repository.append(
            command,
            DEPLOYMENT_OPERATION_APPLIED_EVENT,
            DeploymentOpsEvent::DeploymentOperationApplied {
                deployment_id: command.payload.deployment_id.clone(),
                environment: command.payload.environment.as_str().to_owned(),
                provider: command.payload.endpoint.provider.clone(),
            },
        )
    }
}

pub fn apply_deployment_operation(
    repository: &mut DeploymentOpsRepository,
    command: &CommandEnvelope<ApplyDeploymentOperation>,
) -> KernelResult<DeploymentOpsEventEnvelope> {
    DeploymentOpsService::apply_deployment_operation(repository, command)
}
