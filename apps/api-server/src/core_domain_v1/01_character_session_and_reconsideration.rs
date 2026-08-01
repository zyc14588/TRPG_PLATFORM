macro_rules! lifecycle_character_session_methods {
    () => {
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
    };
}
