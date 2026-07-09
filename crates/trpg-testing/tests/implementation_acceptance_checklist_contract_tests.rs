use trpg_testing::{implementation_acceptance_checklist, record_contract_decision};

const S11_ACCEPTANCE: &str =
    include_str!("../../../stages/s11-testing-quality-golden-ci/ACCEPTANCE_PROMPT.md");

#[test]
fn implementation_acceptance_checklist_preserves_strict_governance() {
    record_contract_decision(&implementation_acceptance_checklist::contract()).expect("recorded");

    assert!(S11_ACCEPTANCE.contains("P0"));
    assert!(S11_ACCEPTANCE.contains("P1"));
    assert!(S11_ACCEPTANCE.contains("visibility"));
    assert!(implementation_acceptance_checklist::required_items()
        .iter()
        .all(|item| item.required));
    assert!(implementation_acceptance_checklist::authority_contract_requires_fork());
}
