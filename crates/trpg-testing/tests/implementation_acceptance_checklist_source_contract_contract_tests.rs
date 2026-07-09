use trpg_testing::{implementation_acceptance_checklist_source_contract, record_contract_decision};

const S11_ACCEPTANCE: &str =
    include_str!("../../../stages/s11-testing-quality-golden-ci/ACCEPTANCE_PROMPT.md");
const STRICT_CHECKLIST: &str = include_str!("../../../CODEX_STRICT_OPERATION_CHECKLIST.md");

#[test]
fn implementation_acceptance_checklist_source_contract_keeps_strict_sources() {
    record_contract_decision(&implementation_acceptance_checklist_source_contract::contract())
        .expect("recorded");

    assert!(implementation_acceptance_checklist_source_contract::requires_zero_open_p0_p1());
    assert!(
        implementation_acceptance_checklist_source_contract::source_rules()
            .iter()
            .any(|rule| rule.required_gate == "authority_contract_immutable")
    );
    assert!(S11_ACCEPTANCE.contains("P0") || S11_ACCEPTANCE.contains("验收"));
    assert!(STRICT_CHECKLIST.contains("AGENTS.md"));
}
