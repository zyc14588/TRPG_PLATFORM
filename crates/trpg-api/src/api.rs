crate::define_api_realtime_contract_module!(
    "api",
    "ApiContractRecorded",
    "api.event_schema",
    crate::contract_core::ApiRealtimeOperation::DispatchCommand
);
