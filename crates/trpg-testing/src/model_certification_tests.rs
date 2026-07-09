use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_agent_runtime::{
    certify_local_model, ensure_ai_keeper_model, evaluate_cloud_fallback, CertificationInput,
    FallbackPolicy, LocalModelLevel, ProviderType,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0091-10-TESTING-QUALITY-6730499fe0";
pub const MODULE: &str = "testing_quality::model_certification_tests";

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/model_certification_tests.rs",
        "crates/trpg-testing/tests/model_certification_tests_contract_tests.rs",
        TestingQualityAction::VerifyModelCertification,
        &[
            "test-data/provider_model_certification_cases.md",
            "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
        ],
        &[
            "local_model_level_4_required_for_ai_keeper",
            "silent_local_to_cloud_fallback_denied",
            "provider_boundary_uses_agent_gateway",
        ],
    )
}

pub fn certified_local_model() -> CertificationInput {
    CertificationInput {
        model_id: "json-tool-stable".to_owned(),
        json_schema_support: true,
        tool_call_support: true,
        visibility_tests_pass: true,
        prompt_injection_tests_pass: true,
        rules_eval_pass: true,
        latency_ms: 1_800,
    }
}

pub fn uncertified_local_model() -> CertificationInput {
    CertificationInput {
        model_id: "unstable-chat".to_owned(),
        json_schema_support: false,
        tool_call_support: false,
        visibility_tests_pass: false,
        prompt_injection_tests_pass: false,
        rules_eval_pass: false,
        latency_ms: 5_000,
    }
}

pub fn level4_is_required_for_ai_keeper() -> bool {
    let level = certify_local_model(&certified_local_model());
    let weak_level = certify_local_model(&uncertified_local_model());

    level == LocalModelLevel::Level4
        && ensure_ai_keeper_model(level).is_ok()
        && ensure_ai_keeper_model(weak_level).is_err()
}

pub fn silent_cloud_fallback_is_denied() -> bool {
    evaluate_cloud_fallback(
        ProviderType::Ollama,
        ProviderType::Cloud,
        FallbackPolicy {
            cloud_fallback_enabled: false,
            user_notice: false,
            snapshot_recorded: false,
        },
    )
    .is_err()
}

pub fn explicit_cloud_fallback_is_allowed() -> bool {
    evaluate_cloud_fallback(
        ProviderType::Ollama,
        ProviderType::Cloud,
        FallbackPolicy {
            cloud_fallback_enabled: true,
            user_notice: true,
            snapshot_recorded: true,
        },
    )
    .is_ok()
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
