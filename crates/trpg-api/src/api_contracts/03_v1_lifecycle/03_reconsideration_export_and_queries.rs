impl<P> V1LifecycleApi<P>
where
    P: V1LifecyclePort,
{
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
        if request.audience != "PLAYER" {
            context.require_keeper()?;
        }
        if request.command.expected_version != 0
            || request.requested_by != context.actor_id()
            || !matches!(
                request.audience.as_str(),
                "PLAYER" | "KEEPER_PRIVATE" | "AUDIT" | "CAMPAIGN_ARCHIVE"
            )
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

    pub async fn issue_campaign_export_download(
        &self,
        actor_id: &str,
        include_all: bool,
        campaign_id: &str,
        export_id: &str,
        now_unix_ms: u64,
    ) -> Result<CampaignExportDownloadAuthorizationApiResponse, CoreApiError> {
        validate_ids(&[actor_id, campaign_id, export_id])?;
        self.port
            .issue_campaign_export_download(
                actor_id,
                include_all,
                campaign_id,
                export_id,
                now_unix_ms,
            )
            .await
    }

    pub async fn consume_campaign_export_download(
        &self,
        actor_id: &str,
        campaign_id: &str,
        export_id: &str,
        token: &str,
        now_unix_ms: u64,
    ) -> Result<CampaignExportDownloadDescriptor, CoreApiError> {
        validate_ids(&[actor_id, campaign_id, export_id])?;
        if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(CoreApiError::NotFound);
        }
        self.port
            .consume_campaign_export_download(
                actor_id,
                campaign_id,
                export_id,
                token,
                now_unix_ms,
            )
            .await
    }
}
