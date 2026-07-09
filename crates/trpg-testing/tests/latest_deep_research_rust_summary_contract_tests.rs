use trpg_testing::{latest_deep_research_rust_summary, record_contract_decision};

const S11_TEST_PLAN: &str =
    include_str!("../../../stages/s11-testing-quality-golden-ci/TEST_PLAN.md");

#[test]
fn latest_deep_research_rust_summary_records_current_safe_decisions() {
    record_contract_decision(&latest_deep_research_rust_summary::contract()).expect("recorded");

    assert_eq!(
        latest_deep_research_rust_summary::selected_for("database_access"),
        Some("sqlx")
    );
    assert_eq!(
        latest_deep_research_rust_summary::selected_for("canonical_state"),
        Some("event_store")
    );
    assert!(latest_deep_research_rust_summary::all_decisions_have_validation());
    assert!(S11_TEST_PLAN.contains("cargo test -p trpg-testing --all-features"));
}
