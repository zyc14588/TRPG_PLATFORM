use crate::runtime_state_machines::{
    append_runtime_event, EventStore, RuntimeEventPayload, RuntimeResult,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EntityId, EventEnvelope};

pub fn start_session<T: Clone>(
    store: &mut EventStore<RuntimeEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    session_id: impl Into<String>,
) -> RuntimeResult<EventEnvelope<RuntimeEventPayload>> {
    append_runtime_event(
        store,
        contract,
        command,
        "SessionStarted",
        RuntimeEventPayload::SessionStarted {
            session_id: EntityId::new(session_id)?,
        },
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DurableSessionState {
    Scheduled,
    Active,
    Paused,
    Ended,
}

impl DurableSessionState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "SCHEDULED",
            Self::Active => "ACTIVE",
            Self::Paused => "PAUSED",
            Self::Ended => "ENDED",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DurableSceneState {
    Active,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableScene {
    pub scene_id: EntityId,
    pub scene_key: String,
    pub state: DurableSceneState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSceneStateMachine {
    session_id: EntityId,
    state: DurableSessionState,
    active_scene: Option<DurableScene>,
    closed_scenes: Vec<DurableScene>,
    version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionSceneStateError {
    InvalidIdentifier,
    InvalidSceneKey,
    InvalidVersion,
    InvalidTransition {
        from: DurableSessionState,
        operation: &'static str,
    },
    ActiveSceneMissing,
    SceneIdentityConflict,
    CorruptRecoveryState,
}

impl std::fmt::Display for SessionSceneStateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIdentifier => formatter.write_str("RUNTIME_SESSION_INVALID_IDENTIFIER"),
            Self::InvalidSceneKey => formatter.write_str("RUNTIME_SCENE_KEY_INVALID"),
            Self::InvalidVersion => formatter.write_str("RUNTIME_SESSION_VERSION_INVALID"),
            Self::InvalidTransition { from, operation } => write!(
                formatter,
                "RUNTIME_SESSION_INVALID_TRANSITION:{}:{operation}",
                from.as_str()
            ),
            Self::ActiveSceneMissing => formatter.write_str("RUNTIME_ACTIVE_SCENE_MISSING"),
            Self::SceneIdentityConflict => formatter.write_str("RUNTIME_SCENE_IDENTITY_CONFLICT"),
            Self::CorruptRecoveryState => formatter.write_str("RUNTIME_SESSION_RECOVERY_CORRUPT"),
        }
    }
}

impl std::error::Error for SessionSceneStateError {}

impl SessionSceneStateMachine {
    pub fn scheduled(session_id: impl Into<String>) -> Result<Self, SessionSceneStateError> {
        Ok(Self {
            session_id: EntityId::new(session_id)
                .map_err(|_| SessionSceneStateError::InvalidIdentifier)?,
            state: DurableSessionState::Scheduled,
            active_scene: None,
            closed_scenes: Vec::new(),
            version: 0,
        })
    }

    pub fn recover(
        session_id: impl Into<String>,
        state: DurableSessionState,
        active_scene: Option<(String, String, DurableSceneState)>,
        closed_scenes: Vec<(String, String)>,
        version: u64,
    ) -> Result<Self, SessionSceneStateError> {
        if version == 0 || state == DurableSessionState::Scheduled {
            return Err(SessionSceneStateError::InvalidVersion);
        }
        let active_scene = active_scene
            .map(|(scene_id, scene_key, scene_state)| {
                Ok(DurableScene {
                    scene_id: EntityId::new(scene_id)
                        .map_err(|_| SessionSceneStateError::InvalidIdentifier)?,
                    scene_key: validate_scene_key(scene_key)?,
                    state: scene_state,
                })
            })
            .transpose()?;
        let closed_scenes = closed_scenes
            .into_iter()
            .map(|(scene_id, scene_key)| {
                Ok(DurableScene {
                    scene_id: EntityId::new(scene_id)
                        .map_err(|_| SessionSceneStateError::InvalidIdentifier)?,
                    scene_key: validate_scene_key(scene_key)?,
                    state: DurableSceneState::Closed,
                })
            })
            .collect::<Result<Vec<_>, SessionSceneStateError>>()?;
        let active_shape_valid = match state {
            DurableSessionState::Active | DurableSessionState::Paused => active_scene
                .as_ref()
                .is_some_and(|scene| scene.state == DurableSceneState::Active),
            DurableSessionState::Ended => active_scene
                .as_ref()
                .is_some_and(|scene| scene.state == DurableSceneState::Closed),
            DurableSessionState::Scheduled => false,
        };
        if !active_shape_valid {
            return Err(SessionSceneStateError::CorruptRecoveryState);
        }
        let machine = Self {
            session_id: EntityId::new(session_id)
                .map_err(|_| SessionSceneStateError::InvalidIdentifier)?,
            state,
            active_scene,
            closed_scenes,
            version,
        };
        machine.validate_unique_scene_identity()?;
        Ok(machine)
    }

    pub fn start(
        &mut self,
        scene_id: impl Into<String>,
        scene_key: impl Into<String>,
    ) -> Result<(), SessionSceneStateError> {
        if self.state != DurableSessionState::Scheduled {
            return Err(SessionSceneStateError::InvalidTransition {
                from: self.state,
                operation: "START",
            });
        }
        self.active_scene = Some(DurableScene {
            scene_id: EntityId::new(scene_id)
                .map_err(|_| SessionSceneStateError::InvalidIdentifier)?,
            scene_key: validate_scene_key(scene_key)?,
            state: DurableSceneState::Active,
        });
        self.state = DurableSessionState::Active;
        self.advance_version()
    }

    pub fn pause(&mut self) -> Result<(), SessionSceneStateError> {
        if self.state != DurableSessionState::Active {
            return Err(SessionSceneStateError::InvalidTransition {
                from: self.state,
                operation: "PAUSE",
            });
        }
        self.state = DurableSessionState::Paused;
        self.advance_version()
    }

    pub fn resume(&mut self) -> Result<(), SessionSceneStateError> {
        if self.state != DurableSessionState::Paused {
            return Err(SessionSceneStateError::InvalidTransition {
                from: self.state,
                operation: "RESUME",
            });
        }
        self.state = DurableSessionState::Active;
        self.advance_version()
    }

    pub fn switch_scene(
        &mut self,
        scene_id: impl Into<String>,
        scene_key: impl Into<String>,
    ) -> Result<(), SessionSceneStateError> {
        if self.state != DurableSessionState::Active {
            return Err(SessionSceneStateError::InvalidTransition {
                from: self.state,
                operation: "SWITCH_SCENE",
            });
        }
        let next = DurableScene {
            scene_id: EntityId::new(scene_id)
                .map_err(|_| SessionSceneStateError::InvalidIdentifier)?,
            scene_key: validate_scene_key(scene_key)?,
            state: DurableSceneState::Active,
        };
        if self
            .active_scene
            .iter()
            .chain(self.closed_scenes.iter())
            .any(|scene| scene.scene_id == next.scene_id || scene.scene_key == next.scene_key)
        {
            return Err(SessionSceneStateError::SceneIdentityConflict);
        }
        let mut previous = self
            .active_scene
            .take()
            .ok_or(SessionSceneStateError::ActiveSceneMissing)?;
        previous.state = DurableSceneState::Closed;
        self.closed_scenes.push(previous);
        self.active_scene = Some(next);
        self.advance_version()
    }

    pub fn end(&mut self) -> Result<(), SessionSceneStateError> {
        if !matches!(
            self.state,
            DurableSessionState::Active | DurableSessionState::Paused
        ) {
            return Err(SessionSceneStateError::InvalidTransition {
                from: self.state,
                operation: "END",
            });
        }
        let active = self
            .active_scene
            .as_mut()
            .ok_or(SessionSceneStateError::ActiveSceneMissing)?;
        active.state = DurableSceneState::Closed;
        self.state = DurableSessionState::Ended;
        self.advance_version()
    }

    fn advance_version(&mut self) -> Result<(), SessionSceneStateError> {
        self.version = self
            .version
            .checked_add(1)
            .ok_or(SessionSceneStateError::InvalidVersion)?;
        Ok(())
    }

    fn validate_unique_scene_identity(&self) -> Result<(), SessionSceneStateError> {
        let mut ids = std::collections::HashSet::new();
        let mut keys = std::collections::HashSet::new();
        for scene in self.active_scene.iter().chain(self.closed_scenes.iter()) {
            if !ids.insert(scene.scene_id.as_str()) || !keys.insert(scene.scene_key.as_str()) {
                return Err(SessionSceneStateError::SceneIdentityConflict);
            }
        }
        Ok(())
    }

    pub fn session_id(&self) -> &EntityId {
        &self.session_id
    }

    pub const fn state(&self) -> DurableSessionState {
        self.state
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn active_scene(&self) -> Option<&DurableScene> {
        self.active_scene.as_ref()
    }

    pub fn closed_scenes(&self) -> &[DurableScene] {
        &self.closed_scenes
    }
}

fn validate_scene_key(scene_key: impl Into<String>) -> Result<String, SessionSceneStateError> {
    let scene_key = scene_key.into();
    if scene_key.trim().is_empty() || scene_key.len() > 256 {
        Err(SessionSceneStateError::InvalidSceneKey)
    } else {
        Ok(scene_key)
    }
}

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type PlayerActionRuntimeFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, PlayerActionRuntimeError>> + Send + 'a>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerActionRuntimeError {
    InvalidInput(&'static str),
    Forbidden,
    NotFound,
    ToolExecutionFailed(&'static str),
    PersistenceFailed(&'static str),
}

impl std::fmt::Display for PlayerActionRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(field) => {
                write!(formatter, "PLAYER_ACTION_INPUT_INVALID:{field}")
            }
            Self::Forbidden => formatter.write_str("PLAYER_ACTION_FORBIDDEN"),
            Self::NotFound => formatter.write_str("PLAYER_ACTION_NOT_FOUND"),
            Self::ToolExecutionFailed(reason) => {
                write!(formatter, "PLAYER_ACTION_TOOL_EXECUTION_FAILED:{reason}")
            }
            Self::PersistenceFailed(reason) => {
                write!(formatter, "PLAYER_ACTION_PERSISTENCE_FAILED:{reason}")
            }
        }
    }
}

impl std::error::Error for PlayerActionRuntimeError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerActionAuthorityContext {
    pub campaign_id: String,
    pub authority_mode: String,
    pub authority_owner: String,
    pub actor_id: String,
    pub actor_role: String,
}

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
