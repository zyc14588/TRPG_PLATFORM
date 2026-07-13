crate::define_api_realtime_contract_module!(
    "realtime_room_sync",
    "RealtimeRoomSyncContractRecorded",
    "realtime_room_sync.event_schema",
    crate::contract_core::ApiRealtimeOperation::PublishRealtimeDelta
);
