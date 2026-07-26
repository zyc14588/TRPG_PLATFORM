//! Production P07 player-action adapter.
//!
//! The API layer supplies only authenticated intent. This adapter composes the
//! runtime confirmation workflow, the COC7 rules executor, and the canonical
//! PostgreSQL repository. No transport/model supplied dice value reaches the
//! executor.

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

impl PlayerActionToolExecutor for Coc7PlayerActionToolExecutor {
    fn execute(
        &self,
        pending: &PendingPlayerAction,
        _confirmation: &PlayerActionConfirmation,
    ) -> Result<PlayerActionToolResult, PlayerActionRuntimeError> {
        match &pending.intent {
            PlayerActionIntent::Investigation {
                skill_name,
                clue_id,
                clue_importance,
                adjustment,
            } => {
                let sheet: Coc7CharacterSheet = serde_json::from_str(&pending.character_sheet_json)
                    .map_err(|_| {
                        PlayerActionRuntimeError::ToolExecutionFailed("character_sheet_invalid")
                    })?;
                sheet.validate().map_err(|_| {
                    PlayerActionRuntimeError::ToolExecutionFailed("character_sheet_invalid")
                })?;
                let target = *sheet.skills.get(skill_name).ok_or(
                    PlayerActionRuntimeError::ToolExecutionFailed("character_skill_missing"),
                )?;
                let adjustment_value = parse_adjustment(adjustment)?;
                let roll = server_roll_skill_check(target, adjustment_value).map_err(|_| {
                    PlayerActionRuntimeError::ToolExecutionFailed("server_rng_unavailable")
                })?;
                let succeeded = matches!(
                    roll.outcome().success_level,
                    SuccessLevel::Critical
                        | SuccessLevel::Extreme
                        | SuccessLevel::Hard
                        | SuccessLevel::Regular
                );
                let importance = match clue_importance.as_str() {
                    "CORE" => ClueImportance::Core,
                    "OPTIONAL" => ClueImportance::Optional,
                    _ => return Err(PlayerActionRuntimeError::InvalidInput("clue_importance")),
                };
                let clue = resolve_clue_check(importance, succeeded);
                Ok(PlayerActionToolResult::Investigation {
                    dice: runtime_dice(&roll, adjustment),
                    skill_name: skill_name.clone(),
                    clue_record_id: format!("clue_result_{}", pending.action_id),
                    clue_id: clue_id.clone(),
                    clue_importance: clue_importance.clone(),
                    clue_outcome: clue_outcome_name(clue.outcome).to_owned(),
                    clue_cost: clue.cost.map(str::to_owned),
                    revealed_to_party: clue.outcome != ClueOutcome::NotFound,
                })
            }
            PlayerActionIntent::SanityCheck {
                success_loss,
                failure_loss,
                day_key,
            } => {
                let sheet: serde_json::Value = serde_json::from_str(&pending.character_sheet_json)
                    .map_err(|_| {
                        PlayerActionRuntimeError::ToolExecutionFailed("character_sheet_invalid")
                    })?;
                let power = json_u8(&sheet, "/characteristics/power")?;
                let state = sheet.get("sanity_state");
                let same_day = state
                    .and_then(|value| value.get("day_key"))
                    .and_then(serde_json::Value::as_str)
                    == Some(day_key);
                let current = state
                    .and_then(|value| value.get("current_sanity"))
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
                    .unwrap_or(power);
                if current == 0 {
                    return Err(PlayerActionRuntimeError::ToolExecutionFailed(
                        "sanity_exhausted",
                    ));
                }
                let day_start = if same_day {
                    state
                        .and_then(|value| value.get("day_start_sanity"))
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|value| u8::try_from(value).ok())
                        .ok_or(PlayerActionRuntimeError::ToolExecutionFailed(
                            "sanity_day_start_missing",
                        ))?
                } else {
                    current
                };
                let prior_day_loss = if same_day {
                    state
                        .and_then(|value| value.get("day_loss"))
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|value| u8::try_from(value).ok())
                        .ok_or(PlayerActionRuntimeError::ToolExecutionFailed(
                            "sanity_day_loss_missing",
                        ))?
                } else {
                    0
                };
                let roll =
                    server_roll_skill_check(current, DiceAdjustment::None).map_err(|_| {
                        PlayerActionRuntimeError::ToolExecutionFailed("server_rng_unavailable")
                    })?;
                let transition = resolve_san_check(
                    roll.outcome().roll,
                    current,
                    *success_loss,
                    *failure_loss,
                    prior_day_loss,
                    day_start,
                )
                .map_err(|_| {
                    PlayerActionRuntimeError::ToolExecutionFailed("sanity_resolution_failed")
                })?;
                Ok(PlayerActionToolResult::Sanity {
                    dice: runtime_dice(&roll, "NONE"),
                    sanity_event_id: format!("sanity_event_{}", pending.action_id),
                    sheet_version_id: format!(
                        "sheet_{}_after_{}",
                        pending.character_id, pending.action_id
                    ),
                    day_key: day_key.clone(),
                    day_start_sanity: transition.day_start_sanity,
                    sanity_before: transition.before,
                    sanity_after: transition.after,
                    sanity_loss: transition.loss,
                    day_loss: transition.day_loss,
                    indefinite_threshold: transition.indefinite_threshold,
                    madness_state: madness_state_name(transition.state).to_owned(),
                })
            }
        }
    }
}

