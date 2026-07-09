use trpg_testing::{principle_to_doc_trace, record_contract_decision};

const TESTING_README: &str = include_str!("../../../docs/codex/10-testing-quality/README.md");
const CONTRACT_MATRIX: &str =
    include_str!("../../../docs/codex/10-testing-quality/contract_test_matrix.md");

#[test]
fn principle_to_doc_trace_points_to_current_docs_without_new_scope() {
    record_contract_decision(&principle_to_doc_trace::contract()).expect("recorded");

    assert!(principle_to_doc_trace::links_current_doc(
        "docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md"
    ));
    assert!(principle_to_doc_trace::links_current_doc(
        "docs/codex/10-testing-quality/contract_test_matrix.md"
    ));
    assert!(TESTING_README.contains("testing") || TESTING_README.contains("Testing"));
    assert!(CONTRACT_MATRIX.contains("visibility") || CONTRACT_MATRIX.contains("Visibility"));
}
