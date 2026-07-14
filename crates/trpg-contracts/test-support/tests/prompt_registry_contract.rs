use trpg_test_support::{
    assert_normalized_prompt_binding, normalized_product_modules, normalized_prompt_bindings,
    normalized_prompt_id,
};

#[test]
fn construction_metadata_is_read_from_the_authoritative_normalized_map() {
    assert_normalized_prompt_binding(
        "trpg-agent-runtime",
        "agent_context_assembler",
        "CODEX-0041-04-AI-AGENT-SYSTEM-570f17da9d",
    );
    assert!(normalized_prompt_bindings().len() > 1_000);
    assert_eq!(
        normalized_prompt_id("trpg-agent-runtime", "agent_context_assembler"),
        "CODEX-0041-04-AI-AGENT-SYSTEM-570f17da9d"
    );
    assert!(normalized_product_modules("trpg-runtime")
        .iter()
        .any(|module| module.ends_with("::session_runtime")));
}
