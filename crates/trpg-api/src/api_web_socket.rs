crate::define_api_realtime_contract_module!(
    "api_web_socket",
    "ApiWebSocketContractRecorded",
    "api_web_socket.event_schema",
    crate::contract_core::ApiRealtimeOperation::PublishRealtimeDelta
);
