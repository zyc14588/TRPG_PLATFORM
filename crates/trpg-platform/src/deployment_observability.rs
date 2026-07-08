use crate::readme::{
    append_platform_event, redact_for_observability, PlatformEvent, PlatformEventEnvelope,
    PlatformEventStore,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult, TrpgError};

pub const DEPLOYMENT_HEALTH_OBSERVED_EVENT: &str =
    "platform.deployment_observability.health_observed";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObserveDeploymentHealth {
    pub service: String,
    pub healthy: bool,
    pub has_healthcheck: bool,
    pub detail: String,
}

pub fn observe_deployment_health(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<ObserveDeploymentHealth>,
) -> KernelResult<PlatformEventEnvelope> {
    if !command.payload.has_healthcheck {
        return Err(TrpgError::InvalidConfiguration(
            "service_healthcheck_required",
        ));
    }

    append_platform_event(
        store,
        command,
        DEPLOYMENT_HEALTH_OBSERVED_EVENT,
        PlatformEvent::DeploymentHealthObserved {
            service: command.payload.service.clone(),
            healthy: command.payload.healthy,
            detail: redact_for_observability(&command.visibility, &command.payload.detail),
        },
    )
}
