use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use trpg_api::api_contracts::SubmitPlayerActionApiRequest;
use trpg_ruleset_coc7::character_combat_san_chase::{
    parse_scenario_yaml, Coc7CharacterSheet, ScenarioDocument,
};
use trpg_ruleset_coc7::dice_roll_contract::{
    server_roll_skill_check, DiceAdjustment, SuccessLevel,
};
use trpg_ruleset_coc7::investigation_clue_npc_time::{
    resolve_clue_check, ClueImportance, ClueOutcome,
};
use trpg_ruleset_coc7::sanity_madness_state_machine::{apply_sanity_loss, MadnessState};
use trpg_runtime::session_runtime::{
    ConfirmablePlayerAction, ExecutedDiceRoll, HumanKpPlayerActionWorkflow, PendingPlayerAction,
    PlayerActionAuthorityContext, PlayerActionCommitReceipt, PlayerActionConfirmation,
    PlayerActionIntent, PlayerActionRuntimeError, PlayerActionRuntimeFuture,
    PlayerActionSubmission, PlayerActionToolExecutor, PlayerActionToolResult,
    PlayerActionWorkflowStore,
};

const CAMPAIGN_ID: &str = "campaign_p07_vertical";
const KEEPER_ID: &str = "keeper_p07_vertical";
const PLAYER_ID: &str = "player_p07_vertical";
const ACTION_ID: &str = "action_p07_vertical";
const TUTORIAL: &str =
    include_str!("../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml");

#[derive(Default)]
struct TutorialStore {
    pending: Mutex<Option<PendingPlayerAction>>,
    resolved: Mutex<Option<PlayerActionCommitReceipt>>,
    committed_result: Mutex<Option<PlayerActionToolResult>>,
    commits: AtomicUsize,
}

impl PlayerActionWorkflowStore for TutorialStore {
    fn submit<'a>(
        &'a self,
        _context: &'a PlayerActionAuthorityContext,
        submission: &'a PlayerActionSubmission,
    ) -> PlayerActionRuntimeFuture<'a, PlayerActionCommitReceipt> {
        Box::pin(async move {
            *self.pending.lock().unwrap() = Some(PendingPlayerAction {
                action_id: submission.action_id.clone(),
                campaign_id: submission.campaign_id.clone(),
                character_id: submission.character_id.clone(),
                scene_id: submission.scene_id.clone(),
                submitted_by: submission.submitted_by.clone(),
                intent: submission.intent.clone(),
                character_sheet_json: serde_json::json!({
                    "name": "Evelyn Hart",
                    "age": 31,
                    "occupation": "Investigative journalist",
                    "era": "1920s",
                    "birthplace": "Brisbane",
                    "characteristics": {
                        "strength": 50,
                        "dexterity": 60,
                        "power": 65,
                        "constitution": 55,
                        "size": 50,
                        "appearance": 55,
                        "intelligence": 70,
                        "education": 75,
                        "luck": 60
                    },
                    "skills": {"Library Use": 70, "Psychology": 55},
                    "backstory_anchors": [
                        "Protects confidential sources",
                        "Distrusts official explanations"
                    ]
                })
                .to_string(),
                character_sheet_version: 1,
            });
            Ok(PlayerActionCommitReceipt {
                first_event_sequence: 1,
                last_event_sequence: 1,
                aggregate_version: 1,
                state: "AWAITING_HUMAN_CONFIRMATION".to_owned(),
                realtime_delta_id: "delta_p07_vertical_submit".to_owned(),
            })
        })
    }

    fn load_for_confirmation<'a>(
        &'a self,
        _context: &'a PlayerActionAuthorityContext,
        _confirmation: &'a PlayerActionConfirmation,
    ) -> PlayerActionRuntimeFuture<'a, ConfirmablePlayerAction> {
        Box::pin(async move {
            if let Some(receipt) = self.resolved.lock().unwrap().clone() {
                return Ok(ConfirmablePlayerAction::Resolved(receipt));
            }
            self.pending
                .lock()
                .unwrap()
                .clone()
                .map(ConfirmablePlayerAction::Pending)
                .ok_or(PlayerActionRuntimeError::NotFound)
        })
    }

    fn commit_execution<'a>(
        &'a self,
        _context: &'a PlayerActionAuthorityContext,
        _pending: &'a PendingPlayerAction,
        _confirmation: &'a PlayerActionConfirmation,
        result: &'a PlayerActionToolResult,
    ) -> PlayerActionRuntimeFuture<'a, PlayerActionCommitReceipt> {
        Box::pin(async move {
            let PlayerActionToolResult::Investigation {
                dice,
                clue_importance,
                clue_outcome,
                revealed_to_party,
                ..
            } = result
            else {
                return Err(PlayerActionRuntimeError::InvalidInput("tutorial_action"));
            };
            if !(1..=100).contains(&dice.rolled_value)
                || dice.target_value != 70
                || clue_importance != "CORE"
                || !matches!(clue_outcome.as_str(), "REVEALED" | "REVEALED_WITH_COST")
                || !revealed_to_party
            {
                return Err(PlayerActionRuntimeError::PersistenceFailed(
                    "invalid_rules_result",
                ));
            }
            self.commits.fetch_add(1, Ordering::SeqCst);
            *self.committed_result.lock().unwrap() = Some(result.clone());
            let receipt = PlayerActionCommitReceipt {
                first_event_sequence: 2,
                last_event_sequence: 5,
                aggregate_version: 5,
                state: "RESOLVED".to_owned(),
                realtime_delta_id: "delta_p07_vertical_resolved".to_owned(),
            };
            *self.resolved.lock().unwrap() = Some(receipt.clone());
            Ok(receipt)
        })
    }
}

