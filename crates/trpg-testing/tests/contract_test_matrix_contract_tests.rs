use trpg_testing::{contract_test_matrix, record_contract_decision};

#[test]
fn contract_test_matrix_has_one_row_per_primary_module() {
    record_contract_decision(&contract_test_matrix::contract()).expect("recorded");

    let rows = contract_test_matrix::matrix_rows();

    assert_eq!(rows.len(), 12);
    assert!(rows
        .iter()
        .any(|row| row.test_file.ends_with("benchmark_plan_contract_tests.rs")));
    assert!(rows
        .iter()
        .any(|row| row.requirement == "verify_visibility_leakage"));
    assert!(rows
        .iter()
        .all(|row| row.module.starts_with("testing_quality::")));
}
