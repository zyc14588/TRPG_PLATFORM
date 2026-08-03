mod certification_support;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use certification_support::{execute_certification, CertificationFault};
use trpg_agent_runtime::local_model_certification::{
    CertificationCaseKind, CertificationCaseStatus, CertificationInput, CertificationRequest,
    CertificationRunStatus, LocalModelCertificate, LocalModelCertificationAuthority,
    LocalModelCertificationRunner, LocalModelCertificationSuite, LocalModelLevel,
};
use trpg_agent_runtime::model_provider::{
    ExecutableModelProvider, ProviderCancellation, ProviderType,
};

#[path = "certification_ledger_integrity/checkpoint_store.rs"]
mod checkpoint_store;
use checkpoint_store::TestFileCheckpointStore;

const MODEL: &str = "rf01-model";
const ARTIFACT: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);

fn authority() -> (std::path::PathBuf, LocalModelCertificationAuthority) {
    let root = std::env::temp_dir().join(format!(
        "rf01-local-certification-{}-{}",
        std::process::id(),
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let authority = LocalModelCertificationAuthority::new_with_checkpoint(
        "rf01-test-signing-key",
        &[0x42; 32],
        root.join("registry.jsonl"),
        TestFileCheckpointStore::shared(root.join("registry.external-witness")),
    )
    .unwrap();
    (root, authority)
}

#[test]
#[allow(deprecated)]
fn forged_all_true_assessment_cannot_issue_a_level4_certificate() {
    let (root, authority) = authority();
    let forged = CertificationInput {
        model_id: MODEL.to_owned(),
        json_schema_support: true,
        tool_call_support: true,
        visibility_tests_pass: true,
        prompt_injection_tests_pass: true,
        rules_eval_pass: true,
        latency_ms: 1,
    };

    assert!(authority
        .issue_level4(&forged, ARTIFACT, "forged-suite", Duration::from_secs(60))
        .is_err());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn eight_case_fake_provider_transcript_is_required_for_level4() {
    let outcome = execute_certification(
        MODEL,
        ARTIFACT,
        CertificationFault::None,
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    assert_eq!(outcome.provider.chat_calls(), 9);
    assert_eq!(outcome.run.level(), LocalModelLevel::Level4);
    assert_eq!(
        outcome.run.manifest().status(),
        CertificationRunStatus::Passed
    );
    assert_eq!(outcome.run.manifest().cases().len(), 8);
    for expected in CertificationCaseKind::ALL {
        let evidence = outcome
            .run
            .manifest()
            .cases()
            .iter()
            .find(|case| case.kind() == expected)
            .unwrap();
        assert_eq!(evidence.status(), CertificationCaseStatus::Pass);
        assert!(evidence.request_sha256().starts_with("sha256:"));
        assert!(evidence.response_sha256().unwrap().starts_with("sha256:"));
        assert!(evidence.error_code().is_none());
    }
    let encoded = std::str::from_utf8(outcome.run.canonical_manifest()).unwrap();
    assert!(encoded.contains("\"suite_version\": \"1.0.1\""));
    assert!(encoded.contains("\"redacted_request\""));
    assert!(encoded.contains("\"redacted_response\""));

    let (root, authority) = authority();
    let certificate = authority
        .issue_level4_from_run(&outcome.run, Duration::from_secs(60))
        .unwrap();
    assert_eq!(
        certificate.certification_binding().evidence_sha256(),
        outcome.run.evidence_sha256()
    );
    authority
        .ensure_ai_keeper_provider(&certificate, outcome.provider.as_ref())
        .unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn repeated_issuance_reuses_the_active_certificate_without_registry_mutation() {
    let run = certification_support::passing_run_blocking(MODEL, ARTIFACT);
    let (root, authority) = authority();

    let first = authority
        .issue_level4_from_run(&run, Duration::from_secs(60))
        .unwrap();
    let registry_before = fs::read(root.join("registry.jsonl")).unwrap();
    let repeated = authority
        .issue_level4_from_run(&run, Duration::from_secs(60))
        .unwrap();

    assert_eq!(repeated, first);
    assert_eq!(
        fs::read(root.join("registry.jsonl")).unwrap(),
        registry_before
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn caller_supplied_runtime_identity_cannot_be_certified() {
    let suite = LocalModelCertificationSuite::keeper_v1();
    let provider = std::sync::Arc::new(
        certification_support::DeterministicCertificationProvider::new(
            MODEL,
            ARTIFACT,
            CertificationFault::None,
        ),
    );
    let forged_runtime = format!("sha256:{}", "c".repeat(64));
    let request = CertificationRequest::new(
        "rf01-forged-runtime-request",
        MODEL,
        ARTIFACT,
        provider.provider_id().as_str(),
        provider.provider_type(),
        forged_runtime,
        suite.suite_id(),
        suite.suite_version(),
    )
    .unwrap();
    let runner =
        LocalModelCertificationRunner::new(provider, suite, Duration::from_secs(1)).unwrap();

    assert!(runner
        .run(&request, &ProviderCancellation::default())
        .await
        .is_err());
}

#[tokio::test]
async fn evidence_manifest_preserves_safe_redacted_transcript_content() {
    let outcome = execute_certification(
        MODEL,
        ARTIFACT,
        CertificationFault::None,
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    let encoded = std::str::from_utf8(outcome.run.canonical_manifest()).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(encoded).unwrap();
    let golden = manifest["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["kind"] == "golden")
        .unwrap();

    assert!(golden["redacted_request"]
        .as_str()
        .unwrap()
        .contains("certification_case:golden"));
    assert!(golden["redacted_response"]
        .as_str()
        .unwrap()
        .contains("request_skill_check"));
    assert!(encoded.contains("[REDACTED_PRIVATE_CONTENT]"));
    assert!(!encoded.contains("[REDACTED_MODEL_REQUEST"));
    assert!(!encoded.contains("[REDACTED_MODEL_TRANSCRIPT"));
    assert!(!encoded.contains("KEEPER_ONLY_CANARY_RF01"));
}

#[tokio::test]
async fn model_suite_runtime_and_evidence_tampering_invalidates_certificate() {
    let outcome = execute_certification(
        MODEL,
        ARTIFACT,
        CertificationFault::None,
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    let (root, authority) = authority();
    let certificate = authority
        .issue_level4_from_run(&outcome.run, Duration::from_secs(60))
        .unwrap();

    for mutation in [
        "model_id",
        "model_artifact",
        "suite_id",
        "suite_version",
        "provider_id",
        "provider_type",
        "provider_runtime",
        "suite_hash",
        "prompt_hash",
        "tool_schema_hash",
        "ruleset_hash",
        "policy_hash",
        "evidence_hash",
    ] {
        let mut encoded = serde_json::to_value(&certificate).unwrap();
        match mutation {
            "model_id" => encoded["model_id"] = serde_json::json!("rf01-model-tampered"),
            "model_artifact" => {
                encoded["model_artifact_sha256"] =
                    serde_json::json!(format!("sha256:{}", "0".repeat(64)))
            }
            "suite_id" => encoded["suite_id"] = serde_json::json!("tampered-suite"),
            "suite_version" => {
                encoded["certification_binding"]["suite_version"] = serde_json::json!("9.9.9")
            }
            "provider_id" => {
                encoded["certification_binding"]["provider_id"] =
                    serde_json::json!("rf01_fake_provider_tampered")
            }
            "provider_type" => {
                encoded["certification_binding"]["provider_type"] = serde_json::json!("llama_cpp")
            }
            "provider_runtime" => {
                encoded["certification_binding"]["provider_runtime_sha256"] =
                    serde_json::json!(format!("sha256:{}", "c".repeat(64)))
            }
            hash_field => {
                let field = match hash_field {
                    "suite_hash" => "suite_sha256",
                    "prompt_hash" => "prompt_set_sha256",
                    "tool_schema_hash" => "tool_schema_sha256",
                    "ruleset_hash" => "ruleset_sha256",
                    "policy_hash" => "policy_sha256",
                    "evidence_hash" => "evidence_sha256",
                    _ => unreachable!(),
                };
                encoded["certification_binding"][field] =
                    serde_json::json!(format!("sha256:{}", "d".repeat(64)));
            }
        }
        let tampered: LocalModelCertificate = serde_json::from_value(encoded).unwrap();
        assert!(authority
            .ensure_ai_keeper_provider(&tampered, outcome.provider.as_ref())
            .is_err());
    }

    authority
        .ensure_ai_keeper_provider(&certificate, outcome.provider.as_ref())
        .unwrap();
    let drifted_runtime = format!("sha256:{}", "c".repeat(64));
    let drifted_provider =
        certification_support::DeterministicCertificationProvider::new_with_identity(
            "rf01_fake_provider",
            ProviderType::Ollama,
            MODEL,
            ARTIFACT,
            &drifted_runtime,
            CertificationFault::None,
        );
    assert!(authority
        .ensure_ai_keeper_provider(&certificate, &drifted_provider)
        .is_err());

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn injection_visibility_and_tool_instability_fail_closed() {
    for (fault, expected_error) in [
        (
            CertificationFault::PromptInjection,
            "prompt_injection_resistance_failed",
        ),
        (
            CertificationFault::VisibilityLeakage,
            "visibility_leakage_detected",
        ),
        (
            CertificationFault::ToolInstability,
            "tool_use_instability_detected",
        ),
    ] {
        let run = execute_certification(MODEL, ARTIFACT, fault, Duration::from_secs(1))
            .await
            .unwrap()
            .run;
        assert_eq!(run.level(), LocalModelLevel::Level3);
        assert_eq!(run.manifest().status(), CertificationRunStatus::Failed);
        assert!(run
            .manifest()
            .cases()
            .iter()
            .any(|case| case.error_code() == Some(expected_error)));
        assert!(!std::str::from_utf8(run.canonical_manifest())
            .unwrap()
            .contains("KEEPER_ONLY_CANARY_RF01"));
        assert!(!format!("{run:?}").contains("KEEPER_ONLY_CANARY_RF01"));
        let (root, authority) = authority();
        assert!(authority
            .issue_level4_from_run(&run, Duration::from_secs(60))
            .is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

#[tokio::test]
async fn timeout_and_partial_execution_cannot_reach_level4() {
    let timed_out = execute_certification(
        MODEL,
        ARTIFACT,
        CertificationFault::Timeout,
        Duration::from_millis(5),
    )
    .await
    .unwrap()
    .run;
    assert_eq!(
        timed_out.manifest().status(),
        CertificationRunStatus::Failed
    );
    assert!(timed_out
        .manifest()
        .cases()
        .iter()
        .any(|case| case.error_code() == Some("certification_case_timeout")));
    let capability_timeout = timed_out
        .manifest()
        .cases()
        .iter()
        .find(|case| case.kind() == CertificationCaseKind::CapabilityProbe)
        .unwrap();
    assert_eq!(capability_timeout.retry_count(), 1);

    let suite = LocalModelCertificationSuite::keeper_v1();
    let provider = std::sync::Arc::new(
        certification_support::DeterministicCertificationProvider::new(
            MODEL,
            ARTIFACT,
            CertificationFault::None,
        ),
    );
    let request = CertificationRequest::new(
        "rf01-cancelled-request",
        MODEL,
        ARTIFACT,
        provider.provider_id().as_str(),
        ProviderType::Ollama,
        certification_support::RUNTIME_SHA256,
        suite.suite_id(),
        suite.suite_version(),
    )
    .unwrap();
    let runner =
        LocalModelCertificationRunner::new(provider, suite, Duration::from_secs(1)).unwrap();
    let cancellation = ProviderCancellation::default();
    cancellation.cancel();
    let partial = runner.run(&request, &cancellation).await.unwrap();
    assert_eq!(partial.manifest().status(), CertificationRunStatus::Partial);
    assert!(partial
        .manifest()
        .cases()
        .iter()
        .all(|case| case.status() == CertificationCaseStatus::NotRun));

    let (root, authority) = authority();
    assert!(authority
        .issue_level4_from_run(&timed_out, Duration::from_secs(60))
        .is_err());
    assert!(authority
        .issue_level4_from_run(&partial, Duration::from_secs(60))
        .is_err());
    fs::remove_dir_all(root).unwrap();
}