#[derive(Default)]
struct RealCoc7TutorialExecutor {
    calls: AtomicUsize,
}

impl PlayerActionToolExecutor for RealCoc7TutorialExecutor {
    fn execute(
        &self,
        pending: &PendingPlayerAction,
        _confirmation: &PlayerActionConfirmation,
    ) -> Result<PlayerActionToolResult, PlayerActionRuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let PlayerActionIntent::Investigation {
            skill_name,
            clue_id,
            clue_importance,
            adjustment,
        } = &pending.intent
        else {
            return Err(PlayerActionRuntimeError::InvalidInput("tutorial_action"));
        };
        let sheet: Coc7CharacterSheet = serde_json::from_str(&pending.character_sheet_json)
            .map_err(|_| {
                PlayerActionRuntimeError::ToolExecutionFailed("character_sheet_invalid")
            })?;
        sheet.validate().map_err(|_| {
            PlayerActionRuntimeError::ToolExecutionFailed("character_sheet_invalid")
        })?;
        let target =
            *sheet
                .skills
                .get(skill_name)
                .ok_or(PlayerActionRuntimeError::ToolExecutionFailed(
                    "character_skill_missing",
                ))?;
        let adjustment_value = match adjustment.as_str() {
            "NONE" => DiceAdjustment::None,
            "BONUS" => DiceAdjustment::Bonus,
            "PENALTY" => DiceAdjustment::Penalty,
            _ => return Err(PlayerActionRuntimeError::InvalidInput("dice_adjustment")),
        };
        let generated = server_roll_skill_check(target, adjustment_value)
            .map_err(|_| PlayerActionRuntimeError::ToolExecutionFailed("server_rng_unavailable"))?;
        let outcome = generated.outcome();
        let succeeded = matches!(
            outcome.success_level,
            SuccessLevel::Critical
                | SuccessLevel::Extreme
                | SuccessLevel::Hard
                | SuccessLevel::Regular
        );
        let clue = resolve_clue_check(ClueImportance::Core, succeeded);
        let revealed_to_party = clue.outcome != ClueOutcome::NotFound;
        Ok(PlayerActionToolResult::Investigation {
            dice: ExecutedDiceRoll {
                roll_id: generated.roll_id().to_owned(),
                target_value: outcome.target,
                rolled_value: outcome.roll,
                success_level: success_level_name(outcome.success_level).to_owned(),
                selected_tens_digit: outcome.selected_tens_digit,
                ones_digit: outcome.ones_digit,
                adjustment: adjustment.clone(),
            },
            skill_name: skill_name.clone(),
            clue_record_id: format!("clue_result_{}", pending.action_id),
            clue_id: clue_id.clone(),
            clue_importance: clue_importance.clone(),
            clue_outcome: clue_outcome_name(clue.outcome).to_owned(),
            clue_cost: clue.cost.map(str::to_owned),
            revealed_to_party,
        })
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

fn player_context() -> PlayerActionAuthorityContext {
    PlayerActionAuthorityContext {
        campaign_id: CAMPAIGN_ID.to_owned(),
        authority_mode: "human_kp".to_owned(),
        authority_owner: KEEPER_ID.to_owned(),
        actor_id: PLAYER_ID.to_owned(),
        actor_role: "investigator".to_owned(),
    }
}

fn keeper_context() -> PlayerActionAuthorityContext {
    PlayerActionAuthorityContext {
        actor_id: KEEPER_ID.to_owned(),
        actor_role: "human_keeper".to_owned(),
        ..player_context()
    }
}

fn submission() -> PlayerActionSubmission {
    PlayerActionSubmission {
        action_id: ACTION_ID.to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        character_id: "character_p07_vertical".to_owned(),
        scene_id: "scene_archive_front".to_owned(),
        submitted_by: PLAYER_ID.to_owned(),
        submitted_at_unix_ms: 2_600_000_000_000,
        intent: PlayerActionIntent::Investigation {
            skill_name: "Library Use".to_owned(),
            clue_id: "clue_wrong_signature".to_owned(),
            clue_importance: "CORE".to_owned(),
            adjustment: "NONE".to_owned(),
        },
    }
}

fn confirmation() -> PlayerActionConfirmation {
    PlayerActionConfirmation {
        action_id: ACTION_ID.to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        decision_id: "decision_p07_vertical".to_owned(),
        tool_execution_id: "tool_execution_p07_vertical".to_owned(),
        resolved_at_unix_ms: 2_600_000_000_100,
    }
}

#[tokio::test]
async fn tutorial_human_kp_slice_reaches_a_core_clue_without_model_or_client_dice() {
    let validated = parse_scenario_yaml(TUTORIAL).expect("tutorial scenario validates");
    assert_eq!(validated.opening_scene_id, "scene_archive_front");
    let document: ScenarioDocument = serde_json::from_str(&validated.canonical_json).unwrap();
    let core_clue = document
        .clues
        .iter()
        .find(|clue| clue.id == "clue_wrong_signature")
        .expect("tutorial contains the investigation clue");
    assert_eq!(core_clue.clue_type, "core");
    assert_eq!(core_clue.visibility, "party_visible");

    let client_supplied_dice = serde_json::json!({
        "command": {
            "command_id": "command_p07_vertical",
            "idempotency_key": "idempotency_p07_vertical",
            "expected_version": 0,
            "correlation_id": "correlation_p07_vertical",
            "causation_id": "causation_p07_vertical",
            "trace_id": "trace_p07_vertical"
        },
        "campaign_id": CAMPAIGN_ID,
        "action_id": ACTION_ID,
        "character_id": "character_p07_vertical",
        "scene_id": "scene_archive_front",
        "submitted_at_unix_ms": 2_600_000_000_000_u64,
        "intent": {
            "kind": "INVESTIGATION",
            "skill_name": "Library Use",
            "clue_id": "clue_wrong_signature",
            "clue_importance": "CORE",
            "adjustment": "NONE",
            "rolled_value": 1
        }
    });
    assert!(
        serde_json::from_value::<SubmitPlayerActionApiRequest>(client_supplied_dice).is_err(),
        "the transport contract must not deserialize caller-selected dice"
    );

    let store = Arc::new(TutorialStore::default());
    let executor = Arc::new(RealCoc7TutorialExecutor::default());
    let workflow = HumanKpPlayerActionWorkflow::new(Arc::clone(&store), Arc::clone(&executor));
    let pending = workflow
        .submit(&player_context(), &submission())
        .await
        .expect("investigator submits an authenticated tutorial action");
    assert_eq!(pending.state, "AWAITING_HUMAN_CONFIRMATION");
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0);
    assert_eq!(store.commits.load(Ordering::SeqCst), 0);

