use trpg_agent_runtime::local_model_certification::LocalModelLevel;
use trpg_agent_runtime::model_provider::{
    Environment, FallbackDecision, FallbackPolicy, ProviderConfig, ProviderType,
};
use trpg_agent_runtime::model_provider_local_cloud_impl;

fn local_dev_config() -> ProviderConfig {
    ProviderConfig {
        provider_type: ProviderType::Ollama,
        base_url: "http://127.0.0.1:11434/v1".to_owned(),
        api_key: "ollama-dev".to_owned(),
        environment: Environment::Dev,
        reverse_proxy_auth: false,
    }
}

#[test]
fn model_provider_local_cloud_impl_requires_level4_for_ai_keeper() {
    assert_eq!(
        model_provider_local_cloud_impl::PROMPT_ID,
        "CODEX-0484-04-AI-AGENT-SYSTEM-e96dc3868d"
    );
    let error = model_provider_local_cloud_impl::evaluate_provider_route_for_ai_keeper(
        &local_dev_config(),
        ProviderType::Ollama,
        ProviderType::Ollama,
        FallbackPolicy {
            cloud_fallback_enabled: false,
            user_notice: false,
            snapshot_recorded: false,
        },
        LocalModelLevel::Level3,
    )
    .unwrap_err();

    assert_eq!(error.code(), "LOCAL_MODEL_NOT_CERTIFIED_FOR_AI_KP");
}

#[test]
fn model_provider_local_cloud_impl_blocks_silent_local_to_cloud_fallback() {
    let error = model_provider_local_cloud_impl::evaluate_provider_route_for_ai_keeper(
        &local_dev_config(),
        ProviderType::Ollama,
        ProviderType::Cloud,
        FallbackPolicy {
            cloud_fallback_enabled: false,
            user_notice: false,
            snapshot_recorded: false,
        },
        LocalModelLevel::Level4,
    )
    .unwrap_err();

    assert_eq!(error.code(), "SILENT_FALLBACK_FORBIDDEN");
}

#[test]
fn model_provider_local_cloud_impl_accepts_explicit_audited_route() {
    let evaluation = model_provider_local_cloud_impl::evaluate_provider_route_for_ai_keeper(
        &local_dev_config(),
        ProviderType::Ollama,
        ProviderType::Cloud,
        FallbackPolicy {
            cloud_fallback_enabled: true,
            user_notice: true,
            snapshot_recorded: true,
        },
        LocalModelLevel::Level4,
    )
    .unwrap();

    assert_eq!(evaluation.fallback, FallbackDecision::Allow);
    assert!(evaluation.ai_keeper_allowed);
    assert_eq!(evaluation.boundary.gateway, "Agent Gateway");
    assert_eq!(
        evaluation.boundary.provider_adapter,
        "Model Provider Adapter"
    );
}
