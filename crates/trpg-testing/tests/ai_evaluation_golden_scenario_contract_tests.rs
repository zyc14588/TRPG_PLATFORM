use trpg_testing::{ai_evaluation_golden_scenario, record_contract_decision};

const GOLDEN_SCENARIO: &str =
    include_str!("../../../fixtures/scenarios/golden_salt_bell.scenario.yaml.md");
const MODEL_CERTIFICATION_CASES: &str =
    include_str!("../../../test-data/provider_model_certification_cases.md");

#[test]
fn ai_evaluation_golden_scenario_requires_tool_gate_and_certified_keeper() {
    record_contract_decision(&ai_evaluation_golden_scenario::contract()).expect("recorded");

    assert!(ai_evaluation_golden_scenario::requires_tool_request_only());
    assert!(ai_evaluation_golden_scenario::evaluation_guards()
        .iter()
        .any(|guard| guard.area == "agent_gateway"));
    assert!(GOLDEN_SCENARIO.contains("visibility"));
    assert!(MODEL_CERTIFICATION_CASES.contains("LOCAL_MODEL_LEVEL_4"));
}
