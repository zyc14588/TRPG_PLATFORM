use trpg_testing::{record_contract_decision, top_level_principle_trace};

const TOP_LEVEL_DESIGN: &str =
    include_str!("../../../docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md");

#[test]
fn top_level_principle_trace_maps_red_lines_to_tests() {
    record_contract_decision(&top_level_principle_trace::contract()).expect("recorded");

    assert!(top_level_principle_trace::covers_principle(
        "authority_contract_is_immutable"
    ));
    assert!(top_level_principle_trace::covers_principle(
        "agent_gateway_is_required"
    ));
    assert!(top_level_principle_trace::covers_principle(
        "visibility_label_propagates"
    ));
    assert!(TOP_LEVEL_DESIGN.contains("Authority Contract"));
    assert!(TOP_LEVEL_DESIGN.contains("Agent Gateway"));
    assert!(TOP_LEVEL_DESIGN.contains("Visibility"));
}
