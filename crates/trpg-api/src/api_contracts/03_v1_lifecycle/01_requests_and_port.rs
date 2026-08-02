#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct UpdateCharacterApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub character_id: String,
    pub owner_user_id: String,
    pub display_name: String,
    pub sheet_version_id: String,
    pub sheet_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct JoinCharacterSessionApiRequest {
    pub command: ApiCommandFields,
    pub join_id: String,
    pub campaign_id: String,
    pub session_id: String,
    pub character_id: String,
    pub owner_user_id: String,
    pub joined_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ImportScenarioApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub scenario_id: String,
    pub ruleset_id: String,
    pub format_version: String,
    pub content_hash: String,
    pub document_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct StartSessionApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub session_id: String,
    pub room_id: String,
    pub scenario_id: String,
    pub scene_id: String,
    pub scene_key: String,
    pub scene_name: String,
    pub started_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionStateApiRequest {
    Active,
    Paused,
    Ended,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ChangeSessionStateApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub session_id: String,
    pub state: SessionStateApiRequest,
    pub changed_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct SwitchSceneApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub session_id: String,
    pub next_scene_id: String,
    pub next_scene_key: String,
    pub next_scene_name: String,
    pub switched_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum PublicGameplayActionApiRequest {
    NpcInteraction {
        character_id: String,
        npc_id: String,
        approach: String,
        public_response: String,
    },
    CombatRound {
        character_id: String,
        npc_id: String,
        action_kind: String,
        defense: String,
    },
    ChaseSegment {
        character_id: String,
        npc_id: String,
        initial_range: i8,
        obstacle_id: Option<String>,
        obstacle_cost: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitPublicGameplayActionApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub session_id: String,
    pub action_id: String,
    pub action: PublicGameplayActionApiRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct RequestReconsiderationApiRequest {
    pub command: ApiCommandFields,
    pub reconsideration_id: String,
    pub campaign_id: String,
    pub original_event_sequence: i64,
    pub requested_by: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ReviewReconsiderationApiRequest {
    pub command: ApiCommandFields,
    pub reconsideration_id: String,
    pub campaign_id: String,
    pub review_event_id: String,
    pub review_summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReconsiderationOutcomeApiRequest {
    Upheld,
    Corrected,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ResolveReconsiderationApiRequest {
    pub command: ApiCommandFields,
    pub reconsideration_id: String,
    pub campaign_id: String,
    pub resolution_event_id: String,
    pub outcome: ReconsiderationOutcomeApiRequest,
    pub resolution: String,
    pub corrected_event_type: Option<String>,
    pub corrected_payload_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ForkCampaignApiRequest {
    pub command: ApiCommandFields,
    pub fork_id: String,
    pub parent_campaign_id: String,
    pub child_campaign_id: String,
    pub source_session_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct RequestCampaignExportApiRequest {
    pub command: ApiCommandFields,
    pub export_id: String,
    pub campaign_id: String,
    pub requested_by: String,
    pub audience: String,
    pub requested_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CampaignApiResponse {
    pub campaign_id: String,
    pub owner_user_id: String,
    pub authority_contract_id: String,
    pub title: String,
    pub state: String,
    pub aggregate_version: i64,
    pub last_event_sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CampaignExportApiResponse {
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CampaignExportDownloadAuthorizationApiResponse {
    pub token: String,
    pub expires_at_unix_ms: i64,
    pub download_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignExportDownloadDescriptor {
    pub artifact_key: String,
    pub artifact_hash: String,
}

pub trait V1LifecyclePort: CampaignCharacterCommandPort {
    fn update_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a UpdateCharacterApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn join_character_session<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a JoinCharacterSessionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn import_scenario<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ImportScenarioApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn start_session<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a StartSessionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn change_session_state<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ChangeSessionStateApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn switch_scene<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a SwitchSceneApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn request_reconsideration<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a RequestReconsiderationApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn review_reconsideration<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ReviewReconsiderationApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn resolve_reconsideration<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ResolveReconsiderationApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn fork_campaign<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ForkCampaignApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn request_campaign_export<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a RequestCampaignExportApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn list_campaigns<'a>(
        &'a self,
        actor_id: &'a str,
        include_all: bool,
    ) -> CoreApiFuture<'a, Vec<CampaignApiResponse>>;

    fn get_campaign<'a>(
        &'a self,
        actor_id: &'a str,
        include_all: bool,
        campaign_id: &'a str,
    ) -> CoreApiFuture<'a, CampaignApiResponse>;

    fn get_campaign_export<'a>(
        &'a self,
        actor_id: &'a str,
        include_all: bool,
        campaign_id: &'a str,
        export_id: &'a str,
    ) -> CoreApiFuture<'a, CampaignExportApiResponse>;

    fn issue_campaign_export_download<'a>(
        &'a self,
        actor_id: &'a str,
        include_all: bool,
        campaign_id: &'a str,
        export_id: &'a str,
        now_unix_ms: u64,
    ) -> CoreApiFuture<'a, CampaignExportDownloadAuthorizationApiResponse>;

    fn consume_campaign_export_download<'a>(
        &'a self,
        actor_id: &'a str,
        campaign_id: &'a str,
        export_id: &'a str,
        token: &'a str,
        now_unix_ms: u64,
    ) -> CoreApiFuture<'a, CampaignExportDownloadDescriptor>;
}
