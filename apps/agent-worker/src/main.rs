#![forbid(unsafe_code)]

use trpg_agent_runtime::agent_runtime::AgentError;
use trpg_agent_runtime::model_provider::{
    evaluate_cloud_fallback, provider_boundary_snapshot, FallbackPolicy, ProviderType,
};
use trpg_service_runtime::{serve, Check, Readiness, ServiceConfig};

fn build_readiness(
    boundary: &trpg_agent_runtime::model_provider::ModelProviderBoundarySnapshot,
    silent_fallback_rejected: bool,
) -> Readiness {
    let boundary_valid = !boundary.gateway.is_empty()
        && !boundary.runtime.is_empty()
        && !boundary.provider_adapter.is_empty();
    Readiness::new(vec![
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
    ])
}

fn main() {
    let boundary = provider_boundary_snapshot();
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
    let readiness = build_readiness(&boundary, silent_fallback_rejected);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_provider_boundary_is_not_ready() {
        let mut boundary = provider_boundary_snapshot();
        boundary.gateway = "";
        assert!(!build_readiness(&boundary, true).is_ready());
    }
}
