use trpg_platform::local_dev_environment::{
    record_local_dev_environment, validate_local_dev_profile, LocalService,
    ValidateLocalDevEnvironment, LOCAL_DEV_ENVIRONMENT_VALIDATED_EVENT,
};
use trpg_platform::PlatformEventStore;
use trpg_shared_kernel::{ActorRole, AuthorityMode, CommandEnvelope, TrpgError};

fn local_profile(url: &str) -> ValidateLocalDevEnvironment {
    ValidateLocalDevEnvironment {
        profile: "dev".to_owned(),
        services: vec![LocalService {
            name: "model-provider".to_owned(),
            url: url.to_owned(),
            authenticated: false,
        }],
    }
}

#[test]
fn local_dev_allows_loopback_service() {
    validate_local_dev_profile(&local_profile("http://localhost:11434")).expect("loopback allowed");
}

#[test]
fn local_dev_rejects_public_service_url() {
    let err = validate_local_dev_profile(&local_profile("http://192.0.2.10:11434"))
        .expect_err("public url denied");

    assert_eq!(err, TrpgError::PolicyDenied);
}

#[test]
fn local_dev_rejects_hostname_that_only_contains_loopback_text() {
    let err = validate_local_dev_profile(&local_profile("http://localhost.example:11434"))
        .expect_err("hostname trap denied");

    assert_eq!(err, TrpgError::PolicyDenied);
}

#[test]
fn local_dev_validation_is_evented() {
    let command = CommandEnvelope::governed(
        local_profile("http://127.0.0.1:11434"),
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    let mut store = PlatformEventStore::default();

    let event = record_local_dev_environment(&mut store, &command).expect("local profile recorded");

    assert_eq!(event.event_type, LOCAL_DEV_ENVIRONMENT_VALIDATED_EVENT);
}
