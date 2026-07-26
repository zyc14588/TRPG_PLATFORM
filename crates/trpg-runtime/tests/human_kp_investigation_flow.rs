use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use trpg_runtime::session_runtime::{
    ConfirmablePlayerAction, ExecutedDiceRoll, HumanKpPlayerActionWorkflow, PendingPlayerAction,
    PlayerActionAuthorityContext, PlayerActionCommitReceipt, PlayerActionConfirmation,
    PlayerActionIntent, PlayerActionRuntimeError, PlayerActionRuntimeFuture,
    PlayerActionSubmission, PlayerActionToolExecutor, PlayerActionToolResult,
    PlayerActionWorkflowStore,
};

#[derive(Default)]
struct RecordingStore {
    pending: Mutex<Option<PendingPlayerAction>>,
    resolved: Mutex<Option<PlayerActionCommitReceipt>>,
    commits: AtomicUsize,
}

impl PlayerActionWorkflowStore for RecordingStore {
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
                character_sheet_json:
                    r#"{"skills":{"Library Use":70},"characteristics":{"power":60}}"#.to_owned(),
                character_sheet_version: 1,
            });
            Ok(PlayerActionCommitReceipt {
                first_event_sequence: 1,
                last_event_sequence: 1,
                aggregate_version: 1,
                state: "AWAITING_HUMAN_CONFIRMATION".to_owned(),
                realtime_delta_id: "delta_submit_action_001".to_owned(),
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
            assert!(matches!(
                result,
                PlayerActionToolResult::Investigation { .. }
            ));
            self.commits.fetch_add(1, Ordering::SeqCst);
            let receipt = PlayerActionCommitReceipt {
                first_event_sequence: 2,
                last_event_sequence: 5,
                aggregate_version: 5,
                state: "RESOLVED".to_owned(),
                realtime_delta_id: "delta_resolve_action_001".to_owned(),
            };
            *self.resolved.lock().unwrap() = Some(receipt.clone());
            Ok(receipt)
        })
    }
}

#[derive(Default)]
struct RecordingExecutor {
    calls: AtomicUsize,
    fail: AtomicBool,
}

impl PlayerActionToolExecutor for RecordingExecutor {
    fn execute(
        &self,
        pending: &PendingPlayerAction,
        _confirmation: &PlayerActionConfirmation,
    ) -> Result<PlayerActionToolResult, PlayerActionRuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(PlayerActionRuntimeError::ToolExecutionFailed(
                "injected_failure",
            ));
        }
        let PlayerActionIntent::Investigation {
            skill_name,
            clue_id,
            clue_importance,
            adjustment,
        } = &pending.intent
        else {
            return Err(PlayerActionRuntimeError::InvalidInput("action_kind"));
        };
        Ok(PlayerActionToolResult::Investigation {
            dice: ExecutedDiceRoll {
                roll_id: "dice_runtime_flow_001".to_owned(),
                target_value: 70,
                rolled_value: 42,
                success_level: "REGULAR".to_owned(),
                selected_tens_digit: 4,
                ones_digit: 2,
                adjustment: adjustment.clone(),
            },
            skill_name: skill_name.clone(),
            clue_record_id: "clue_record_runtime_flow_001".to_owned(),
            clue_id: clue_id.clone(),
            clue_importance: clue_importance.clone(),
            clue_outcome: "REVEALED".to_owned(),
            clue_cost: None,
            revealed_to_party: true,
        })
    }
}

fn player_context() -> PlayerActionAuthorityContext {
    PlayerActionAuthorityContext {
        campaign_id: "campaign_runtime_flow".to_owned(),
        authority_mode: "human_kp".to_owned(),
        authority_owner: "keeper_runtime_flow".to_owned(),
        actor_id: "player_runtime_flow".to_owned(),
        actor_role: "investigator".to_owned(),
    }
}

fn keeper_context() -> PlayerActionAuthorityContext {
    PlayerActionAuthorityContext {
        actor_id: "keeper_runtime_flow".to_owned(),
        actor_role: "human_keeper".to_owned(),
        ..player_context()
    }
}

fn submission() -> PlayerActionSubmission {
    PlayerActionSubmission {
        action_id: "action_runtime_flow_001".to_owned(),
        campaign_id: "campaign_runtime_flow".to_owned(),
        character_id: "character_runtime_flow".to_owned(),
        scene_id: "scene_runtime_flow".to_owned(),
        submitted_by: "player_runtime_flow".to_owned(),
        submitted_at_unix_ms: 2_300_000_000_000,
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
        action_id: "action_runtime_flow_001".to_owned(),
        campaign_id: "campaign_runtime_flow".to_owned(),
        decision_id: "decision_runtime_flow_001".to_owned(),
        tool_execution_id: "tool_execution_runtime_flow_001".to_owned(),
        resolved_at_unix_ms: 2_300_000_000_100,
    }
}

#[tokio::test]
async fn human_kp_confirmation_executes_before_commit_and_retry_does_not_reroll() {
    let store = Arc::new(RecordingStore::default());
    let executor = Arc::new(RecordingExecutor::default());
    let workflow = HumanKpPlayerActionWorkflow::new(Arc::clone(&store), Arc::clone(&executor));

    let pending = workflow
        .submit(&player_context(), &submission())
        .await
        .expect("authenticated investigator submits a pending action");
    assert_eq!(pending.state, "AWAITING_HUMAN_CONFIRMATION");
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0);
    assert_eq!(store.commits.load(Ordering::SeqCst), 0);

    let mut wrong_keeper = keeper_context();
    wrong_keeper.actor_id = "different_keeper".to_owned();
    assert_eq!(
        workflow.confirm(&wrong_keeper, &confirmation()).await,
        Err(PlayerActionRuntimeError::Forbidden)
    );
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0);

    executor.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        workflow.confirm(&keeper_context(), &confirmation()).await,
        Err(PlayerActionRuntimeError::ToolExecutionFailed(
            "injected_failure"
        ))
    );
    assert_eq!(store.commits.load(Ordering::SeqCst), 0);

    executor.fail.store(false, Ordering::SeqCst);
    let committed = workflow
        .confirm(&keeper_context(), &confirmation())
        .await
        .expect("human KP confirmation executes and commits");
    assert_eq!(committed.state, "RESOLVED");
    assert_eq!(executor.calls.load(Ordering::SeqCst), 2);
    assert_eq!(store.commits.load(Ordering::SeqCst), 1);

    let retry = workflow
        .confirm(&keeper_context(), &confirmation())
        .await
        .expect("exact retry returns durable result without a second roll");
    assert_eq!(retry, committed);
    assert_eq!(executor.calls.load(Ordering::SeqCst), 2);
    assert_eq!(store.commits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn ai_or_non_owner_cannot_confirm_a_human_kp_action() {
    let store = Arc::new(RecordingStore::default());
    let executor = Arc::new(RecordingExecutor::default());
    let workflow = HumanKpPlayerActionWorkflow::new(Arc::clone(&store), Arc::clone(&executor));
    workflow
        .submit(&player_context(), &submission())
        .await
        .unwrap();

    let mut ai = keeper_context();
    ai.actor_role = "ai_keeper_orchestrator".to_owned();
    assert_eq!(
        workflow.confirm(&ai, &confirmation()).await,
        Err(PlayerActionRuntimeError::Forbidden)
    );
    assert_eq!(executor.calls.load(Ordering::SeqCst), 0);
    assert_eq!(store.commits.load(Ordering::SeqCst), 0);
}
