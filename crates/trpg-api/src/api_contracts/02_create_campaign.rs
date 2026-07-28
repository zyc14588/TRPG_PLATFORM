
pub trait CampaignCharacterCommandPort: Send + Sync {
    fn create_campaign<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CreateCampaignApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn issue_invite<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a IssueInviteApiRequest,
    ) -> CoreApiFuture<'a, IssuedInviteApiResponse>;

    fn accept_invite<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a AcceptInviteApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn create_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CreateCharacterApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn submit_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CharacterTransitionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;

    fn review_character<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a CharacterTransitionApiRequest,
    ) -> CoreApiFuture<'a, CoreApiCommitReceipt>;
}

#[derive(Clone)]
pub struct CampaignCharacterApi<P> {
    port: Arc<P>,
}

impl<P> CampaignCharacterApi<P>
where
    P: CampaignCharacterCommandPort,
{
    pub fn new(port: Arc<P>) -> Self {
        Self { port }
    }

    pub async fn create_campaign(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CreateCampaignApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "campaign", &request.campaign_id)?;
        context.require_keeper()?;
        if request.command.expected_version != 0
            || request.owner_user_id != context.actor_id()
            || request.authority.contract_id != context.authority_contract_id()
            || request.authority.authority_owner != context.authority_owner()
            || request.authority.authority_mode.to_ascii_lowercase() != context.authority_mode()
            || request.title.trim().is_empty()
            || request.room_name.trim().is_empty()
            || request.created_at_unix_ms == 0
        {
            return Err(CoreApiError::InvalidInput("campaign"));
        }
        for value in [
            request.campaign_id.as_str(),
            request.owner_user_id.as_str(),
            request.room_id.as_str(),
            request.authority.contract_id.as_str(),
        ] {
            EntityId::new(value).map_err(|_| CoreApiError::InvalidInput("campaign_id"))?;
        }
        self.port.create_campaign(context, request).await
    }

    pub async fn issue_invite(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &IssueInviteApiRequest,
    ) -> Result<IssuedInviteApiResponse, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "campaign_invite", &request.invite_id)?;
        context.require_keeper()?;
        if request.command.expected_version != 0
            || !matches!(request.role.as_str(), "PLAYER" | "SPECTATOR")
            || request.expires_at_unix_ms == 0
        {
            return Err(CoreApiError::InvalidInput("invite"));
        }
        for value in [request.invite_id.as_str(), request.invited_user_id.as_str()] {
            EntityId::new(value).map_err(|_| CoreApiError::InvalidInput("invite_id"))?;
        }
        self.port.issue_invite(context, request).await
    }

    pub async fn accept_invite(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &AcceptInviteApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "campaign_invite", &request.invite_id)?;
        if request.command.expected_version != 1
            || request.accepting_user_id != context.actor_id()
            || request.raw_token.is_empty()
            || request.raw_token.len() > 256
        {
            return Err(CoreApiError::InvalidInput("invite_accept"));
        }
        self.port.accept_invite(context, request).await
    }

    pub async fn create_character(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CreateCharacterApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "character", &request.character_id)?;
        if request.command.expected_version != 0
            || request.owner_user_id != context.actor_id()
            || request.display_name.trim().is_empty()
        {
            return Err(CoreApiError::InvalidInput("character"));
        }
        let sheet: serde_json::Value = serde_json::from_str(&request.sheet_json)
            .map_err(|_| CoreApiError::InvalidInput("character_sheet"))?;
        if !sheet.is_object() {
            return Err(CoreApiError::InvalidInput("character_sheet"));
        }
        self.port.create_character(context, request).await
    }

    pub async fn submit_character(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CharacterTransitionApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "character", &request.character_id)?;
        self.port.submit_character(context, request).await
    }

    pub async fn review_character(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &CharacterTransitionApiRequest,
    ) -> Result<CoreApiCommitReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "character", &request.character_id)?;
        context.require_keeper()?;
        self.port.review_character(context, request).await
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum PlayerActionIntentApiRequest {
    Investigation {
        skill_name: String,
        clue_id: String,
        clue_importance: String,
        adjustment: String,
    },
    SanityCheck {
        success_loss: u8,
        failure_loss: u8,
        day_key: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitPlayerActionApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub action_id: String,
    pub character_id: String,
    pub scene_id: String,
    pub submitted_at_unix_ms: u64,
    pub intent: PlayerActionIntentApiRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmPlayerActionApiRequest {
    pub command: ApiCommandFields,
    pub campaign_id: String,
    pub action_id: String,
    pub resolved_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PlayerActionApiReceipt {
    pub first_event_sequence: i64,
    pub last_event_sequence: i64,
    pub aggregate_version: i64,
    pub state: String,
    pub realtime_delta_id: String,
}

pub trait PlayerActionCommandPort: Send + Sync {
    fn submit_player_action<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a SubmitPlayerActionApiRequest,
    ) -> CoreApiFuture<'a, PlayerActionApiReceipt>;

    fn confirm_player_action<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ConfirmPlayerActionApiRequest,
    ) -> CoreApiFuture<'a, PlayerActionApiReceipt>;
}

#[derive(Clone)]
pub struct PlayerActionApi<P> {
    port: Arc<P>,
}

impl<P> PlayerActionApi<P>
where
    P: PlayerActionCommandPort,
{
    pub fn new(port: Arc<P>) -> Self {
        Self { port }
    }

    pub async fn submit(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &SubmitPlayerActionApiRequest,
    ) -> Result<PlayerActionApiReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "player_action", &request.action_id)?;
        for value in [
            request.campaign_id.as_str(),
            request.action_id.as_str(),
            request.character_id.as_str(),
            request.scene_id.as_str(),
        ] {
            EntityId::new(value).map_err(|_| CoreApiError::InvalidInput("player_action_id"))?;
        }
        if context.authority_mode() != "human_kp"
            || context.actor_role() != "investigator"
            || request.command.expected_version != 0
            || request.submitted_at_unix_ms == 0
        {
            return Err(CoreApiError::Forbidden);
        }
        match &request.intent {
            PlayerActionIntentApiRequest::Investigation {
                skill_name,
                clue_id,
                clue_importance,
                adjustment,
            } => {
                if skill_name.trim().is_empty()
                    || skill_name.len() > 128
                    || EntityId::new(clue_id).is_err()
                    || !matches!(clue_importance.as_str(), "CORE" | "OPTIONAL")
                    || !matches!(adjustment.as_str(), "NONE" | "BONUS" | "PENALTY")
                {
                    return Err(CoreApiError::InvalidInput("investigation_intent"));
                }
            }
            PlayerActionIntentApiRequest::SanityCheck {
                success_loss,
                failure_loss,
                day_key,
            } => {
                if day_key.trim().is_empty()
                    || day_key.len() > 128
                    || success_loss > failure_loss
                    || *failure_loss > 99
                {
                    return Err(CoreApiError::InvalidInput("sanity_intent"));
                }
            }
        }
        self.port.submit_player_action(context, request).await
    }

    pub async fn confirm(
        &self,
        context: &AuthorizedCoreApiContext,
        request: &ConfirmPlayerActionApiRequest,
    ) -> Result<PlayerActionApiReceipt, CoreApiError> {
        request.command.validate()?;
        context.validate(&request.campaign_id, "player_action", &request.action_id)?;
        context.require_keeper()?;
        if context.authority_mode() != "human_kp"
            || context.actor_id() != context.authority_owner()
            || request.command.expected_version != 1
            || request.resolved_at_unix_ms == 0
        {
            return Err(CoreApiError::Forbidden);
        }
        self.port.confirm_player_action(context, request).await
    }
}

