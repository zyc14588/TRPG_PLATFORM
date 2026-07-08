use crate::readme::{
    append_platform_event, PlatformEvent, PlatformEventEnvelope, PlatformEventStore,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult, TrpgError};

pub const LOCAL_DEV_ENVIRONMENT_VALIDATED_EVENT: &str = "platform.local_dev_environment.validated";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalService {
    pub name: String,
    pub url: String,
    pub authenticated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidateLocalDevEnvironment {
    pub profile: String,
    pub services: Vec<LocalService>,
}

pub fn validate_local_dev_profile(profile: &ValidateLocalDevEnvironment) -> KernelResult<()> {
    if profile.services.is_empty() {
        return Err(TrpgError::InvalidConfiguration("local_service_required"));
    }

    for service in &profile.services {
        if !loopback_url(&service.url) {
            return Err(TrpgError::PolicyDenied);
        }
    }

    Ok(())
}

fn loopback_url(url: &str) -> bool {
    let normalized = url.trim().to_ascii_lowercase();
    normalized.starts_with("http://localhost:")
        || normalized.starts_with("https://localhost:")
        || normalized.starts_with("http://127.0.0.1:")
        || normalized.starts_with("https://127.0.0.1:")
}

pub fn record_local_dev_environment(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<ValidateLocalDevEnvironment>,
) -> KernelResult<PlatformEventEnvelope> {
    validate_local_dev_profile(&command.payload)?;

    append_platform_event(
        store,
        command,
        LOCAL_DEV_ENVIRONMENT_VALIDATED_EVENT,
        PlatformEvent::LocalDevEnvironmentValidated {
            profile: command.payload.profile.clone(),
            service_count: command.payload.services.len(),
        },
    )
}
