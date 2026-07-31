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

impl V1LifecyclePort for RepositoryCampaignCharacterPort {
    fn update_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a UpdateCharacterApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let sheet: Coc7CharacterSheet = serde_json::from_str(&request.sheet_json)
                .map_err(|_| CoreApiError::InvalidInput("coc7_character_sheet"))?;
            sheet
                .validate()
                .map_err(|_| CoreApiError::InvalidInput("coc7_character_sheet"))?;
            let sheet_json =
                serde_json::to_string(&sheet).map_err(|_| CoreApiError::Unavailable("serde"))?;
            let persisted = self
                .repository
                .update_character(
                    &Self::metadata(
                        context,
                        &request.command,
                        "private_to_player",
                        &request.owner_user_id,
                    ),
                    &UpdateCharacterRequest {
                        character_id: request.character_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        owner_user_id: request.owner_user_id.clone(),
                        display_name: request.display_name.clone(),
                        sheet_version_id: request.sheet_version_id.clone(),
                        sheet_json,
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn join_character_session<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a JoinCharacterSessionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .join_character_session(
                    &Self::metadata(
                        context,
                        &request.command,
                        "party_visible",
                        "not_applicable",
                    ),
                    &JoinCharacterSessionRequest {
                        join_id: request.join_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        session_id: request.session_id.clone(),
                        character_id: request.character_id.clone(),
                        owner_user_id: request.owner_user_id.clone(),
                        joined_at_unix_ms: request.joined_at_unix_ms,
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn import_scenario<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ImportScenarioApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .import_scenario(
                    &Self::metadata(
                        context,
                        &request.command,
                        "keeper_only",
                        "not_applicable",
                    ),
                    &ImportScenarioRequest {
                        scenario_id: request.scenario_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        ruleset_id: request.ruleset_id.clone(),
                        format_version: request.format_version.clone(),
                        content_hash: request.content_hash.clone(),
                        document_json: request.document_json.clone(),
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn start_session<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a StartSessionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .start_session(
                    &Self::metadata(
                        context,
                        &request.command,
                        "party_visible",
                        "not_applicable",
                    ),
                    &StartSessionRequest {
                        session_id: request.session_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        room_id: request.room_id.clone(),
                        scenario_id: request.scenario_id.clone(),
                        scene_id: request.scene_id.clone(),
                        scene_key: request.scene_key.clone(),
                        scene_name: request.scene_name.clone(),
                        started_at_unix_ms: request.started_at_unix_ms,
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn change_session_state<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ChangeSessionStateApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let next_state = match request.state {
                trpg_api::api_contracts::SessionStateApiRequest::Active => SessionState::Active,
                trpg_api::api_contracts::SessionStateApiRequest::Paused => SessionState::Paused,
                trpg_api::api_contracts::SessionStateApiRequest::Ended => SessionState::Ended,
            };
            let persisted = self
                .repository
                .change_session_state(
                    &Self::metadata(
                        context,
                        &request.command,
                        "party_visible",
                        "not_applicable",
                    ),
                    &request.campaign_id,
                    &request.session_id,
                    next_state,
                    request.changed_at_unix_ms,
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn switch_scene<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a SwitchSceneApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .switch_scene(
                    &Self::metadata(
                        context,
                        &request.command,
                        "party_visible",
                        "not_applicable",
                    ),
                    &SwitchSceneRequest {
                        session_id: request.session_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        next_scene_id: request.next_scene_id.clone(),
                        next_scene_key: request.next_scene_key.clone(),
                        next_scene_name: request.next_scene_name.clone(),
                        switched_at_unix_ms: request.switched_at_unix_ms,
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn request_reconsideration<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a RequestReconsiderationApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .request_reconsideration(
                    &Self::metadata(
                        context,
                        &request.command,
                        "party_visible",
                        "not_applicable",
                    ),
                    &RequestReconsiderationRequest {
                        reconsideration_id: request.reconsideration_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        original_event_sequence: request.original_event_sequence,
                        requested_by: request.requested_by.clone(),
                        reason: request.reason.clone(),
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn review_reconsideration<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ReviewReconsiderationApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let persisted = self
                .repository
                .review_reconsideration(
                    &Self::metadata(
                        context,
                        &request.command,
                        "party_visible",
                        "not_applicable",
                    ),
                    &ReviewReconsiderationRequest {
                        reconsideration_id: request.reconsideration_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        review_event_id: request.review_event_id.clone(),
                        review_summary: request.review_summary.clone(),
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn resolve_reconsideration<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ResolveReconsiderationApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let outcome = match request.outcome {
                ReconsiderationOutcomeApiRequest::Upheld => ReconsiderationOutcome::Upheld,
                ReconsiderationOutcomeApiRequest::Corrected => ReconsiderationOutcome::Corrected,
            };
            let persisted = self
                .repository
                .resolve_reconsideration(
                    &Self::metadata(
                        context,
                        &request.command,
                        "party_visible",
                        "not_applicable",
                    ),
                    &ResolveReconsiderationRequest {
                        reconsideration_id: request.reconsideration_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        resolution_event_id: request.resolution_event_id.clone(),
                        outcome,
                        resolution: request.resolution.clone(),
                        corrected_event_type: request.corrected_event_type.clone(),
                        corrected_payload_json: request.corrected_payload_json.clone(),
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn fork_campaign<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ForkCampaignApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let snapshot = self
                .repository
                .preview_campaign_fork(
                    &request.parent_campaign_id,
                    &request.source_session_id,
                    context.actor_id(),
                )
                .await
                .map_err(Self::map_error)?;
            let persisted = self
                .repository
                .record_campaign_fork(
                    &Self::metadata(
                        context,
                        &request.command,
                        "keeper_only",
                        "not_applicable",
                    ),
                    &RecordCampaignForkRequest {
                        fork_id: request.fork_id.clone(),
                        parent_campaign_id: request.parent_campaign_id.clone(),
                        child_campaign_id: request.child_campaign_id.clone(),
                        source_session_id: request.source_session_id.clone(),
                        snapshot_hash: snapshot.snapshot_hash,
                        reason: request.reason.clone(),
                        copy_scopes: snapshot.copy_scopes,
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn request_campaign_export<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a RequestCampaignExportApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt> {
        Box::pin(async move {
            let (visibility_label, visibility_subject) = if request.audience == "PLAYER" {
                ("private_to_player", request.requested_by.as_str())
            } else {
                ("keeper_only", "not_applicable")
            };
            let persisted = self
                .repository
                .request_campaign_export(
                    &Self::metadata(
                        context,
                        &request.command,
                        visibility_label,
                        visibility_subject,
                    ),
                    &RequestCampaignExportRequest {
                        export_id: request.export_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        requested_by: request.requested_by.clone(),
                        audience: request.audience.clone(),
                        requested_at_unix_ms: request.requested_at_unix_ms,
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted))
        })
    }

    fn list_campaigns<'a>(
        &'a self,
        actor_id: &'a str,
        include_all: bool,
    ) -> CoreApiFuture<'a, Vec<CampaignApiResponse>> {
        Box::pin(async move {
            self.repository
                .list_campaigns_for_actor(actor_id, include_all)
                .await
                .map(|campaigns| {
                    campaigns
                        .into_iter()
                        .map(|campaign| CampaignApiResponse {
                            campaign_id: campaign.campaign_id,
                            owner_user_id: campaign.owner_user_id,
                            authority_contract_id: campaign.authority_contract_id,
                            title: campaign.title,
                            state: campaign.state,
                            aggregate_version: campaign.aggregate_version,
                            last_event_sequence: campaign.last_event_sequence,
                        })
                        .collect()
                })
                .map_err(Self::map_error)
        })
    }

    fn get_campaign<'a>(
        &'a self,
        actor_id: &'a str,
        include_all: bool,
        campaign_id: &'a str,
    ) -> CoreApiFuture<'a, CampaignApiResponse> {
        Box::pin(async move {
            self.repository
                .get_campaign_for_actor(actor_id, include_all, campaign_id)
                .await
                .map(|campaign| CampaignApiResponse {
                    campaign_id: campaign.campaign_id,
                    owner_user_id: campaign.owner_user_id,
                    authority_contract_id: campaign.authority_contract_id,
                    title: campaign.title,
                    state: campaign.state,
                    aggregate_version: campaign.aggregate_version,
                    last_event_sequence: campaign.last_event_sequence,
                })
                .map_err(Self::map_error)
        })
    }

    fn get_campaign_export<'a>(
        &'a self,
        actor_id: &'a str,
        include_all: bool,
        campaign_id: &'a str,
        export_id: &'a str,
    ) -> CoreApiFuture<'a, CampaignExportApiResponse> {
        Box::pin(async move {
            self.repository
                .get_campaign_export_for_actor(actor_id, include_all, campaign_id, export_id)
                .await
                .map(|export| CampaignExportApiResponse {
                    export_id: export.export_id,
                    campaign_id: export.campaign_id,
                    requested_by: export.requested_by,
                    audience: export.audience,
                    state: export.state,
                    attempt_count: export.attempt_count,
                    max_attempts: export.max_attempts,
                    failure_code: export.failure_code,
                    artifact_schema: export.artifact_schema,
                    visibility_policy_version: export.visibility_policy_version,
                    artifact_hash: export.artifact_hash,
                    manifest_hash: export.manifest_hash,
                    artifact_size: export.artifact_size,
                    first_event_sequence: export.first_event_sequence,
                    last_exported_event_sequence: export.last_exported_event_sequence,
                    event_count: export.event_count,
                    retention_expires_at_unix_ms: export.retention_expires_at_unix_ms,
                    fork_id: export.fork_id,
                    parent_campaign_id: export.parent_campaign_id,
                    source_session_id: export.source_session_id,
                    source_snapshot_hash: export.source_snapshot_hash,
                    child_snapshot_hash: export.child_snapshot_hash,
                    aggregate_version: export.aggregate_version,
                    last_event_sequence: export.last_event_sequence,
                })
                .map_err(Self::map_error)
        })
    }

    fn issue_campaign_export_download<'a>(
        &'a self,
        actor_id: &'a str,
        include_all: bool,
        campaign_id: &'a str,
        export_id: &'a str,
        now_unix_ms: u64,
    ) -> CoreApiFuture<'a, CampaignExportDownloadAuthorizationApiResponse> {
        Box::pin(async move {
            self.repository
                .issue_campaign_export_download_for_actor(
                    actor_id,
                    include_all,
                    campaign_id,
                    export_id,
                    now_unix_ms,
                    60_000,
                )
                .await
                .map(|authorization| CampaignExportDownloadAuthorizationApiResponse {
                    token: authorization.token,
                    expires_at_unix_ms: authorization.expires_at_unix_ms,
                    download_path: format!(
                        "/api/v1/campaigns/{campaign_id}/exports/{export_id}/download"
                    ),
                })
                .map_err(Self::map_error)
        })
    }

    fn consume_campaign_export_download<'a>(
        &'a self,
        actor_id: &'a str,
        campaign_id: &'a str,
        export_id: &'a str,
        token: &'a str,
        now_unix_ms: u64,
    ) -> CoreApiFuture<'a, CampaignExportDownloadDescriptor> {
        Box::pin(async move {
            self.repository
                .consume_campaign_export_download_for_actor(
                    actor_id,
                    campaign_id,
                    export_id,
                    token,
                    now_unix_ms,
                )
                .await
                .map(|CampaignExportDownloadArtifact {
                    artifact_key,
                    artifact_hash,
                }| CampaignExportDownloadDescriptor {
                    artifact_key,
                    artifact_hash,
                })
                .map_err(Self::map_error)
        })
    }
}
