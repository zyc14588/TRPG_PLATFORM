use trpg_testing::{contract_test_matrix, record_contract_decision};

#[test]
fn contract_test_matrix_has_one_row_per_primary_module() {
    record_contract_decision(&contract_test_matrix::contract()).expect("recorded");

    let rows = contract_test_matrix::matrix_rows();

    assert_eq!(rows.len(), 23);
    assert!(rows
        .iter()
        .any(|row| row.test_file.ends_with("benchmark_plan_contract_tests.rs")));
    assert!(rows
        .iter()
        .any(|row| row.requirement == "verify_visibility_leakage"));
    assert!(rows
        .iter()
        .any(|row| row.requirement == "verify_golden_ci_test_matrix"));
    assert!(rows
        .iter()
        .any(|row| row.requirement == "verify_requirement_to_test_trace"));
    assert!(rows.iter().any(|row| row
        .test_file
        .ends_with("test_strategy_impl_contract_tests.rs")));
    assert!(rows.iter().any(|row| row
        .test_file
        .ends_with("latest_deep_research_rust_summary_contract_tests.rs")));
    assert!(rows.iter().any(|row| row
        .test_file
        .ends_with("research_decision_matrix_contract_tests.rs")));
    assert!(rows
        .iter()
        .all(|row| row.module.starts_with("testing_quality::")));
}
