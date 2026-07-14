crate::define_api_realtime_contract_module!(
    "api_and_transport",
    "ApiAndTransportContractRecorded",
    "api_and_transport.event_schema",
    crate::contract_core::ApiRealtimeOperation::ValidateTransportCommand
);
