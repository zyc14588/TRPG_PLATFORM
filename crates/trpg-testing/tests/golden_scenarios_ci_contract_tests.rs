use trpg_shared_kernel::{Actor, ActorRole, AuthorityMode, FormalWritePath};
use trpg_testing::{
    command_for_contract, golden_scenarios_ci, record_contract_decision, TestingQualityAction,
    TestingQualityEvent, TestingQualityRepository, TESTING_QUALITY_DECISION_RECORDED_EVENT,
};

const GOLDEN_ACTIONS: &str =
    include_str!("../../../fixtures/actions/golden_salt_bell_action_sequence.v1.json.md");
const S11_EXPECTED: &str = include_str!(
    "../../../fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md"
);

#[test]
fn golden_scenarios_ci_records_current_safe_contract() {
    let (repository, command, event) =
        record_contract_decision(&golden_scenarios_ci::contract()).expect("recorded");

    assert_eq!(repository.events().len(), 1);
    assert_eq!(event.event_type, TESTING_QUALITY_DECISION_RECORDED_EVENT);
    assert_eq!(event.idempotency_key, command.idempotency_key);
    assert_eq!(event.visibility, command.visibility);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert_eq!(event.correlation_id, command.correlation_id);
    assert_eq!(event.causation_id, command.causation_id);

    match &event.payload {
        TestingQualityEvent::ContractValidated {
            module,
            action,
            fixture_count,
            assertion_count,
        } => {
            assert_eq!(*module, golden_scenarios_ci::MODULE);
            assert_eq!(*action, TestingQualityAction::VerifyGoldenScenariosCi);
            assert_eq!(*fixture_count, 4);
            assert_eq!(*assertion_count, 4);
        }
    }
}

#[test]
fn golden_scenarios_ci_covers_s11_expected_records_and_errors() {
    let combined = format!("{GOLDEN_ACTIONS}\n{S11_EXPECTED}");

    for marker in golden_scenarios_ci::required_fixture_markers() {
        assert!(combined.contains(marker), "missing marker {marker}");
    }
    for record in golden_scenarios_ci::expected_records() {
        assert!(combined.contains(record), "missing record {record}");
        assert!(golden_scenarios_ci::covers_expected_record(record));
    }
    for error_code in golden_scenarios_ci::expected_error_codes() {
        assert!(combined.contains(error_code), "missing error {error_code}");
        assert!(golden_scenarios_ci::covers_expected_error(error_code));
    }

    assert!(golden_scenarios_ci::covers_fixture(
        "fixtures/actions/golden_salt_bell_action_sequence.v1.json.md"
    ));
    assert!(golden_scenarios_ci::covers_fixture(
        "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md"
    ));
}

#[test]
fn golden_scenarios_ci_rejects_direct_agent_and_authority_bypass() {
    let contract = golden_scenarios_ci::contract();

    let mut direct_agent_command = command_for_contract(&contract);
    direct_agent_command.write_path = FormalWritePath::DirectAgent;
    let mut repository = TestingQualityRepository::default();
    let direct_agent_error = golden_scenarios_ci::evaluate(&mut repository, &direct_agent_command)
        .expect_err("direct agent write rejected");
    assert_eq!(direct_agent_error.code(), "DIRECT_AGENT_STATE_WRITE");
    assert!(repository.events().is_empty());

    let mut authority_command = command_for_contract(&contract);
    authority_command.authority_mode = AuthorityMode::AiKp;
    authority_command.actor =
        Actor::new("actor_human_keeper", ActorRole::HumanKeeper).expect("valid actor");
    let authority_error = golden_scenarios_ci::evaluate(&mut repository, &authority_command)
        .expect_err("authority mismatch rejected");
    assert_eq!(authority_error.code(), "AUTHORITY_VIOLATION");
    assert!(repository.events().is_empty());
}
