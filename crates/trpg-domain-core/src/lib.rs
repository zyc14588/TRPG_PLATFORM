pub mod adr_0003_authority_contract_authority_contract;
pub mod authority_contract;
pub mod authority_contract_guard;
pub mod command_authority_visibility;
pub mod command_cqrs;
pub mod command_cqrs_idempotency;
pub mod ddd;
pub mod decision_record_model;
pub mod domain_entities_value_objects;
pub mod domain_policy_hooks;
pub mod event_sourcing_snapshot_projection;
pub mod fork_canon_lineage;
pub mod visibility_fact_provenance;

pub use ddd::{DomainError, DomainResult, FactSource};
