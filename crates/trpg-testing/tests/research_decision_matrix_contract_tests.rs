use trpg_testing::{record_contract_decision, research_decision_matrix};

const V1_ACCEPTANCE: &str = include_str!("../../../V1_ACCEPTANCE_EVIDENCE_MATRIX.md");

#[test]
fn research_decision_matrix_keeps_v1_governance_choices_testable() {
    record_contract_decision(&research_decision_matrix::contract()).expect("recorded");

    assert_eq!(
        research_decision_matrix::decision_for("event_projection_bus")
            .expect("event bus decision")
            .default_choice,
        "nats_jetstream"
    );
    assert_eq!(
        research_decision_matrix::decision_for("relationship_and_context_policy")
            .expect("policy decision")
            .default_choice,
        "openfga_and_opa"
    );
    assert!(research_decision_matrix::all_rows_have_v1_validation());
    assert!(V1_ACCEPTANCE.contains("Event Store"));
}
