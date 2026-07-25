use crate::readme::{
    append_platform_event, PlatformEvent, PlatformEventEnvelope, PlatformEventStore,
};
pub use trpg_security_governance::secret::{
    KmsClient, KmsSecretResolver, SecretManager, SecretReference, SecretResolver,
};
pub use trpg_security_governance::{
    DeploymentEnvironment, ProviderBoundaryAttestation, ProviderEndpoint,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult, TrpgError};

pub const DEPLOYMENT_CONFIGURED_EVENT: &str = "platform.deployment.configured";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigureDeployment {
    pub environment: DeploymentEnvironment,
    pub endpoint: ProviderEndpoint,
}

pub fn validate_provider_boundary<R: SecretResolver>(
    environment: &DeploymentEnvironment,
    endpoint: &ProviderEndpoint,
    secret_manager: &SecretManager<R>,
) -> KernelResult<ProviderBoundaryAttestation> {
    if endpoint.environment() != *environment {
        return Err(TrpgError::InvalidConfiguration(
            "deployment_environment_binding_mismatch",
        ));
    }
    trpg_security_governance::validate_provider_boundary(endpoint, secret_manager)
}

pub fn configure_deployment<R: SecretResolver>(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<ConfigureDeployment>,
    secret_manager: &SecretManager<R>,
) -> KernelResult<PlatformEventEnvelope> {
    let attestation = validate_provider_boundary(
        &command.payload.environment,
        &command.payload.endpoint,
        secret_manager,
    )?;

    append_platform_event(
        store,
        command,
        DEPLOYMENT_CONFIGURED_EVENT,
        PlatformEvent::DeploymentConfigured {
            environment: command.payload.environment.as_str().to_owned(),
            provider: command.payload.endpoint.provider_type().to_owned(),
            security_snapshot_digest: attestation.security_snapshot_digest().to_owned(),
        },
    )
}
