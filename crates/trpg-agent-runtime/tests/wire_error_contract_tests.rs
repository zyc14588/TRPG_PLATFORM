use trpg_agent_runtime::AgentError;
use trpg_contracts::WireErrorCode;
use trpg_shared_kernel::TrpgError;

#[test]
fn every_agent_error_maps_to_the_canonical_wire_registry() {
    let cases = [
        (
            AgentError::Core(TrpgError::PolicyDenied),
            WireErrorCode::PolicyDenied,
        ),
        (
            AgentError::ToolPermissionDenied,
            WireErrorCode::ToolPermissionDenied,
        ),
        (
            AgentError::HumanKpDraftOnly,
            WireErrorCode::HumanKpAiDraftOnly,
        ),
        (
            AgentError::AgentDirectStateWriteForbidden,
            WireErrorCode::AgentDirectStateWriteForbidden,
        ),
        (
            AgentError::DirectLlmCallForbidden,
            WireErrorCode::DirectLlmCallForbidden,
        ),
        (
            AgentError::PromptInjectionDetected,
            WireErrorCode::PromptInjectionDetected,
        ),
        (
            AgentError::LocalModelNotCertifiedForAiKp,
            WireErrorCode::LocalModelNotCertifiedForAiKp,
        ),
        (
            AgentError::SilentFallbackForbidden,
            WireErrorCode::SilentFallbackForbidden,
        ),
        (
            AgentError::UnauthenticatedLocalProviderExposed,
            WireErrorCode::UnauthenticatedLocalProviderExposed,
        ),
        (
            AgentError::RagVisibilityScopeViolation,
            WireErrorCode::RagVisibilityScopeViolation,
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.wire_code(), expected);
        assert_eq!(error.code(), expected.as_str());
    }
}
