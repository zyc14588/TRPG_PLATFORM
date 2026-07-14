use trpg_platform::api_contracts::{
    register_api_command_contract, ApiContractsRepository, RegisterApiCommandContract,
    API_COMMAND_CONTRACT_REGISTERED_EVENT, API_CONTRACTS_METRIC_MODULE,
    API_CONTRACTS_REQUIRED_METRICS,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, PrincipalScope, TrpgError, Visibility,
    VisibilityLabel,
};

fn command() -> CommandEnvelope<RegisterApiCommandContract> {
    trpg_test_support::governed_command(
        RegisterApiCommandContract {
            contract_id: "platform_commands".to_owned(),
            route: "/api/v1/internal/platform/commands".to_owned(),
            schema_version: 1,
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    )
}

#[test]
fn api_contracts_rejects_authority_contract_violation() {
    let command = trpg_test_support::governed_command(
        command().payload,
        ActorRole::AiKeeper,
        AuthorityMode::HumanKp,
    );
    let mut repository = ApiContractsRepository::default();

    let err =
        register_api_command_contract(&mut repository, &command).expect_err("authority denied");

    assert_eq!(err, TrpgError::AuthorityViolation);
    assert!(repository.events().is_empty());
}

#[test]
fn api_contracts_keeps_visibility_and_fact_provenance_on_replay() {
    let mut command = command();
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let mut repository = ApiContractsRepository::default();

    let event =
        register_api_command_contract(&mut repository, &command).expect("contract recorded");

    assert_eq!(event.event_type, API_COMMAND_CONTRACT_REGISTERED_EVENT);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert!(repository
        .replay_visible(&PrincipalScope::Public)
        .is_empty());
    assert_eq!(repository.replay_visible(&PrincipalScope::System).len(), 1);
}

#[test]
fn api_contracts_requires_idempotency_and_expected_version() {
    let mut repository = ApiContractsRepository::default();
    let mut missing_idempotency = command();
    missing_idempotency.idempotency_key.clear();

    let err = register_api_command_contract(&mut repository, &missing_idempotency)
        .expect_err("idempotency key required");

    assert_eq!(err, TrpgError::MissingIdempotencyKey);
    assert!(repository.events().is_empty());

    let first = command();
    register_api_command_contract(&mut repository, &first).expect("first append succeeds");
    let mut stale = command();
    stale.idempotency_key = "idem_002".to_owned();

    let err = register_api_command_contract(&mut repository, &stale).expect_err("version conflict");

    assert_eq!(
        err,
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1
        }
    );
}

#[test]
fn api_contracts_uses_current_safe_event_and_metric_names() {
    assert_eq!(API_CONTRACTS_METRIC_MODULE, "api_contracts");
    assert_eq!(
        API_COMMAND_CONTRACT_REGISTERED_EVENT,
        "platform.api_contracts.command_contract_registered"
    );
    assert!(!API_COMMAND_CONTRACT_REGISTERED_EVENT.contains("impl"));
    assert!(API_CONTRACTS_REQUIRED_METRICS.contains(&"trpg_command_total"));
    assert!(API_CONTRACTS_REQUIRED_METRICS.contains(&"trpg_visibility_redaction_total"));
}
