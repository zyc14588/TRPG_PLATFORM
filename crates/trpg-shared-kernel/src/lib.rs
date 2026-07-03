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
pub mod shared_kernel;
pub mod system_context;
pub mod system_context_impl;
pub mod technology_selection_rust;
pub mod technology_selection_rust_impl;
pub mod workspace_and_governance;

pub use shared_kernel::{
    Actor, ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, EventEnvelope,
    EventStore, FactProvenance, FormalWritePath, KernelContractSnapshot, KernelResult,
    PrincipalScope, ProvenanceKind, TrpgError, Visibility, VisibilityLabel,
};
