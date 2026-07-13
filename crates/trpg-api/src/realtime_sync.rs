crate::define_api_realtime_contract_module!(
    "realtime_sync",
    "RealtimeSyncContractRecorded",
    "realtime_sync.event_schema",
    crate::contract_core::ApiRealtimeOperation::PublishRealtimeDelta
);
