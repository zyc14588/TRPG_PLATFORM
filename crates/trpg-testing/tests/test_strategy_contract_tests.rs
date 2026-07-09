use trpg_testing::{record_contract_decision, test_strategy};

const S11_TEST_PLAN: &str =
    include_str!("../../../stages/s11-testing-quality-golden-ci/TEST_PLAN.md");
const S11_TEST_DATA: &str =
    include_str!("../../../stages/s11-testing-quality-golden-ci/TEST_DATA.md");

#[test]
fn test_strategy_maps_layers_and_negative_cases() {
    record_contract_decision(&test_strategy::contract()).expect("recorded");

    assert!(S11_TEST_PLAN.contains("cargo test -p trpg-testing --all-features"));
    assert!(S11_TEST_DATA.contains("provider_model_certification_cases"));
    assert!(test_strategy::required_layers()
        .iter()
        .any(|layer| layer.name == "stage_fixture"));
    assert!(test_strategy::covers_negative_case("permission_denied"));
    assert!(test_strategy::covers_negative_case("prompt_injection"));
}
