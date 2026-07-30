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
    pub aggregate_version: i64,
    pub last_event_sequence: i64,
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
}

#[derive(Clone)]
pub struct V1LifecycleApi<P> {
    port: Arc<P>,
}

impl<P> V1LifecycleApi<P>
where
    P: V1LifecyclePort,
{
    pub fn new(port: Arc<P>) -> Self {
        Self { port }
    }

    pub async fn create_campaign(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CreateCampaignApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        CampaignCharacterApi::new(Arc::clone(&self.port))
            .create_campaign(context, request)
            .await
    }

    pub async fn issue_invite(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &IssueInviteApiRequest,
    ) -> Result<IssuedInviteApiResponse, CoreApiError> {
        CampaignCharacterApi::new(Arc::clone(&self.port))
            .issue_invite(context, request)
            .await
    }

    pub async fn accept_invite(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &AcceptInviteApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        CampaignCharacterApi::new(Arc::clone(&self.port))
            .accept_invite(context, request)
            .await
    }

    pub async fn create_character(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CreateCharacterApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        CampaignCharacterApi::new(Arc::clone(&self.port))
            .create_character(context, request)
            .await
    }

    pub async fn submit_character(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CharacterTransitionApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        CampaignCharacterApi::new(Arc::clone(&self.port))
            .submit_character(context, request)
            .await
    }

    pub async fn review_character(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CharacterTransitionApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        CampaignCharacterApi::new(Arc::clone(&self.port))
            .review_character(context, request)
            .await
    }

    pub async fn update_character(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &UpdateCharacterApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "character", &request.character_id)?;
        if request.command.expected_version <= 0
            || request.owner_user_id != context.actor_id()
            || request.display_name.trim().is_empty()
        {
            return Err(CoreApiError::InvalidInput("character_update"));
        }
        validate_ids(&[
            &request.campaign_id,
            &request.character_id,
            &request.owner_user_id,
            &request.sheet_version_id,
        ])?;
        let sheet: serde_json::Value = serde_json::from_str(&request.sheet_json)
            .map_err(|_| CoreApiError::InvalidInput("character_sheet"))?;
        if !sheet.is_object() {
            return Err(CoreApiError::InvalidInput("character_sheet"));
        }
        self.port.update_character(context, request).await
    }

    pub async fn join_character_session(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &JoinCharacterSessionApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "session_character", &request.join_id)?;
        if request.command.expected_version != 0
            || request.owner_user_id != context.actor_id()
            || request.joined_at_unix_ms == 0
        {
            return Err(CoreApiError::InvalidInput("character_session_join"));
        }
        validate_ids(&[
            &request.join_id,
            &request.campaign_id,
            &request.session_id,
            &request.character_id,
            &request.owner_user_id,
        ])?;
        self.port.join_character_session(context, request).await
    }

    pub async fn import_scenario(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &ImportScenarioApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "scenario", &request.scenario_id)?;
        context.require_keeper()?;
        if request.command.expected_version != 0
            || request.ruleset_id.trim().is_empty()
            || request.format_version.trim().is_empty()
        {
            return Err(CoreApiError::InvalidInput("scenario_import"));
        }
        validate_ids(&[&request.campaign_id, &request.scenario_id])?;
        let document: serde_json::Value = serde_json::from_str(&request.document_json)
            .map_err(|_| CoreApiError::InvalidInput("scenario_document"))?;
        if !document.is_object() {
            return Err(CoreApiError::InvalidInput("scenario_document"));
        }
        self.port.import_scenario(context, request).await
    }

    pub async fn start_session(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &StartSessionApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "session", &request.session_id)?;
        context.require_keeper()?;
        if request.command.expected_version != 0
            || request.scene_key.trim().is_empty()
            || request.scene_name.trim().is_empty()
            || request.started_at_unix_ms == 0
        {
            return Err(CoreApiError::InvalidInput("session_start"));
        }
        validate_ids(&[
            &request.campaign_id,
            &request.session_id,
            &request.room_id,
            &request.scenario_id,
            &request.scene_id,
        ])?;
        self.port.start_session(context, request).await
    }

    pub async fn change_session_state(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &ChangeSessionStateApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "session", &request.session_id)?;
        context.require_keeper()?;
        if request.command.expected_version <= 0 || request.changed_at_unix_ms == 0 {
            return Err(CoreApiError::InvalidInput("session_state"));
        }
        validate_ids(&[&request.campaign_id, &request.session_id])?;
        self.port.change_session_state(context, request).await
    }

    pub async fn switch_scene(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &SwitchSceneApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "session", &request.session_id)?;
        context.require_keeper()?;
        if request.command.expected_version <= 0
            || request.next_scene_key.trim().is_empty()
            || request.next_scene_name.trim().is_empty()
            || request.switched_at_unix_ms == 0
        {
            return Err(CoreApiError::InvalidInput("scene_switch"));
        }
        validate_ids(&[
            &request.campaign_id,
            &request.session_id,
            &request.next_scene_id,
        ])?;
        self.port.switch_scene(context, request).await
    }

    pub async fn request_reconsideration(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &RequestReconsiderationApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(
            &request.campaign_id,
            "reconsideration",
            &request.reconsideration_id,
        )?;
        if request.command.expected_version != 0
            || request.requested_by != context.actor_id()
            || request.original_event_sequence <= 0
            || request.reason.trim().is_empty()
        {
            return Err(CoreApiError::InvalidInput("reconsideration_request"));
        }
        validate_ids(&[
            &request.campaign_id,
            &request.reconsideration_id,
            &request.requested_by,
        ])?;
        self.port.request_reconsideration(context, request).await
    }

    pub async fn review_reconsideration(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &ReviewReconsiderationApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(
            &request.campaign_id,
            "reconsideration",
            &request.reconsideration_id,
        )?;
        context.require_keeper()?;
        if request.command.expected_version <= 0
            || request.review_summary.trim().is_empty()
        {
            return Err(CoreApiError::InvalidInput("reconsideration_review"));
        }
        validate_ids(&[
            &request.campaign_id,
            &request.reconsideration_id,
            &request.review_event_id,
        ])?;
        self.port.review_reconsideration(context, request).await
    }

    pub async fn resolve_reconsideration(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &ResolveReconsiderationApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(
            &request.campaign_id,
            "reconsideration",
            &request.reconsideration_id,
        )?;
        context.require_keeper()?;
        if request.command.expected_version <= 0 || request.resolution.trim().is_empty() {
            return Err(CoreApiError::InvalidInput("reconsideration_resolution"));
        }
        validate_ids(&[
            &request.campaign_id,
            &request.reconsideration_id,
            &request.resolution_event_id,
        ])?;
        self.port.resolve_reconsideration(context, request).await
    }

    pub async fn fork_campaign(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &ForkCampaignApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(
            &request.child_campaign_id,
            "campaign_fork",
            &request.fork_id,
        )?;
        context.require_keeper()?;
        if request.command.expected_version != 0
            || request.parent_campaign_id == request.child_campaign_id
            || request.reason.trim().is_empty()
        {
            return Err(CoreApiError::InvalidInput("campaign_fork"));
        }
        validate_ids(&[
            &request.fork_id,
            &request.parent_campaign_id,
            &request.child_campaign_id,
            &request.source_session_id,
        ])?;
        self.port.fork_campaign(context, request).await
    }

    pub async fn request_campaign_export(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &RequestCampaignExportApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(
            &request.campaign_id,
            "campaign_export",
            &request.export_id,
        )?;
        context.require_keeper()?;
        if request.command.expected_version != 0
            || request.requested_by != context.actor_id()
            || request.audience != "CAMPAIGN_ARCHIVE"
            || request.requested_at_unix_ms == 0
        {
            return Err(CoreApiError::InvalidInput("campaign_export"));
        }
        validate_ids(&[
            &request.export_id,
            &request.campaign_id,
            &request.requested_by,
        ])?;
        self.port.request_campaign_export(context, request).await
    }

    pub async fn list_campaigns(
        &self,
        actor_id: &str,
        include_all: bool,
    ) -> Result<Vec<CampaignApiResponse>, CoreApiError> {
        EntityId::new(actor_id).map_err(|_| CoreApiError::InvalidAuthorizationContext)?;
        self.port.list_campaigns(actor_id, include_all).await
    }

    pub async fn get_campaign(
        &self,
        actor_id: &str,
        include_all: bool,
        campaign_id: &str,
    ) -> Result<CampaignApiResponse, CoreApiError> {
        validate_ids(&[actor_id, campaign_id])?;
        self.port
            .get_campaign(actor_id, include_all, campaign_id)
            .await
    }

    pub async fn get_campaign_export(
        &self,
        actor_id: &str,
        include_all: bool,
        campaign_id: &str,
        export_id: &str,
    ) -> Result<CampaignExportApiResponse, CoreApiError> {
        validate_ids(&[actor_id, campaign_id, export_id])?;
        self.port
            .get_campaign_export(actor_id, include_all, campaign_id, export_id)
            .await
    }
}

