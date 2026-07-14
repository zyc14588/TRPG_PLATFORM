crate::define_api_realtime_contract_module!(
    "websocket_protocol",
    "WebsocketProtocolContractRecorded",
    "websocket_protocol.event_schema",
    crate::contract_core::ApiRealtimeOperation::PublishRealtimeDelta
);
