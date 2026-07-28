
impl PlayerActionAuthorityContext {
    fn validate_human_kp(&self) -> Result<(), PlayerActionRuntimeError> {
        for value in [
            self.campaign_id.as_str(),
            self.authority_owner.as_str(),
            self.actor_id.as_str(),
        ] {
            EntityId::new(value)
                .map_err(|_| PlayerActionRuntimeError::InvalidInput("authority_context"))?;
        }
        if self.authority_mode != "human_kp" {
            return Err(PlayerActionRuntimeError::Forbidden);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum PlayerActionIntent {
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

impl PlayerActionIntent {
    fn validate(&self) -> Result<(), PlayerActionRuntimeError> {
        match self {
            Self::Investigation {
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
                    return Err(PlayerActionRuntimeError::InvalidInput(
                        "investigation_intent",
                    ));
                }
            }
            Self::SanityCheck {
                success_loss,
                failure_loss,
                day_key,
            } => {
                if day_key.trim().is_empty()
                    || day_key.len() > 128
                    || success_loss > failure_loss
                    || *failure_loss > 99
                {
                    return Err(PlayerActionRuntimeError::InvalidInput("sanity_intent"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerActionSubmission {
    pub action_id: String,
    pub campaign_id: String,
    pub character_id: String,
    pub scene_id: String,
    pub submitted_by: String,
    pub submitted_at_unix_ms: u64,
    pub intent: PlayerActionIntent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingPlayerAction {
    pub action_id: String,
    pub campaign_id: String,
    pub character_id: String,
    pub scene_id: String,
    pub submitted_by: String,
    pub intent: PlayerActionIntent,
    pub character_sheet_json: String,
    pub character_sheet_version: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerActionConfirmation {
    pub action_id: String,
    pub campaign_id: String,
    pub decision_id: String,
    pub tool_execution_id: String,
    pub resolved_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutedDiceRoll {
    pub roll_id: String,
    pub target_value: u8,
    pub rolled_value: u8,
    pub success_level: String,
    pub selected_tens_digit: u8,
    pub ones_digit: u8,
    pub adjustment: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerActionToolResult {
    Investigation {
        dice: ExecutedDiceRoll,
        skill_name: String,
        clue_record_id: String,
        clue_id: String,
        clue_importance: String,
        clue_outcome: String,
        clue_cost: Option<String>,
        revealed_to_party: bool,
    },
    Sanity {
        dice: ExecutedDiceRoll,
        sanity_event_id: String,
        sheet_version_id: String,
        day_key: String,
        day_start_sanity: u8,
        sanity_before: u8,
        sanity_after: u8,
        sanity_loss: u8,
        day_loss: u8,
        indefinite_threshold: u8,
        madness_state: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerActionCommitReceipt {
    pub first_event_sequence: i64,
    pub last_event_sequence: i64,
    pub aggregate_version: i64,
    pub state: String,
    pub realtime_delta_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfirmablePlayerAction {
    Pending(PendingPlayerAction),
    Resolved(PlayerActionCommitReceipt),
}

pub trait PlayerActionToolExecutor: Send + Sync {
    fn execute(
        &self,
        pending: &PendingPlayerAction,
        confirmation: &PlayerActionConfirmation,
    ) -> Result<PlayerActionToolResult, PlayerActionRuntimeError>;
}

pub trait PlayerActionWorkflowStore: Send + Sync {
    fn submit<'a>(
        &'a self,
        context: &'a PlayerActionAuthorityContext,
        submission: &'a PlayerActionSubmission,
    ) -> PlayerActionRuntimeFuture<'a, PlayerActionCommitReceipt>;

    fn load_for_confirmation<'a>(
        &'a self,
        context: &'a PlayerActionAuthorityContext,
        confirmation: &'a PlayerActionConfirmation,
    ) -> PlayerActionRuntimeFuture<'a, ConfirmablePlayerAction>;

    fn commit_execution<'a>(
        &'a self,
        context: &'a PlayerActionAuthorityContext,
        pending: &'a PendingPlayerAction,
        confirmation: &'a PlayerActionConfirmation,
        result: &'a PlayerActionToolResult,
    ) -> PlayerActionRuntimeFuture<'a, PlayerActionCommitReceipt>;
}

#[derive(Clone)]
pub struct HumanKpPlayerActionWorkflow<S, E> {
    store: Arc<S>,
    executor: Arc<E>,
}

impl<S, E> HumanKpPlayerActionWorkflow<S, E>
where
    S: PlayerActionWorkflowStore,
    E: PlayerActionToolExecutor,
{
    pub fn new(store: Arc<S>, executor: Arc<E>) -> Self {
        Self { store, executor }
    }

    pub async fn submit(
        &self,
        context: &PlayerActionAuthorityContext,
        submission: &PlayerActionSubmission,
    ) -> Result<PlayerActionCommitReceipt, PlayerActionRuntimeError> {
        context.validate_human_kp()?;
        submission.intent.validate()?;
        for value in [
            submission.action_id.as_str(),
            submission.campaign_id.as_str(),
            submission.character_id.as_str(),
            submission.scene_id.as_str(),
            submission.submitted_by.as_str(),
        ] {
            EntityId::new(value)
                .map_err(|_| PlayerActionRuntimeError::InvalidInput("submission_id"))?;
        }
        if context.actor_role != "investigator"
            || context.actor_id != submission.submitted_by
            || context.campaign_id != submission.campaign_id
            || submission.submitted_at_unix_ms == 0
        {
            return Err(PlayerActionRuntimeError::Forbidden);
        }
        self.store.submit(context, submission).await
    }

    pub async fn confirm(
        &self,
        context: &PlayerActionAuthorityContext,
        confirmation: &PlayerActionConfirmation,
    ) -> Result<PlayerActionCommitReceipt, PlayerActionRuntimeError> {
        context.validate_human_kp()?;
        for value in [
            confirmation.action_id.as_str(),
            confirmation.campaign_id.as_str(),
            confirmation.decision_id.as_str(),
            confirmation.tool_execution_id.as_str(),
        ] {
            EntityId::new(value)
                .map_err(|_| PlayerActionRuntimeError::InvalidInput("confirmation_id"))?;
        }
        if context.actor_role != "human_keeper"
            || context.actor_id != context.authority_owner
            || context.campaign_id != confirmation.campaign_id
            || confirmation.resolved_at_unix_ms == 0
        {
            return Err(PlayerActionRuntimeError::Forbidden);
        }

        match self
            .store
            .load_for_confirmation(context, confirmation)
            .await?
        {
            ConfirmablePlayerAction::Resolved(receipt) => Ok(receipt),
            ConfirmablePlayerAction::Pending(pending) => {
                if pending.action_id != confirmation.action_id
                    || pending.campaign_id != confirmation.campaign_id
                {
                    return Err(PlayerActionRuntimeError::Forbidden);
                }
                // The commit port is not called until a real tool executor has
                // returned an explicit result. Authorization is not execution.
                let result = self.executor.execute(&pending, confirmation)?;
                self.store
                    .commit_execution(context, &pending, confirmation, &result)
                    .await
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConclusionState {
    AwaitingEnding,
    AwaitingGrowth,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndingRecord {
    pub ending_event_id: EntityId,
    pub ending_id: EntityId,
    pub summary: String,
    pub ended_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillGrowthRecord {
    growth_event_id: EntityId,
    character_id: EntityId,
    source_sheet_version_id: EntityId,
    new_sheet_version_id: EntityId,
    skill_name: String,
    skill_before: u8,
    improvement_check_roll: u8,
    increase_roll: Option<u8>,
    skill_after: u8,
    server_roll_id: EntityId,
    increase_roll_id: Option<EntityId>,
}
