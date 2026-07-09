use trpg_testing::{record_contract_decision, test_strategy_impl};

const S11_TEST_PLAN: &str =
    include_str!("../../../stages/s11-testing-quality-golden-ci/TEST_PLAN.md");
const S11_TEST_DATA: &str =
    include_str!("../../../stages/s11-testing-quality-golden-ci/TEST_DATA.md");

#[test]
fn test_strategy_impl_orders_minimal_and_stage_checks() {
    record_contract_decision(&test_strategy_impl::contract()).expect("recorded");

    assert_eq!(
        test_strategy_impl::command_for_phase("minimal_related"),
        Some("cargo test -p trpg-testing --all-features")
    );
    assert!(test_strategy_impl::command_for_phase("stage_visibility")
        .expect("stage visibility command")
        .contains("visibility_leakage"));
    assert!(S11_TEST_PLAN.contains("cargo test -p trpg-testing --all-features"));
    assert!(S11_TEST_DATA.contains("provider_model_certification_cases"));
}
