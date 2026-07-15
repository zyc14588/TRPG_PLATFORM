use trpg_platform::api_contracts_impl::{
    register_api_command_contract, ApiContractsRepository, RegisterApiCommandContract,
    API_COMMAND_CONTRACT_REGISTERED_EVENT, API_CONTRACTS_IMPL_METRIC_MODULE,
    API_CONTRACTS_IMPL_REQUIRED_METRICS,
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
fn api_contracts_impl_rejects_authority_contract_violation() {
    let command = trpg_test_support::governed_command(
        command().payload,
        ActorRole::AiKeeper,
        AuthorityMode::HumanKp,
    );
    let mut repository = ApiContractsRepository::default();

    let err = register_api_command_contract(&mut repository, &command)
        .expect_err("authority mismatch denied");

    assert_eq!(err, TrpgError::AuthorityViolation);
    assert!(repository.events().is_empty());
}

#[test]
fn api_contracts_impl_keeps_visibility_and_fact_provenance_on_replay() {
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
fn api_contracts_impl_uses_current_safe_metric_module() {
    assert_eq!(API_CONTRACTS_IMPL_METRIC_MODULE, "api_contracts_impl");
    assert!(API_CONTRACTS_IMPL_REQUIRED_METRICS.contains(&"trpg_command_total"));
    assert!(API_CONTRACTS_IMPL_REQUIRED_METRICS.contains(&"trpg_visibility_redaction_total"));
}
