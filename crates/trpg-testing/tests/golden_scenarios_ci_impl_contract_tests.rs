use trpg_testing::{golden_scenarios_ci_impl, record_contract_decision};

const TUTORIAL_SCENARIO: &str =
    include_str!("../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml.md");
const GOLDEN_ACTIONS: &str =
    include_str!("../../../fixtures/actions/golden_salt_bell_action_sequence.v1.json.md");
const EXPORT_DIFF: &str = include_str!(
    "../../../fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md"
);

#[test]
fn golden_scenarios_ci_impl_covers_tutorial_golden_and_export_diff() {
    record_contract_decision(&golden_scenarios_ci_impl::contract()).expect("recorded");

    assert!(golden_scenarios_ci_impl::covers_fixture(
        "fixtures/scenarios/tutorial_mist_archive.scenario.yaml.md"
    ));
    assert!(golden_scenarios_ci_impl::covers_fixture(
        "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md"
    ));
    assert!(TUTORIAL_SCENARIO.contains("tutorial") || TUTORIAL_SCENARIO.contains("Tutorial"));
    assert!(GOLDEN_ACTIONS.contains("dice_source"));
    assert!(EXPORT_DIFF.contains("VisibilityLeakageTestPassed"));
}