fn validate_ids(values: &[&str]) -> Result<(), CoreApiError> {
    for value in values {
        EntityId::new(*value).map_err(|_| CoreApiError::InvalidInput("entity_id"))?;
    }
    Ok(())
}

/// Machine-readable, compatibility-tested surface for the minimum V1
/// lifecycle. The production handler publishes this exact document at
/// `/api/v1/openapi.json`.
pub fn v1_openapi_document() -> serde_json::Value {
    use serde_json::json;

    let command = |summary: &str| {
        json!({
            "summary": summary,
            "security": [{"bearerAuth": []}],
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {"type": "object"}
                    }
                }
            },
            "responses": {
                "200": {"description": "Canonical event receipt"},
                "201": {"description": "Canonical event receipt"},
                "202": {"description": "Canonical event receipt"},
                "400": {"description": "Invalid command"},
                "401": {"description": "Authentication required"},
                "403": {"description": "Policy denied"},
                "404": {"description": "Resource unavailable or invisible"},
                "409": {"description": "Idempotency or aggregate version conflict"}
            }
        })
    };
    let query = |summary: &str| {
        json!({
            "summary": summary,
            "security": [{"bearerAuth": []}],
            "responses": {
                "200": {"description": "Visible projection"},
                "401": {"description": "Authentication required"},
                "404": {"description": "Resource unavailable or invisible"}
            }
        })
    };
    let path_parameters = |names: &[&str]| {
        serde_json::Value::Array(
            names
                .iter()
                .map(|name| {
                    json!({
                        "name": name,
                        "in": "path",
                        "required": true,
                        "schema": {"type": "string", "minLength": 1}
                    })
                })
                .collect(),
        )
    };

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "COC AI TRPG V1 API",
            "version": "1.0.0"
        },
        "paths": {
            "/api/v1/campaigns": {
                "get": query("List visible Campaigns"),
                "post": command("Create Campaign")
            },
            "/api/v1/campaigns/{campaign_id}": {
                "parameters": path_parameters(&["campaign_id"]),
                "get": query("Get visible Campaign")
            },
            "/api/v1/campaigns/{campaign_id}/invites": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Issue Campaign invite")
            },
            "/api/v1/campaigns/{campaign_id}/invites/{invite_id}/accept": {
                "parameters": path_parameters(&["campaign_id", "invite_id"]),
                "post": command("Accept Campaign invite")
            },
            "/api/v1/campaigns/{campaign_id}/characters": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Create Character")
            },
            "/api/v1/campaigns/{campaign_id}/characters/{character_id}": {
                "parameters": path_parameters(&["campaign_id", "character_id"]),
                "put": command("Update Character draft")
            },
            "/api/v1/campaigns/{campaign_id}/characters/{character_id}/submit": {
                "parameters": path_parameters(&["campaign_id", "character_id"]),
                "post": command("Submit Character")
            },
            "/api/v1/campaigns/{campaign_id}/characters/{character_id}/review": {
                "parameters": path_parameters(&["campaign_id", "character_id"]),
                "post": command("Approve Character")
            },
            "/api/v1/campaigns/{campaign_id}/scenarios/import": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Import Scenario")
            },
            "/api/v1/campaigns/{campaign_id}/sessions": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Start Session and opening Scene")
            },
            "/api/v1/campaigns/{campaign_id}/sessions/{session_id}": {
                "parameters": path_parameters(&["campaign_id", "session_id"]),
                "patch": command("Change Session state")
            },
            "/api/v1/campaigns/{campaign_id}/sessions/{session_id}/scenes": {
                "parameters": path_parameters(&["campaign_id", "session_id"]),
                "post": command("Switch active Scene")
            },
            "/api/v1/campaigns/{campaign_id}/sessions/{session_id}/characters/{character_id}/join": {
                "parameters": path_parameters(&["campaign_id", "session_id", "character_id"]),
                "post": command("Join approved Character to Session")
            },
            "/api/v1/campaigns/{campaign_id}/player-actions": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Submit Player action")
            },
            "/api/v1/campaigns/{campaign_id}/player-actions/{action_id}/confirm": {
                "parameters": path_parameters(&["campaign_id", "action_id"]),
                "post": command("Confirm Player action")
            },
            "/api/v1/campaigns/{campaign_id}/reconsiderations": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Request reconsideration")
            },
            "/api/v1/campaigns/{campaign_id}/reconsiderations/{reconsideration_id}/review": {
                "parameters": path_parameters(&["campaign_id", "reconsideration_id"]),
                "post": command("Review reconsideration")
            },
            "/api/v1/campaigns/{campaign_id}/reconsiderations/{reconsideration_id}/resolve": {
                "parameters": path_parameters(&["campaign_id", "reconsideration_id"]),
                "post": command("Resolve reconsideration")
            },
            "/api/v1/campaigns/{campaign_id}/forks": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Fork Campaign from canonical Session history")
            },
            "/api/v1/campaigns/{campaign_id}/exports": {
                "parameters": path_parameters(&["campaign_id"]),
                "post": command("Request Campaign export")
            },
            "/api/v1/campaigns/{campaign_id}/exports/{export_id}": {
                "parameters": path_parameters(&["campaign_id", "export_id"]),
                "get": query("Get Campaign export status")
            }
        },
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer"
                }
            },
            "schemas": {
                "ApiCommandFields": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "command_id",
                        "idempotency_key",
                        "expected_version",
                        "correlation_id",
                        "causation_id",
                        "trace_id"
                    ],
                    "properties": {
                        "command_id": {"type": "string"},
                        "idempotency_key": {"type": "string", "maxLength": 160},
                        "expected_version": {"type": "integer", "minimum": 0},
                        "correlation_id": {"type": "string"},
                        "causation_id": {"type": "string"},
                        "trace_id": {"type": "string"}
                    }
                },
                "CoreApiCommitReceipt": {
                    "type": "object",
                    "required": ["last_event_sequence", "aggregate_version"],
                    "properties": {
                        "last_event_sequence": {"type": "integer"},
                        "aggregate_version": {"type": "integer"}
                    }
                }
            }
        }
    })
}
