#![forbid(unsafe_code)]

use trpg_agent_runtime::agent_runtime::AgentError;
use trpg_agent_runtime::model_provider::{
    evaluate_cloud_fallback, provider_boundary_snapshot, FallbackPolicy, ProviderType,
};
use trpg_service_runtime::{serve, Check, Readiness, ServiceConfig};

fn main() {
    let boundary = provider_boundary_snapshot();
    let boundary_valid = !boundary.gateway.is_empty()
        && !boundary.runtime.is_empty()
        && !boundary.provider_adapter.is_empty();
    let silent_fallback_rejected = matches!(
        evaluate_cloud_fallback(
            ProviderType::Ollama,
            ProviderType::Cloud,
            FallbackPolicy {
                cloud_fallback_enabled: false,
                user_notice: false,
                snapshot_recorded: false,
            },
        ),
        Err(AgentError::SilentFallbackForbidden)
    );
    let readiness = Readiness::new(vec![
        if boundary_valid {
            Check::pass("agent_provider_boundary", "gateway route loaded")
        } else {
            Check::fail("agent_provider_boundary", "provider boundary is incomplete")
        },
        if silent_fallback_rejected {
            Check::pass("silent_fallback_gate", "unsafe cloud fallback rejected")
        } else {
            Check::fail("silent_fallback_gate", "unsafe cloud fallback was accepted")
        },
    ]);

    if let Err(error) = serve(ServiceConfig {
        service: "agent-worker",
        version: env!("CARGO_PKG_VERSION"),
        default_bind_addr: "127.0.0.1:8082",
        readiness,
    }) {
        eprintln!("agent-worker failed: {error}");
        std::process::exit(1);
    }
}