    let mut ai_keeper = keeper_context();
    ai_keeper.actor_role = "ai_keeper_orchestrator".to_owned();
    assert_eq!(
        workflow.confirm(&ai_keeper, &confirmation()).await,
        Err(PlayerActionRuntimeError::Forbidden)
    );
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0);

    let resolved = workflow
        .confirm(&keeper_context(), &confirmation())
        .await
        .expect("authority owner confirmation executes the real rules service");
    assert_eq!(resolved.state, "RESOLVED");
    assert_eq!(executor.calls.load(Ordering::SeqCst), 1);
    assert_eq!(store.commits.load(Ordering::SeqCst), 1);
    let committed = store.committed_result.lock().unwrap().clone().unwrap();
    let PlayerActionToolResult::Investigation {
        dice,
        clue_outcome,
        revealed_to_party,
        ..
    } = committed
    else {
        panic!("tutorial must resolve through an investigation result");
    };
    assert!(dice.roll_id.starts_with("dice_"));
    assert!((1..=100).contains(&dice.rolled_value));
    assert!(matches!(
        clue_outcome.as_str(),
        "REVEALED" | "REVEALED_WITH_COST"
    ));
    assert!(revealed_to_party);

    let retried = workflow
        .confirm(&keeper_context(), &confirmation())
        .await
        .expect("exact confirmation retry uses the durable receipt");
    assert_eq!(retried, resolved);
    assert_eq!(executor.calls.load(Ordering::SeqCst), 1);
    assert_eq!(store.commits.load(Ordering::SeqCst), 1);
}

#[test]
fn tutorial_sanity_sequence_is_invariant_under_event_grouping() {
    let grouped = apply_sanity_loss(60, 12, 0, 60).unwrap();
    let first = apply_sanity_loss(60, 5, 0, 60).unwrap();
    let second = apply_sanity_loss(first.after, 7, first.day_loss, 60).unwrap();
    assert_eq!(grouped.after, second.after);
    assert_eq!(grouped.day_loss, second.day_loss);
    assert_eq!(grouped.indefinite_threshold, 12);
    assert_eq!(second.indefinite_threshold, 12);
    assert_eq!(grouped.state, MadnessState::IndefiniteInsanity);
    assert_eq!(second.state, MadnessState::IndefiniteInsanity);
}
