use trpg_testing::{decision_trace_map, record_contract_decision};

#[test]
fn decision_trace_map_covers_all_batch_prompts_and_primary_outputs() {
    record_contract_decision(&decision_trace_map::contract()).expect("recorded");

    assert_eq!(decision_trace_map::BATCH_038_PROMPT_IDS.len(), 25);
    assert_eq!(decision_trace_map::BATCH_039_PROMPT_IDS.len(), 25);
    assert_eq!(decision_trace_map::BATCH_040_PROMPT_IDS.len(), 25);

    let rows = decision_trace_map::primary_trace_rows();

    assert_eq!(rows.len(), 23);
    assert!(rows
        .iter()
        .any(|row| row.module == "testing_quality::visibility_leakage_tests"));
    assert!(rows
        .iter()
        .any(|row| row.module == "testing_quality::golden_ci_test_matrix"));
    assert!(rows
        .iter()
        .any(|row| row.module == "testing_quality::test_strategy_impl"));
    assert!(rows
        .iter()
        .any(|row| row.module == "testing_quality::latest_deep_research_rust_summary"));
    assert!(rows
        .iter()
        .any(|row| row.module == "testing_quality::research_decision_matrix"));
    assert!(rows
        .iter()
        .all(|row| row.output.starts_with("crates/trpg-testing/src/")));
    assert!(rows
        .iter()
        .all(|row| !row.module.contains("v5") && !row.output.contains("v5")));
}
