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

    pub async fn create_forked_campaign(
        &self,
        create_context: &AuthorizedCoreApiContext,
        fork_context: &AuthorizedCoreApiContext,
        request: &CreateForkedCampaignApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        CampaignCharacterApi::new(Arc::clone(&self.port))
            .create_forked_campaign(create_context, fork_context, request)
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

}
