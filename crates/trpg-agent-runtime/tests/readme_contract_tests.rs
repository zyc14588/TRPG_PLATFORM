use trpg_agent_runtime::readme;

#[test]
fn readme_snapshot_keeps_gateway_runtime_provider_boundary() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "readme"),
        "CODEX-0475-04-AI-AGENT-SYSTEM-2a3840db15"
    );

    let snapshot = readme::readme_governance_snapshot();

    assert_eq!(snapshot.ai_entrypoint, "Agent Gateway");
    assert_eq!(snapshot.runtime_boundary, "Agent Orchestrator/Runtime");
    assert_eq!(snapshot.provider_adapter, "Model Provider Adapter");
    assert_eq!(
        snapshot.forbidden_direct_call_error,
        "DIRECT_LLM_CALL_FORBIDDEN"
    );
    assert!(snapshot.formal_state_policy.contains("Event Store"));
    assert!(snapshot.visibility_policy.contains("RAG"));
    assert!(snapshot.fact_provenance_policy.contains("event envelope"));
}
