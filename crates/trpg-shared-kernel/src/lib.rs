pub mod adr_0001_rust_first;
pub mod cargo_workspace;
pub mod cargo_workspace_impl;
pub mod config_model;
pub mod constitution;
pub mod constitution_impl;
pub mod crate_ownership;
pub mod dependency_direction;
pub mod document_set;
pub mod document_set_impl;
pub mod error_model;
pub mod open_source_reference_matrix;
pub mod open_source_reference_matrix_impl;
pub mod readme;
pub mod rust_cargo_workspace;
pub mod rust_coding_model;
pub mod server_random;
pub mod shared_kernel;
pub mod system_context;
pub mod system_context_impl;
pub mod technology_selection_rust;
pub mod technology_selection_rust_impl;
pub mod workspace_and_governance;

pub use server_random::{
    server_d10_roll, server_damage_roll, server_percentile_roll, ServerD10Roll, ServerDamageRoll,
    ServerGrowthRollEvidence, ServerPercentileRoll,
};
pub use shared_kernel::{
    validate_command_envelope, Actor, ActorOrigin, ActorRole, AgentClass,
    AuthenticatedCommandContext, AuthorityBinding, AuthorityContract, AuthorityContractDraft,
    AuthorityMode, AuthorityRegistry, AuthorityVersionSnapshot, AuthorityVersionSnapshotDraft,
    CanonicalCommitEvent, CanonicalCommitKey, CanonicalCommitPort, CanonicalCommitReceipt,
    CanonicalCommitRequest, CanonicalCommittedEvent, CanonicalPolicyAudit, ChangePolicy,
    CommandEnvelope, CommandMetadata, EntityId, EventActorOriginWire, EventEnvelope,
    EventEnvelopeWire, EventStore, FactProvenance, FormalWritePath, KernelContractSnapshot,
    KernelResult, PrincipalCapability, PrincipalClaims, PrincipalScope, ProvenanceKind,
    ResourceRef, TrpgError, Visibility, VisibilityKind, VisibilityLabel, WorkloadRole,
    EVENT_ENVELOPE_WIRE_SCHEMA_VERSION,
};
pub use trpg_contracts::WireErrorCode;
