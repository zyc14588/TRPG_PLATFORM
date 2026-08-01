#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateCharacterRequest {
    pub character_id: String,
    pub campaign_id: String,
    pub owner_user_id: String,
    pub display_name: String,
    pub sheet_version_id: String,
    pub sheet_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinCharacterSessionRequest {
    pub join_id: String,
    pub campaign_id: String,
    pub session_id: String,
    pub character_id: String,
    pub owner_user_id: String,
    pub joined_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestCampaignExportRequest {
    pub export_id: String,
    pub campaign_id: String,
    pub requested_by: String,
    pub audience: String,
    pub requested_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignProjection {
    pub campaign_id: String,
    pub owner_user_id: String,
    pub authority_contract_id: String,
    pub title: String,
    pub state: String,
    pub aggregate_version: i64,
    pub last_event_sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignExportProjection {
    pub export_id: String,
    pub campaign_id: String,
    pub requested_by: String,
    pub audience: String,
    pub state: String,
    pub attempt_count: i16,
    pub max_attempts: i16,
    pub failure_code: Option<String>,
    pub artifact_schema: String,
    pub visibility_policy_version: String,
    pub artifact_hash: Option<String>,
    pub manifest_hash: Option<String>,
    pub artifact_size: Option<i64>,
    pub first_event_sequence: Option<i64>,
    pub last_exported_event_sequence: Option<i64>,
    pub event_count: Option<i64>,
    pub retention_expires_at_unix_ms: Option<i64>,
    pub fork_id: Option<String>,
    pub parent_campaign_id: Option<String>,
    pub source_session_id: Option<String>,
    pub source_snapshot_hash: Option<String>,
    pub child_snapshot_hash: Option<String>,
    pub aggregate_version: i64,
    pub last_event_sequence: i64,
}
