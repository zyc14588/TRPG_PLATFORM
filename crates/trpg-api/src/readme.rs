use crate::contract_core::{
    append_api_contract_event, ApiRealtimeContract, ApiRealtimeEventPayload, ApiRealtimeOperation,
};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult,
};

pub const MODULE_NAME: &str = "readme";
pub const EVENT_TYPE: &str = "ReadmeContractRecorded";
pub const EVENT_SCHEMA_NAME: &str = "readme.event_schema";

pub const README_GOVERNANCE_REQUIREMENTS: &[&str] = &[
    "authority_contract_locked",
    "agent_gateway_only_ai_access",
    "tool_permission_gate_default_deny",
    "visibility_label_propagation",
    "fact_provenance_required",
    "event_store_is_canon",
    "formal_write_path_only",
    "no_private_realtime_leakage",
];

pub fn contract() -> ApiRealtimeContract {
    ApiRealtimeContract::new(
        MODULE_NAME,
        EVENT_TYPE,
        EVENT_SCHEMA_NAME,
        ApiRealtimeOperation::DispatchCommand,
    )
}

pub fn append_contract_event<T>(
    store: &mut EventStore<ApiRealtimeEventPayload>,
    authority: &AuthorityContract,
    command: &CommandEnvelope<T>,
) -> KernelResult<EventEnvelope<ApiRealtimeEventPayload>> {
    append_api_contract_event(store, authority, command, &contract())
}

pub fn readme_governance_requirements() -> &'static [&'static str] {
    README_GOVERNANCE_REQUIREMENTS
}
