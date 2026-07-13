crate::define_api_realtime_contract_module!(
    "provider",
    "ProviderContractRecorded",
    "provider.event_schema",
    crate::contract_core::ApiRealtimeOperation::RegisterProviderContract
);

pub fn evaluate_provider_access(
    route: crate::contract_core::ProviderAccessPath,
) -> trpg_shared_kernel::KernelResult<crate::contract_core::ProviderPolicyDecision> {
    crate::contract_core::evaluate_provider_access(route)
}