fn parse_adjustment(value: &str) -> Result<DiceAdjustment, PlayerActionRuntimeError> {
    match value {
        "NONE" => Ok(DiceAdjustment::None),
        "BONUS" => Ok(DiceAdjustment::Bonus),
        "PENALTY" => Ok(DiceAdjustment::Penalty),
        _ => Err(PlayerActionRuntimeError::InvalidInput("dice_adjustment")),
    }
}

fn runtime_dice(
    roll: &trpg_ruleset_coc7::dice_roll_contract::ServerDiceRoll,
    adjustment: &str,
) -> ExecutedDiceRoll {
    ExecutedDiceRoll {
        roll_id: roll.roll_id().to_owned(),
        target_value: roll.outcome().target,
        rolled_value: roll.outcome().roll,
        success_level: success_level_name(roll.outcome().success_level).to_owned(),
        selected_tens_digit: roll.outcome().selected_tens_digit,
        ones_digit: roll.outcome().ones_digit,
        adjustment: adjustment.to_owned(),
    }
}

fn success_level_name(value: SuccessLevel) -> &'static str {
    match value {
        SuccessLevel::Critical => "CRITICAL",
        SuccessLevel::Extreme => "EXTREME",
        SuccessLevel::Hard => "HARD",
        SuccessLevel::Regular => "REGULAR",
        SuccessLevel::Failure => "FAILURE",
        SuccessLevel::Fumble => "FUMBLE",
    }
}

fn clue_outcome_name(value: ClueOutcome) -> &'static str {
    match value {
        ClueOutcome::Revealed => "REVEALED",
        ClueOutcome::RevealedWithCost => "REVEALED_WITH_COST",
        ClueOutcome::NotFound => "NOT_FOUND",
    }
}

fn madness_state_name(value: MadnessState) -> &'static str {
    match value {
        MadnessState::Stable => "STABLE",
        MadnessState::TemporaryInsanity => "TEMPORARY_INSANITY",
        MadnessState::IndefiniteInsanity => "INDEFINITE_INSANITY",
    }
}

fn json_u8(value: &serde_json::Value, pointer: &str) -> Result<u8, PlayerActionRuntimeError> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_u64)
        .and_then(|number| u8::try_from(number).ok())
        .ok_or(PlayerActionRuntimeError::ToolExecutionFailed(
            "character_numeric_field_missing",
        ))
}

#[derive(Clone, Debug)]
pub struct RepositoryPlayerActionPort {
    repository: CoreDomainRepository,
}

impl RepositoryPlayerActionPort {
    pub fn new(repository: CoreDomainRepository) -> Self {
        Self { repository }
    }

