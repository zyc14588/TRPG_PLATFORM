use trpg_shared_kernel::{FormalWritePath, TrpgError};
use trpg_testing::{
    benchmark_plan, command_for_contract, record_contract_decision, TestingQualityRepository,
    TESTING_QUALITY_DECISION_RECORDED_EVENT,
};

#[test]
fn benchmark_plan_records_governed_contract_event() {
    let contract = benchmark_plan::contract();

    let (repository, command, event) =
        record_contract_decision(&contract).expect("benchmark contract recorded");

    assert_eq!(repository.events().len(), 1);
    assert_eq!(event.event_type, TESTING_QUALITY_DECISION_RECORDED_EVENT);
    assert_eq!(event.visibility, command.visibility);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert!(benchmark_plan::required_budgets()
        .iter()
        .any(|budget| budget.required_gate == "no_visibility_leakage"));
    assert!(benchmark_plan::sample_within_budget(
        "golden_scenario_replay",
        1_000
    ));
    assert!(!benchmark_plan::sample_within_budget(
        "golden_scenario_replay",
        2_001
    ));
}

#[test]
fn benchmark_plan_rejects_direct_agent_write_path() {
    let contract = benchmark_plan::contract();
    let mut command = command_for_contract(&contract);
    command.write_path = FormalWritePath::DirectAgent;
    let mut repository = TestingQualityRepository::default();

    let err = benchmark_plan::evaluate(&mut repository, &command)
        .expect_err("direct agent write is blocked");

    assert_eq!(err, TrpgError::DirectAgentStateWrite);
    assert!(repository.events().is_empty());
}
