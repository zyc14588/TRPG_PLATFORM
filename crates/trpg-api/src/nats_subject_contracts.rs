crate::define_api_realtime_contract_module!(
    "nats_subject_contracts",
    "NatsSubjectContractRecorded",
    "nats_subject_contracts.event_schema",
    crate::contract_core::ApiRealtimeOperation::RegisterSchema
);

pub fn validate_subject(subject: &str) -> trpg_shared_kernel::KernelResult<()> {
    crate::contract_core::validate_nats_subject(subject)
}
