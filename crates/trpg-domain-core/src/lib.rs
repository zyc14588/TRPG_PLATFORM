pub mod adr_0003_authority_contract;
pub mod adr_0003_authority_contract_authority_contract;
pub mod authority_contract;
pub mod authority_contract_guard;
pub mod authority_contract_impl;
pub mod character_combat_san_chase;
pub mod character_combat_san_chase_impl {
    pub use crate::character_combat_san_chase::*;
}
pub mod command_authority_visibility;
pub mod command_cqrs;
pub mod command_cqrs_idempotency;
pub mod command_cqrs_impl;
pub mod ddd;
pub mod decision_record_model;
pub mod domain_command_cqrs;
pub mod domain_entities_value_objects;
pub mod domain_event_sourcing_projection;
pub mod domain_model;
pub mod domain_model_impl;
pub mod domain_policy_hooks;
pub mod domain_visibility_fact_provenance;
pub mod event_sourcing_projection;
pub mod event_sourcing_projection_impl;
pub mod event_sourcing_snapshot_projection;
pub mod fork_canon_lineage;
pub mod investigation_clue_npc_time;
pub mod investigation_clue_npc_time_impl {
    pub use crate::investigation_clue_npc_time::*;
}
pub mod openfga_opa_visibility;
pub mod readme;
pub mod rule_runtime_coc7;
pub mod rule_runtime_coc7_impl {
    pub use crate::rule_runtime_coc7::*;
}
pub mod visibility_enforcement_points;
pub mod visibility_fact_provenance;
pub mod visibility_fact_provenance_impl;
pub mod visibility_leakage_tests;

pub use ddd::{DomainError, DomainResult, FactSource};

pub const PUBLIC_COMPATIBILITY_MODULES: &[&str] = &[
    "character_combat_san_chase_impl",
    "investigation_clue_npc_time_impl",
    "rule_runtime_coc7_impl",
];
