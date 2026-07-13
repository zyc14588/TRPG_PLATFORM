crate::define_api_realtime_contract_module!(
    "request_idempotency_contract",
    "RequestIdempotencyContractRecorded",
    "request_idempotency_contract.event_schema",
    crate::contract_core::ApiRealtimeOperation::DispatchCommand
);
