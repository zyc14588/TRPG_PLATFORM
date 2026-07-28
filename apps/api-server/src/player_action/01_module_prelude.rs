use std::sync::Arc;

use trpg_api::api_contracts::{
    ApiCommandFields, AuthorizedCoreApiContext, ConfirmPlayerActionApiRequest, CoreApiError,
    CoreApiFuture, PlayerActionApiReceipt, PlayerActionCommandPort, PlayerActionIntentApiRequest,
    SubmitPlayerActionApiRequest,
};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{PersistedCommit, PolicyAuditDraft};
use trpg_data_eventing::persistence_postgresql::{
    CoreCommandMetadata, CoreDomainRepository, CoreDomainRepositoryError,
    InvestigationExecutionRecord, PlayerActionDiceRecord, PlayerActionIntentRecord,
    SanityExecutionRecord, SubmitPlayerActionRequest,
};
use trpg_ruleset_coc7::character_combat_san_chase::Coc7CharacterSheet;
use trpg_ruleset_coc7::dice_roll_contract::{
    server_roll_skill_check, DiceAdjustment, SuccessLevel,
};
use trpg_ruleset_coc7::investigation_clue_npc_time::{
    resolve_clue_check, ClueImportance, ClueOutcome,
};
use trpg_ruleset_coc7::san::resolve_san_check;
use trpg_ruleset_coc7::sanity_madness_state_machine::MadnessState;
use trpg_runtime::session_runtime::{
    ConfirmablePlayerAction, ExecutedDiceRoll, HumanKpPlayerActionWorkflow, PendingPlayerAction,
    PlayerActionAuthorityContext, PlayerActionCommitReceipt, PlayerActionConfirmation,
    PlayerActionIntent, PlayerActionRuntimeError, PlayerActionRuntimeFuture,
    PlayerActionSubmission, PlayerActionToolExecutor, PlayerActionToolResult,
    PlayerActionWorkflowStore,
};
use trpg_shared_kernel::EventActorOriginWire;

#[derive(Clone)]
struct RepositoryPlayerActionStore {
    repository: CoreDomainRepository,
    metadata: CoreCommandMetadata,
}

impl RepositoryPlayerActionStore {
    fn receipt(persisted: PersistedCommit, state: &str) -> PlayerActionCommitReceipt {
        PlayerActionCommitReceipt {
            first_event_sequence: persisted.first_event_sequence,
            last_event_sequence: persisted.last_event_sequence,
            aggregate_version: persisted.last_stream_version,
            state: state.to_owned(),
            realtime_delta_id: format!("delta_player_action_{}", persisted.last_event_sequence),
        }
    }

    fn map_error(error: CoreDomainRepositoryError) -> PlayerActionRuntimeError {
        match error {
            CoreDomainRepositoryError::InvalidInput(field) => {
                PlayerActionRuntimeError::InvalidInput(field)
            }
            CoreDomainRepositoryError::Forbidden
            | CoreDomainRepositoryError::PolicyEvidenceMismatch => {
                PlayerActionRuntimeError::Forbidden
            }
            CoreDomainRepositoryError::NotFound(_) => PlayerActionRuntimeError::NotFound,
            CoreDomainRepositoryError::Domain(_)
            | CoreDomainRepositoryError::ConcurrentStart
            | CoreDomainRepositoryError::Integrity(_) => {
                PlayerActionRuntimeError::PersistenceFailed("integrity")
            }
            CoreDomainRepositoryError::Canonical(_)
            | CoreDomainRepositoryError::Database(_)
            | CoreDomainRepositoryError::Serialization => {
                PlayerActionRuntimeError::PersistenceFailed("repository")
            }
        }
    }
}

