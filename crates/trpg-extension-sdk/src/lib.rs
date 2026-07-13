pub mod adr_0008_plugin_boundaries;
pub mod agent_pack_sdk;
pub mod extension_compatibility_matrix;
pub mod plugin_sdk;
pub mod readme;
pub mod ruleset_pack_sdk;
pub mod sdk;
pub mod tool_provider_sdk;

pub use readme::{
    all_extension_contracts, append_extension_event, contract, is_current_safe_name,
    redact_extension_output, replay_visible_extension_events, CompatibilityResult,
    ExtensionCapability, ExtensionCapabilityGrantSet, ExtensionCommand, ExtensionContract,
    ExtensionEvent, ExtensionEventEnvelope, ExtensionEventRecord, ExtensionEventStore,
    ExtensionExecution, ExtensionExternalContract, ExtensionObservabilityRecord,
    ExtensionOperation, ExtensionPolicyGate, ExtensionSdkError, ExtensionSdkResult,
    SdkCompatibilityReport, EXTENSION_CANON_BOUNDARY, EXTENSION_REDACTED,
    EXTENSION_REQUIRED_COMMAND_FIELDS, FORBIDDEN_CAPABILITIES,
};

pub use trpg_shared_kernel::{
    Actor, ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, EventEnvelope,
    EventStore, FactProvenance, FormalWritePath, KernelResult, PrincipalScope, ProvenanceKind,
    TrpgError, Visibility, VisibilityLabel, WireErrorCode,
};
