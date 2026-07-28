
impl PlayerActionCommandPort for RepositoryPlayerActionPort {
    fn submit_player_action<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a SubmitPlayerActionApiRequest,
    ) -> CoreApiFuture<'a, PlayerActionApiReceipt> {
        Box::pin(async move {
            let (intent, visibility_label, visibility_subject) = match &request.intent {
                PlayerActionIntentApiRequest::Investigation {
                    skill_name,
                    clue_id,
                    clue_importance,
                    adjustment,
                } => (
                    PlayerActionIntent::Investigation {
                        skill_name: skill_name.clone(),
                        clue_id: clue_id.clone(),
                        clue_importance: clue_importance.clone(),
                        adjustment: adjustment.clone(),
                    },
                    "party_visible",
                    "not_applicable".to_owned(),
                ),
                PlayerActionIntentApiRequest::SanityCheck {
                    success_loss,
                    failure_loss,
                    day_key,
                } => (
                    PlayerActionIntent::SanityCheck {
                        success_loss: *success_loss,
                        failure_loss: *failure_loss,
                        day_key: day_key.clone(),
                    },
                    "private_to_player",
                    context.actor_id().to_owned(),
                ),
            };
            let store = Arc::new(RepositoryPlayerActionStore {
                repository: self.repository.clone(),
                metadata: Self::metadata(
                    context,
                    &request.command,
                    visibility_label,
                    &visibility_subject,
                ),
            });
            let workflow =
                HumanKpPlayerActionWorkflow::new(store, Arc::new(Coc7PlayerActionToolExecutor));
            workflow
                .submit(
                    &Self::runtime_context(context),
                    &PlayerActionSubmission {
                        action_id: request.action_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        character_id: request.character_id.clone(),
                        scene_id: request.scene_id.clone(),
                        submitted_by: context.actor_id().to_owned(),
                        submitted_at_unix_ms: request.submitted_at_unix_ms,
                        intent,
                    },
                )
                .await
                .map(Self::api_receipt)
                .map_err(Self::map_runtime_error)
        })
    }

    fn confirm_player_action<'a>(
        &'a self,
        context: &'a AuthorizedCoreApiContext,
        request: &'a ConfirmPlayerActionApiRequest,
    ) -> CoreApiFuture<'a, PlayerActionApiReceipt> {
        Box::pin(async move {
            let header = self
                .repository
                .load_player_action_header(&request.campaign_id, &request.action_id)
                .await
                .map_err(|error| {
                    Self::map_runtime_error(RepositoryPlayerActionStore::map_error(error))
                })?;
            let (visibility_label, visibility_subject) = if header.action_kind == "SANITY_CHECK" {
                ("private_to_player", header.submitted_by)
            } else {
                ("party_visible", "not_applicable".to_owned())
            };
            let store = Arc::new(RepositoryPlayerActionStore {
                repository: self.repository.clone(),
                metadata: Self::metadata(
                    context,
                    &request.command,
                    visibility_label,
                    &visibility_subject,
                ),
            });
            let workflow =
                HumanKpPlayerActionWorkflow::new(store, Arc::new(Coc7PlayerActionToolExecutor));
            workflow
                .confirm(
                    &Self::runtime_context(context),
                    &PlayerActionConfirmation {
                        action_id: request.action_id.clone(),
                        campaign_id: request.campaign_id.clone(),
                        decision_id: format!("decision_{}", request.command.command_id),
                        tool_execution_id: format!("tool_execution_{}", request.command.command_id),
                        resolved_at_unix_ms: request.resolved_at_unix_ms,
                    },
                )
                .await
                .map(Self::api_receipt)
                .map_err(Self::map_runtime_error)
        })
    }
}
