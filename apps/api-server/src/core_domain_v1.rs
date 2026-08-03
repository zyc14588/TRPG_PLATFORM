use trpg_api::api_contracts::{
    CampaignApiResponse, CampaignExportApiResponse,
    CampaignExportDownloadAuthorizationApiResponse, CampaignExportDownloadDescriptor,
    ChangeSessionStateApiRequest,
    ForkCampaignApiRequest, ImportScenarioApiRequest, JoinCharacterSessionApiRequest,
    ReconsiderationOutcomeApiRequest, RequestCampaignExportApiRequest,
    RequestReconsiderationApiRequest, ResolveReconsiderationApiRequest,
    ReviewReconsiderationApiRequest, StartSessionApiRequest, SwitchSceneApiRequest,
    UpdateCharacterApiRequest, V1LifecyclePort,
};
use trpg_data_eventing::persistence_postgresql::{
    ImportScenarioRequest, JoinCharacterSessionRequest, RecordCampaignForkRequest,
    CampaignExportDownloadArtifact,
    LifecycleReconsiderationOutcome as ReconsiderationOutcome,
    LifecycleSessionState as SessionState,
    RequestCampaignExportRequest, RequestReconsiderationRequest, ResolveReconsiderationRequest,
    ReviewReconsiderationRequest, StartSessionRequest, SwitchSceneRequest,
    UpdateCharacterRequest,
};

include!("core_domain_v1/01_character_session_and_reconsideration.rs");
include!("core_domain_v1/02_fork_export_and_campaign_queries.rs");
include!("core_domain_v1/03_export_download_authorization.rs");

impl V1LifecyclePort for RepositoryCampaignCharacterPort {
    lifecycle_character_session_methods!();
    lifecycle_fork_export_query_methods!();
    lifecycle_export_download_methods!();
}