    fn metadata(
        context: &AuthorizedCoreApiContext,
        command: &ApiCommandFields,
        visibility_label: &str,
        visibility_subject: &str,
    ) -> CoreCommandMetadata {
        let audit = context.policy_audit();
        CoreCommandMetadata {
            commit_id: format!("commit_{}", command.command_id),
            command_id: command.command_id.clone(),
            idempotency_key: command.idempotency_key.clone(),
            expected_version: command.expected_version,
            requesting_actor_id: context.actor_id().to_owned(),
            requesting_actor_role: context.actor_role().to_owned(),
            authenticated_actor_id: context.workflow_actor_id().to_owned(),
            authenticated_actor_role: context.workflow_actor_role().to_owned(),
            authenticated_actor_origin: EventActorOriginWire::Workload {
                role: "workflow_engine".to_owned(),
            },
            authority_mode: context.authority_mode().to_owned(),
            authority_contract_version: context.authority_contract_version(),
            authority_contract_id: context.authority_contract_id().to_owned(),
            authority_owner: context.authority_owner().to_owned(),
            visibility_label: visibility_label.to_owned(),
            visibility_subject: visibility_subject.to_owned(),
            data_subject_id: if visibility_subject == "not_applicable" {
                "not_applicable".to_owned()
            } else {
                visibility_subject.to_owned()
            },
            provenance_kind: if context.actor_role() == "human_keeper" {
                "human_keeper_statement".to_owned()
            } else {
                "user_statement".to_owned()
            },
            provenance_reference: command.command_id.clone(),
            provenance_recorded_by: context.actor_id().to_owned(),
            correlation_id: command.correlation_id.clone(),
            causation_id: command.causation_id.clone(),
            trace_id: command.trace_id.clone(),
            audit: PolicyAuditDraft {
                actor_id: audit.actor_id.clone(),
                actor_origin: audit.actor_origin.clone(),
                authentication_reference: audit.authentication_reference.clone(),
                resource_type: audit.resource_type.clone(),
                resource_id: audit.resource_id.clone(),
                action: audit.action.clone(),
                requested_role: audit.requested_role.clone(),
                openfga_decision_id: audit.openfga_decision_id.clone(),
                openfga_policy_revision: audit.openfga_policy_revision.clone(),
                opa_decision_id: audit.opa_decision_id.clone(),
                opa_policy_revision: audit.opa_policy_revision.clone(),
            },
        }
    }

    fn runtime_context(context: &AuthorizedCoreApiContext) -> PlayerActionAuthorityContext {
        PlayerActionAuthorityContext {
            campaign_id: context.campaign_id().to_owned(),
            authority_mode: context.authority_mode().to_owned(),
            authority_owner: context.authority_owner().to_owned(),
            actor_id: context.actor_id().to_owned(),
            actor_role: context.actor_role().to_owned(),
        }
    }

    fn api_receipt(receipt: PlayerActionCommitReceipt) -> PlayerActionApiReceipt {
        PlayerActionApiReceipt {
            first_event_sequence: receipt.first_event_sequence,
            last_event_sequence: receipt.last_event_sequence,
            aggregate_version: receipt.aggregate_version,
            state: receipt.state,
            realtime_delta_id: receipt.realtime_delta_id,
        }
    }

    fn map_runtime_error(error: PlayerActionRuntimeError) -> CoreApiError {
        match error {
            PlayerActionRuntimeError::InvalidInput(field) => CoreApiError::InvalidInput(field),
            PlayerActionRuntimeError::Forbidden => CoreApiError::Forbidden,
            PlayerActionRuntimeError::NotFound => CoreApiError::Conflict("player_action_not_found"),
            PlayerActionRuntimeError::ToolExecutionFailed(_) => {
                CoreApiError::Unavailable("rules_tool_execution")
            }
            PlayerActionRuntimeError::PersistenceFailed(_) => {
                CoreApiError::Unavailable("player_action_repository")
            }
        }
    }
}

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