#[cfg(test)]
mod tests {
    use super::{AcceptInviteApiRequest, IssueInviteApiRequest};

    #[test]
    fn invite_api_rejects_client_supplied_clock_fields() {
        let command = serde_json::json!({
            "command_id": "command_invite_clock",
            "idempotency_key": "idempotency_invite_clock",
            "expected_version": 0,
            "correlation_id": "correlation_invite_clock",
            "causation_id": "causation_invite_clock",
            "trace_id": "trace_invite_clock"
        });
        let issue = serde_json::json!({
            "command": command,
            "campaign_id": "campaign_invite_clock",
            "invite_id": "invite_clock",
            "invited_user_id": "player_invite_clock",
            "role": "PLAYER",
            "expires_at_unix_ms": 2_000_000_000_000_u64,
            "now_unix_ms": 1
        });
        assert!(serde_json::from_value::<IssueInviteApiRequest>(issue).is_err());

        let accept = serde_json::json!({
            "command": {
                "command_id": "command_accept_clock",
                "idempotency_key": "idempotency_accept_clock",
                "expected_version": 1,
                "correlation_id": "correlation_accept_clock",
                "causation_id": "causation_accept_clock",
                "trace_id": "trace_accept_clock"
            },
            "campaign_id": "campaign_invite_clock",
            "invite_id": "invite_clock",
            "accepting_user_id": "player_invite_clock",
            "raw_token": "opaque-token",
            "accepted_at_unix_ms": 1
        });
        assert!(serde_json::from_value::<AcceptInviteApiRequest>(accept).is_err());
    }
}
