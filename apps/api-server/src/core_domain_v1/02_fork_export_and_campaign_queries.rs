macro_rules! lifecycle_fork_export_query_methods {
    () => {
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
    };
}
