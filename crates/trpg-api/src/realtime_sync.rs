crate::define_api_realtime_contract_module!(
    "realtime_sync",
    "RealtimeSyncContractRecorded",
    "realtime_sync.event_schema",
    crate::contract_core::ApiRealtimeOperation::PublishRealtimeDelta
);

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealtimeEvent {
    pub cursor: u64,
    pub stream_version: u64,
    pub event_type: String,
    pub event_schema_version: u32,
    pub campaign_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub authority_mode: String,
    pub authority_epoch: u64,
    pub visibility_label: String,
    #[serde(default)]
    pub visibility_subject: Option<String>,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub trace_id: String,
    pub payload: Value,
}

impl RealtimeEvent {
    pub fn empty_payload() -> Value {
        Value::Object(serde_json::Map::new())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayBatch {
    pub events: Vec<RealtimeEvent>,
    pub source_cursor: u64,
    pub latest_cursor: u64,
    pub has_more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResyncRequired {
    pub reason: &'static str,
    pub earliest_cursor: u64,
    pub latest_cursor: u64,
}
