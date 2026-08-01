#[path = "../../trpg-agent-runtime/tests/certification_support.rs"]
mod certification_support;

use std::time::Duration;

use certification_support::{execute_certification, CertificationFault};
use trpg_agent_runtime::{CertificationCaseKind, CertificationRunStatus, LocalModelLevel};
use trpg_testing::model_certification_tests;

const MODEL_CERTIFICATION_FIXTURE: &str =
    include_str!("../../../test-data/provider_model_certification_cases.md");

#[tokio::test]
async fn model_certification_stage_gate() {
    assert!(MODEL_CERTIFICATION_FIXTURE.contains("LOCAL_MODEL_LEVEL_4"));
    assert!(MODEL_CERTIFICATION_FIXTURE.contains("DENY_AND_AUDIT"));
    assert!(!MODEL_CERTIFICATION_FIXTURE.contains("json_schema_support"));
    for case in [
        "capability_probe",
        "golden",
        "tool_use_stability",
        "visibility_leakage",
        "prompt_injection",
        "coc_rules_mini_eval",
        "latency",
        "context_stress",
    ] {
        assert!(MODEL_CERTIFICATION_FIXTURE.contains(case));
    }
    for binding in [
        "provider_runtime_sha256",
        "suite_sha256",
        "prompt_set_sha256",
        "tool_schema_sha256",
        "ruleset_sha256",
        "policy_sha256",
        "evidence_sha256",
    ] {
        assert!(MODEL_CERTIFICATION_FIXTURE.contains(binding));
    }
    assert!(model_certification_tests::level4_is_required_for_ai_keeper());
    assert!(model_certification_tests::silent_cloud_fallback_is_denied());
    assert!(model_certification_tests::explicit_cloud_fallback_is_allowed().await);

    let artifact = format!("sha256:{}", "1".repeat(64));
    let passing = execute_certification(
        "json-tool-stable",
        &artifact,
        CertificationFault::None,
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    assert_eq!(passing.provider.chat_calls(), 9);
    assert_eq!(passing.run.level(), LocalModelLevel::Level4);
    assert_eq!(
        passing.run.manifest().status(),
        CertificationRunStatus::Passed
    );
    assert_eq!(
        passing.run.manifest().cases().len(),
        CertificationCaseKind::ALL.len()
    );

    for fault in [
        CertificationFault::PromptInjection,
        CertificationFault::VisibilityLeakage,
        CertificationFault::ToolInstability,
    ] {
        let failed =
            execute_certification("json-tool-stable", &artifact, fault, Duration::from_secs(1))
                .await
                .unwrap();
        assert_eq!(failed.run.level(), LocalModelLevel::Level3);
        assert_eq!(
            failed.run.manifest().status(),
            CertificationRunStatus::Failed
        );
    }
}