impl PlayerActionWorkflowStore for RepositoryPlayerActionStore {
    fn submit<'a>(
        &'a self,
        _context: &'a PlayerActionAuthorityContext,
        submission: &'a PlayerActionSubmission,
    ) -> PlayerActionRuntimeFuture<'a, PlayerActionCommitReceipt> {
        Box::pin(async move {
            let intent = match &submission.intent {
                PlayerActionIntent::Investigation {
                    skill_name,
                    clue_id,
                    clue_importance,
                    adjustment,
                } => PlayerActionIntentRecord::Investigation {
                    skill_name: skill_name.clone(),
                    clue_id: clue_id.clone(),
                    clue_importance: clue_importance.clone(),
                    adjustment: adjustment.clone(),
                },
                PlayerActionIntent::SanityCheck {
                    success_loss,
                    failure_loss,
                    day_key,
                } => PlayerActionIntentRecord::SanityCheck {
                    success_loss: *success_loss,
                    failure_loss: *failure_loss,
                    day_key: day_key.clone(),
                },
            };
            let persisted = self
                .repository
                .submit_player_action(
                    &self.metadata,
                    &SubmitPlayerActionRequest {
                        action_id: submission.action_id.clone(),
                        campaign_id: submission.campaign_id.clone(),
                        character_id: submission.character_id.clone(),
                        scene_id: submission.scene_id.clone(),
                        submitted_by: submission.submitted_by.clone(),
                        submitted_at_unix_ms: submission.submitted_at_unix_ms,
                        intent,
                    },
                )
                .await
                .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted, "AWAITING_HUMAN_CONFIRMATION"))
        })
    }

    fn load_for_confirmation<'a>(
        &'a self,
        _context: &'a PlayerActionAuthorityContext,
        confirmation: &'a PlayerActionConfirmation,
    ) -> PlayerActionRuntimeFuture<'a, ConfirmablePlayerAction> {
        Box::pin(async move {
            match self
                .repository
                .load_pending_player_action(&confirmation.campaign_id, &confirmation.action_id)
                .await
            {
                Ok(pending) => Ok(ConfirmablePlayerAction::Pending(PendingPlayerAction {
                    action_id: pending.action_id,
                    campaign_id: pending.campaign_id,
                    character_id: pending.character_id,
                    scene_id: pending.scene_id,
                    submitted_by: pending.submitted_by,
                    intent: match pending.intent {
                        PlayerActionIntentRecord::Investigation {
                            skill_name,
                            clue_id,
                            clue_importance,
                            adjustment,
                        } => PlayerActionIntent::Investigation {
                            skill_name,
                            clue_id,
                            clue_importance,
                            adjustment,
                        },
                        PlayerActionIntentRecord::SanityCheck {
                            success_loss,
                            failure_loss,
                            day_key,
                        } => PlayerActionIntent::SanityCheck {
                            success_loss,
                            failure_loss,
                            day_key,
                        },
                    },
                    character_sheet_json: pending.character_sheet_json,
                    character_sheet_version: pending.character_sheet_version,
                })),
                Err(CoreDomainRepositoryError::NotFound(_)) => {
                    let persisted = self
                        .repository
                        .load_resolved_player_action_receipt(
                            &self.metadata,
                            &confirmation.campaign_id,
                            &confirmation.action_id,
                        )
                        .await
                        .map_err(Self::map_error)?;
                    Ok(ConfirmablePlayerAction::Resolved(Self::receipt(
                        persisted, "RESOLVED",
                    )))
                }
                Err(error) => Err(Self::map_error(error)),
            }
        })
    }

    fn commit_execution<'a>(
        &'a self,
        context: &'a PlayerActionAuthorityContext,
        pending: &'a PendingPlayerAction,
        confirmation: &'a PlayerActionConfirmation,
        result: &'a PlayerActionToolResult,
    ) -> PlayerActionRuntimeFuture<'a, PlayerActionCommitReceipt> {
        Box::pin(async move {
            let persisted = match result {
                PlayerActionToolResult::Investigation {
                    dice,
                    skill_name,
                    clue_record_id,
                    clue_id,
                    clue_importance,
                    clue_outcome,
                    clue_cost,
                    revealed_to_party,
                } => {
                    self.repository
                        .commit_investigation_execution(
                            &self.metadata,
                            &InvestigationExecutionRecord {
                                action_id: pending.action_id.clone(),
                                campaign_id: pending.campaign_id.clone(),
                                character_id: pending.character_id.clone(),
                                decision_id: confirmation.decision_id.clone(),
                                tool_execution_id: confirmation.tool_execution_id.clone(),
                                confirmed_by: context.actor_id.clone(),
                                resolved_at_unix_ms: confirmation.resolved_at_unix_ms,
                                dice: data_dice(dice),
                                skill_name: skill_name.clone(),
                                clue_record_id: clue_record_id.clone(),
                                clue_id: clue_id.clone(),
                                clue_importance: clue_importance.clone(),
                                clue_outcome: clue_outcome.clone(),
                                clue_cost: clue_cost.clone(),
                                revealed_to_party: *revealed_to_party,
                            },
                        )
                        .await
                }
                PlayerActionToolResult::Sanity {
                    dice,
                    sanity_event_id,
                    sheet_version_id,
                    day_key,
                    day_start_sanity,
                    sanity_before,
                    sanity_after,
                    sanity_loss,
                    day_loss,
                    indefinite_threshold,
                    madness_state,
                } => {
                    self.repository
                        .commit_sanity_execution(
                            &self.metadata,
                            &SanityExecutionRecord {
                                action_id: pending.action_id.clone(),
                                campaign_id: pending.campaign_id.clone(),
                                character_id: pending.character_id.clone(),
                                decision_id: confirmation.decision_id.clone(),
                                tool_execution_id: confirmation.tool_execution_id.clone(),
                                confirmed_by: context.actor_id.clone(),
                                resolved_at_unix_ms: confirmation.resolved_at_unix_ms,
                                dice: data_dice(dice),
                                sanity_event_id: sanity_event_id.clone(),
                                sheet_version_id: sheet_version_id.clone(),
                                day_key: day_key.clone(),
                                day_start_sanity: *day_start_sanity,
                                sanity_before: *sanity_before,
                                sanity_after: *sanity_after,
                                sanity_loss: *sanity_loss,
                                day_loss: *day_loss,
                                indefinite_threshold: *indefinite_threshold,
                                madness_state: madness_state.clone(),
                            },
                        )
                        .await
                }
            }
            .map_err(Self::map_error)?;
            Ok(Self::receipt(persisted, "RESOLVED"))
        })
    }
}

fn data_dice(dice: &ExecutedDiceRoll) -> PlayerActionDiceRecord {
    PlayerActionDiceRecord {
        roll_id: dice.roll_id.clone(),
        target_value: dice.target_value,
        rolled_value: dice.rolled_value,
        success_level: dice.success_level.clone(),
        selected_tens_digit: dice.selected_tens_digit,
        ones_digit: dice.ones_digit,
        adjustment: dice.adjustment.clone(),
    }
}

#[derive(Clone, Debug, Default)]
struct Coc7PlayerActionToolExecutor;
