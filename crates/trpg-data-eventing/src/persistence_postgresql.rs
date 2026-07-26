crate::define_data_event_module!(
    PersistencePostgresqlCommand,
    PersistencePostgresqlOperation,
    append_persistence_postgresql_event,
    "persistence_postgresql",
    "PersistencePostgresqlRecorded",
    "data_eventing.persistence_postgresql.event_schema",
    crate::DataEventOperation::EventStoreAppend,
    ["event_store", "event_outbox", "projection_checkpoint"]
);

crate::define_data_event_artifacts!(
    PersistencePostgresqlService,
    PersistencePostgresqlRepository,
    PersistencePostgresqlEvent,
    PersistencePostgresqlError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const STORAGE_TABLES: &[&str] = &["event_store", "event_outbox", "projection_checkpoint"];

pub fn required_storage_tables() -> &'static [&'static str] {
    STORAGE_TABLES
}

use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, TimeZone, Utc};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Postgres, Row, Transaction};
pub use trpg_domain_core::domain_entities_value_objects::MembershipRole;
use trpg_domain_core::domain_entities_value_objects::{
    CampaignAggregate, CampaignInvite, Character, CharacterState, CoreDomainEvent, CoreEntityError,
    Room, Session, SessionState, UserId,
};
use trpg_shared_kernel::{EntityId, EventActorOriginWire};

use crate::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, CanonicalProjectionTarget, CanonicalReplayEvent,
    CanonicalStoreError, PersistedCommit, PolicyAuditDraft, PostgresCanonicalStore,
};

const CORE_EVENT_SCHEMA_VERSION: u16 = CoreDomainEvent::SCHEMA_VERSION;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreCommandMetadata {
    pub commit_id: String,
    pub command_id: String,
    pub idempotency_key: String,
    pub expected_version: i64,
    /// Authenticated user or trusted service that requested the business
    /// operation. This principal is used for domain authorization and fact
    /// provenance, but never receives the canonical Event Store capability.
    pub requesting_actor_id: String,
    pub requesting_actor_role: String,
    /// The workflow decision principal that owns the formal write.
    pub authenticated_actor_id: String,
    pub authenticated_actor_role: String,
    pub authenticated_actor_origin: EventActorOriginWire,
    pub authority_mode: String,
    pub authority_contract_version: i64,
    pub authority_contract_id: String,
    pub authority_owner: String,
    pub visibility_label: String,
    pub visibility_subject: String,
    pub data_subject_id: String,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub trace_id: String,
    pub audit: PolicyAuditDraft,
}

impl CoreCommandMetadata {
    fn to_draft(
        &self,
        campaign_id: &str,
        stream_id: &str,
        resource_type: &str,
        _action: &str,
        event: &CoreDomainEvent,
        projection_targets: Vec<CanonicalProjectionTarget>,
    ) -> Result<AtomicCommitDraft, CoreDomainRepositoryError> {
        event.validate_schema_version()?;
        if self.requesting_actor_id.trim().is_empty()
            || self.requesting_actor_role.trim().is_empty()
            || self.provenance_recorded_by != self.requesting_actor_id
            || self.authenticated_actor_role != "workflow"
            || !matches!(
                self.authenticated_actor_origin,
                EventActorOriginWire::Workload { ref role }
                    if role == "workflow_engine"
            )
            || self.audit.actor_id != self.authenticated_actor_id
            || self.audit.actor_origin != "workload"
            || self.audit.authentication_reference != self.authenticated_actor_id
            || self.audit.resource_id != stream_id
            || self.audit.resource_type != resource_type
            || self.audit.action != "write_official_state"
            || self.audit.requested_role != "workflow"
        {
            return Err(CoreDomainRepositoryError::PolicyEvidenceMismatch);
        }
        let payload_json =
            serde_json::to_string(event).map_err(|_| CoreDomainRepositoryError::Serialization)?;
        Ok(AtomicCommitDraft {
            commit_id: self.commit_id.clone(),
            campaign_id: campaign_id.to_owned(),
            stream_id: stream_id.to_owned(),
            idempotency_key: self.idempotency_key.clone(),
            expected_version: self.expected_version,
            command_id: self.command_id.clone(),
            authenticated_actor_id: self.authenticated_actor_id.clone(),
            authenticated_actor_role: self.authenticated_actor_role.clone(),
            authenticated_actor_origin: self.authenticated_actor_origin.clone(),
            authority_mode: self.authority_mode.clone(),
            authority_contract_version: self.authority_contract_version,
            authority_contract_id: self.authority_contract_id.clone(),
            authority_owner: self.authority_owner.clone(),
            visibility_label: self.visibility_label.clone(),
            visibility_subject: self.visibility_subject.clone(),
            data_subject_id: self.data_subject_id.clone(),
            provenance_kind: self.provenance_kind.clone(),
            provenance_reference: self.provenance_reference.clone(),
            provenance_recorded_by: self.provenance_recorded_by.clone(),
            correlation_id: self.correlation_id.clone(),
            causation_id: self.causation_id.clone(),
            trace_id: self.trace_id.clone(),
            events: vec![CanonicalEventDraft {
                event_type: event.event_type().to_owned(),
                payload_json,
                projection_targets,
            }],
            audit: self.audit.clone(),
        })
    }

    fn to_player_action_draft(
        &self,
        campaign_id: &str,
        action_id: &str,
        events: Vec<CanonicalEventDraft>,
    ) -> Result<AtomicCommitDraft, CoreDomainRepositoryError> {
        if events.is_empty()
            || self.requesting_actor_id.trim().is_empty()
            || self.requesting_actor_role.trim().is_empty()
            || self.provenance_recorded_by != self.requesting_actor_id
            || self.authenticated_actor_role != "workflow"
            || !matches!(
                self.authenticated_actor_origin,
                EventActorOriginWire::Workload { ref role }
                    if role == "workflow_engine"
            )
            || self.audit.actor_id != self.authenticated_actor_id
            || self.audit.actor_origin != "workload"
            || self.audit.authentication_reference != self.authenticated_actor_id
            || self.audit.resource_id != action_id
            || self.audit.resource_type != "player_action"
            || self.audit.action != "write_official_state"
            || self.audit.requested_role != "workflow"
        {
            return Err(CoreDomainRepositoryError::PolicyEvidenceMismatch);
        }
        Ok(AtomicCommitDraft {
            commit_id: self.commit_id.clone(),
            campaign_id: campaign_id.to_owned(),
            stream_id: action_id.to_owned(),
            idempotency_key: self.idempotency_key.clone(),
            expected_version: self.expected_version,
            command_id: self.command_id.clone(),
            authenticated_actor_id: self.authenticated_actor_id.clone(),
            authenticated_actor_role: self.authenticated_actor_role.clone(),
            authenticated_actor_origin: self.authenticated_actor_origin.clone(),
            authority_mode: self.authority_mode.clone(),
            authority_contract_version: self.authority_contract_version,
            authority_contract_id: self.authority_contract_id.clone(),
            authority_owner: self.authority_owner.clone(),
            visibility_label: self.visibility_label.clone(),
            visibility_subject: self.visibility_subject.clone(),
            data_subject_id: self.data_subject_id.clone(),
            provenance_kind: self.provenance_kind.clone(),
            provenance_reference: self.provenance_reference.clone(),
            provenance_recorded_by: self.provenance_recorded_by.clone(),
            correlation_id: self.correlation_id.clone(),
            causation_id: self.causation_id.clone(),
            trace_id: self.trace_id.clone(),
            events,
            audit: self.audit.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityContractSnapshot {
    pub contract_id: String,
    pub authority_mode: String,
    pub authority_owner: String,
    pub ruleset_version: String,
    pub house_rules_version: String,
    pub scenario_version: String,
    pub prompt_version: String,
    pub agent_pack_version: String,
    pub tool_schema_version: String,
    pub safety_profile_version: String,
    pub ai_provider_snapshot: String,
    pub model_route_snapshot: String,
    pub character_sheet_template_version: String,
}

impl AuthorityContractSnapshot {
    fn validate(
        &self,
        campaign_id: &str,
        metadata: &CoreCommandMetadata,
    ) -> Result<(), CoreDomainRepositoryError> {
        let expected_wire_mode = match self.authority_mode.as_str() {
            "HUMAN_KP" => "human_kp",
            "AI_KP" => "ai_kp",
            _ => return Err(CoreDomainRepositoryError::InvalidInput("authority_mode")),
        };
        let required = [
            self.contract_id.as_str(),
            self.authority_owner.as_str(),
            self.ruleset_version.as_str(),
            self.house_rules_version.as_str(),
            self.scenario_version.as_str(),
            self.prompt_version.as_str(),
            self.agent_pack_version.as_str(),
            self.tool_schema_version.as_str(),
            self.safety_profile_version.as_str(),
            self.ai_provider_snapshot.as_str(),
            self.model_route_snapshot.as_str(),
            self.character_sheet_template_version.as_str(),
        ];
        if required.iter().any(|value| value.trim().is_empty())
            || expected_wire_mode != metadata.authority_mode
            || self.contract_id != metadata.authority_contract_id
            || self.authority_owner != metadata.authority_owner
            || metadata.authority_contract_version != 1
            || campaign_id.trim().is_empty()
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "authority_contract",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateCampaignRequest {
    pub campaign_id: String,
    pub owner_user_id: String,
    pub title: String,
    pub room_id: String,
    pub room_name: String,
    pub created_at_unix_ms: u64,
    pub authority: AuthorityContractSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueInviteRequest {
    pub invite_id: String,
    pub campaign_id: String,
    pub invited_user_id: String,
    pub role: MembershipRole,
    pub expires_at_unix_ms: u64,
}

pub struct IssuedCampaignInvite {
    pub invite_id: String,
    pub raw_token: String,
    pub expires_at_unix_ms: u64,
    pub persisted: PersistedCommit,
}

impl fmt::Debug for IssuedCampaignInvite {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IssuedCampaignInvite")
            .field("invite_id", &self.invite_id)
            .field("raw_token", &"[REDACTED]")
            .field("expires_at_unix_ms", &self.expires_at_unix_ms)
            .field("persisted", &self.persisted)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptInviteRequest {
    pub campaign_id: String,
    pub invite_id: String,
    pub accepting_user_id: String,
    pub raw_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateCharacterRequest {
    pub character_id: String,
    pub campaign_id: String,
    pub owner_user_id: String,
    pub display_name: String,
    pub sheet_version_id: String,
    pub sheet_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum PlayerActionIntentRecord {
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

impl PlayerActionIntentRecord {
    fn kind_name(&self) -> &'static str {
        match self {
            Self::Investigation { .. } => "INVESTIGATION",
            Self::SanityCheck { .. } => "SANITY_CHECK",
        }
    }

    fn validate(&self) -> Result<(), CoreDomainRepositoryError> {
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
                    return Err(CoreDomainRepositoryError::InvalidInput(
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
                    return Err(CoreDomainRepositoryError::InvalidInput("sanity_intent"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmitPlayerActionRequest {
    pub action_id: String,
    pub campaign_id: String,
    pub character_id: String,
    pub scene_id: String,
    pub submitted_by: String,
    pub submitted_at_unix_ms: u64,
    pub intent: PlayerActionIntentRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingPlayerActionRecord {
    pub action_id: String,
    pub campaign_id: String,
    pub character_id: String,
    pub scene_id: String,
    pub submitted_by: String,
    pub intent: PlayerActionIntentRecord,
    pub character_sheet_json: String,
    pub character_sheet_version: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerActionHeader {
    pub action_kind: String,
    pub submitted_by: String,
    pub state: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerActionDiceRecord {
    pub roll_id: String,
    pub target_value: u8,
    pub rolled_value: u8,
    pub success_level: String,
    pub selected_tens_digit: u8,
    pub ones_digit: u8,
    pub adjustment: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvestigationExecutionRecord {
    pub action_id: String,
    pub campaign_id: String,
    pub character_id: String,
    pub decision_id: String,
    pub tool_execution_id: String,
    pub confirmed_by: String,
    pub resolved_at_unix_ms: u64,
    pub dice: PlayerActionDiceRecord,
    pub skill_name: String,
    pub clue_record_id: String,
    pub clue_id: String,
    pub clue_importance: String,
    pub clue_outcome: String,
    pub clue_cost: Option<String>,
    pub revealed_to_party: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanityExecutionRecord {
    pub action_id: String,
    pub campaign_id: String,
    pub character_id: String,
    pub decision_id: String,
    pub tool_execution_id: String,
    pub confirmed_by: String,
    pub resolved_at_unix_ms: u64,
    pub dice: PlayerActionDiceRecord,
    pub sanity_event_id: String,
    pub sheet_version_id: String,
    pub day_key: String,
    pub day_start_sanity: u8,
    pub sanity_before: u8,
    pub sanity_after: u8,
    pub sanity_loss: u8,
    pub day_loss: u8,
    pub indefinite_threshold: u8,
    pub madness_state: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportScenarioRequest {
    pub scenario_id: String,
    pub campaign_id: String,
    pub ruleset_id: String,
    pub format_version: String,
    pub content_hash: String,
    pub document_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartSessionRequest {
    pub session_id: String,
    pub campaign_id: String,
    pub room_id: String,
    pub scenario_id: String,
    pub scene_id: String,
    pub scene_key: String,
    pub scene_name: String,
    pub started_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchSceneRequest {
    pub session_id: String,
    pub campaign_id: String,
    pub next_scene_id: String,
    pub next_scene_key: String,
    pub next_scene_name: String,
    pub switched_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionProjectionRebuildReport {
    pub campaign_id: String,
    pub replayed_events: usize,
    pub restored_sessions: i64,
    pub restored_scenes: i64,
    pub last_event_sequence: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordCampaignForkRequest {
    pub fork_id: String,
    pub parent_campaign_id: String,
    pub child_campaign_id: String,
    pub source_session_id: String,
    pub snapshot_hash: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestReconsiderationRequest {
    pub reconsideration_id: String,
    pub campaign_id: String,
    pub original_event_sequence: i64,
    pub requested_by: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewReconsiderationRequest {
    pub reconsideration_id: String,
    pub campaign_id: String,
    pub review_event_id: String,
    pub resolved: bool,
    pub resolution: String,
}

#[derive(Debug)]
pub enum CoreDomainRepositoryError {
    InvalidInput(&'static str),
    Domain(CoreEntityError),
    Canonical(CanonicalStoreError),
    Database(&'static str),
    Serialization,
    NotFound(&'static str),
    Forbidden,
    PolicyEvidenceMismatch,
    Integrity(&'static str),
    ConcurrentStart,
}

impl fmt::Display for CoreDomainRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(field) => write!(formatter, "CORE_INPUT_INVALID:{field}"),
            Self::Domain(error) => error.fmt(formatter),
            Self::Canonical(error) => error.fmt(formatter),
            Self::Database(operation) => write!(formatter, "CORE_DATABASE_ERROR:{operation}"),
            Self::Serialization => formatter.write_str("CORE_SERIALIZATION_ERROR"),
            Self::NotFound(entity) => write!(formatter, "CORE_NOT_FOUND:{entity}"),
            Self::Forbidden => formatter.write_str("CORE_FORBIDDEN"),
            Self::PolicyEvidenceMismatch => formatter.write_str("CORE_POLICY_EVIDENCE_MISMATCH"),
            Self::Integrity(reason) => write!(formatter, "CORE_INTEGRITY_ERROR:{reason}"),
            Self::ConcurrentStart => formatter.write_str("CORE_SESSION_ALREADY_LIVE"),
        }
    }
}

impl Error for CoreDomainRepositoryError {}

impl From<CoreEntityError> for CoreDomainRepositoryError {
    fn from(error: CoreEntityError) -> Self {
        Self::Domain(error)
    }
}

impl From<CanonicalStoreError> for CoreDomainRepositoryError {
    fn from(error: CanonicalStoreError) -> Self {
        Self::Canonical(error)
    }
}

pub trait CoreDomainClock: fmt::Debug + Send + Sync {
    fn now_unix_ms(&self) -> Result<u64, CoreDomainRepositoryError>;
}

#[derive(Debug)]
struct SystemCoreDomainClock;

impl CoreDomainClock for SystemCoreDomainClock {
    fn now_unix_ms(&self) -> Result<u64, CoreDomainRepositoryError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| CoreDomainRepositoryError::Integrity("trusted_clock_before_epoch"))?;
        u64::try_from(elapsed.as_millis())
            .map_err(|_| CoreDomainRepositoryError::Integrity("trusted_clock_out_of_range"))
    }
}

#[derive(Clone)]
pub struct CoreDomainRepository {
    primary: PgPool,
    canonical: PostgresCanonicalStore,
    clock: Arc<dyn CoreDomainClock>,
}

impl fmt::Debug for CoreDomainRepository {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CoreDomainRepository")
            .field("primary", &"[POSTGRESQL POOL]")
            .field("canonical", &self.canonical)
            .field("clock", &"[TRUSTED SERVER CLOCK]")
            .finish()
    }
}

impl CoreDomainRepository {
    /// Composes the API-owned projection pool with the separately credentialed
    /// canonical store. Production must not reuse the canonical service role
    /// for identity or business-table reads/writes.
    pub fn new(projection_pool: PgPool, canonical: PostgresCanonicalStore) -> Self {
        Self::new_with_clock(projection_pool, canonical, Arc::new(SystemCoreDomainClock))
    }

    pub fn new_with_clock(
        projection_pool: PgPool,
        canonical: PostgresCanonicalStore,
        clock: Arc<dyn CoreDomainClock>,
    ) -> Self {
        Self {
            primary: projection_pool,
            canonical,
            clock,
        }
    }

    pub async fn connect(
        database_url: &str,
        canonical: PostgresCanonicalStore,
    ) -> Result<Self, CoreDomainRepositoryError> {
        let options = PgConnectOptions::from_str(database_url)
            .map_err(|_| CoreDomainRepositoryError::Database("parse_projection_database_url"))?;
        let primary = PgPoolOptions::new()
            .max_connections(20)
            .connect_with(options)
            .await
            .map_err(database_error("connect_projection_database"))?;
        Ok(Self::new(primary, canonical))
    }

    pub fn primary_pool(&self) -> PgPool {
        self.primary.clone()
    }

    pub async fn character_owner_user_id(
        &self,
        campaign_id: &str,
        character_id: &str,
    ) -> Result<String, CoreDomainRepositoryError> {
        let row = sqlx::query(
            "SELECT campaign_id, owner_user_id \
             FROM public.characters WHERE character_id = $1",
        )
        .bind(character_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_character_owner"))?
        .ok_or(CoreDomainRepositoryError::NotFound("character"))?;
        if row.get::<String, _>("campaign_id") != campaign_id {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        Ok(row.get("owner_user_id"))
    }

    async fn set_projection_capability(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        commit_id: &str,
        operation: &'static str,
    ) -> Result<(), CoreDomainRepositoryError> {
        let capability = self
            .canonical
            .derive_core_projection_capability(commit_id)?;
        sqlx::query("SELECT set_config('trpg.projection_capability', $1, TRUE)")
            .bind(capability.as_str())
            .execute(&mut **transaction)
            .await
            .map_err(database_error(operation))?;
        Ok(())
    }

    async fn begin_projection_transaction<'a>(
        &'a self,
        commit_id: &str,
        operation: &'static str,
    ) -> Result<Transaction<'a, Postgres>, CoreDomainRepositoryError> {
        let mut transaction = self
            .primary
            .begin()
            .await
            .map_err(database_error(operation))?;
        self.set_projection_capability(&mut transaction, commit_id, "set_projection_capability")
            .await?;
        Ok(transaction)
    }

    async fn commit_event(
        &self,
        metadata: &CoreCommandMetadata,
        campaign_id: &str,
        stream_id: &str,
        route: (&str, &str),
        event: &CoreDomainEvent,
        projection_targets: Vec<CanonicalProjectionTarget>,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let draft = metadata.to_draft(
            campaign_id,
            stream_id,
            route.0,
            route.1,
            event,
            projection_targets,
        )?;
        self.canonical.commit(&draft).await.map_err(Into::into)
    }
}

fn player_action_event(
    event_type: &str,
    payload: serde_json::Value,
    projection_targets: Vec<CanonicalProjectionTarget>,
) -> Result<CanonicalEventDraft, CoreDomainRepositoryError> {
    Ok(CanonicalEventDraft {
        event_type: event_type.to_owned(),
        payload_json: serde_json::to_string(&payload)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?,
        projection_targets,
    })
}

fn canonical_success_level(
    roll: u8,
    target: u8,
) -> Result<&'static str, CoreDomainRepositoryError> {
    if !(1..=100).contains(&roll) || target > 100 {
        return Err(CoreDomainRepositoryError::InvalidInput("dice_range"));
    }
    Ok(if roll == 1 {
        "CRITICAL"
    } else if (target < 50 && roll >= 96) || (target >= 50 && roll == 100) {
        "FUMBLE"
    } else if roll <= target / 5 {
        "EXTREME"
    } else if roll <= target / 2 {
        "HARD"
    } else if roll <= target {
        "REGULAR"
    } else {
        "FAILURE"
    })
}

fn validate_server_dice_record(
    dice: &PlayerActionDiceRecord,
    expected_adjustment: &str,
) -> Result<(), CoreDomainRepositoryError> {
    let reconstructed = if dice.selected_tens_digit == 0 && dice.ones_digit == 0 {
        100
    } else {
        dice.selected_tens_digit * 10 + dice.ones_digit
    };
    if EntityId::new(&dice.roll_id).is_err()
        || dice.target_value == 0
        || dice.selected_tens_digit > 9
        || dice.ones_digit > 9
        || reconstructed != dice.rolled_value
        || dice.adjustment != expected_adjustment
        || canonical_success_level(dice.rolled_value, dice.target_value)? != dice.success_level
    {
        return Err(CoreDomainRepositoryError::InvalidInput(
            "server_dice_record",
        ));
    }
    Ok(())
}

impl CoreDomainRepository {
    async fn campaign_invite_acceptance_projection_id(
        &self,
        projection: &serde_json::Value,
    ) -> Result<String, CoreDomainRepositoryError> {
        sqlx::query_scalar("SELECT core_domain.campaign_invite_acceptance_projection_id($1::JSONB)")
            .bind(sqlx::types::Json(projection.clone()))
            .fetch_one(&self.primary)
            .await
            .map_err(database_error(
                "derive_campaign_invite_acceptance_projection_id",
            ))
    }

    async fn player_action_projection_id(
        &self,
        projection: &serde_json::Value,
    ) -> Result<String, CoreDomainRepositoryError> {
        sqlx::query_scalar("SELECT core_domain.player_action_projection_id($1::JSONB)")
            .bind(sqlx::types::Json(projection.clone()))
            .fetch_one(&self.primary)
            .await
            .map_err(database_error("derive_player_action_projection_id"))
    }

    async fn verify_player_action_commit(
        &self,
        metadata: &CoreCommandMetadata,
        action_id: &str,
        expected_state: &str,
        persisted: &PersistedCommit,
    ) -> Result<(), CoreDomainRepositoryError> {
        let valid: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.player_actions AS action
                  JOIN public.formal_commits AS formal
                    ON formal.commit_id = $1
                  JOIN public.event_store AS event
                    ON event.sequence BETWEEN
                       formal.first_event_sequence AND formal.last_event_sequence
                 WHERE action.action_id = $2
                   AND action.state = $3
                   AND formal.idempotency_key = $4
                   AND event.command_id = $5
                   AND formal.status = 'committed'
                   AND event.campaign_id = action.campaign_id
                   AND event.stream_id = action.action_id
                   AND event.integrity_status = 'verified_hmac'
                   AND event.event_integrity_version = 3
                   AND action.last_event_sequence BETWEEN
                       formal.first_event_sequence AND formal.last_event_sequence
            )
            "#,
        )
        .bind(&metadata.commit_id)
        .bind(action_id)
        .bind(expected_state)
        .bind(&metadata.idempotency_key)
        .bind(&metadata.command_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("verify_player_action_commit"))?;
        if !valid || persisted.commit_id != metadata.commit_id {
            return Err(CoreDomainRepositoryError::Integrity(
                "player_action_commit_projection_mismatch",
            ));
        }
        Ok(())
    }

    pub async fn submit_player_action(
        &self,
        metadata: &CoreCommandMetadata,
        request: &SubmitPlayerActionRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        request.intent.validate()?;
        for value in [
            request.action_id.as_str(),
            request.campaign_id.as_str(),
            request.character_id.as_str(),
            request.scene_id.as_str(),
            request.submitted_by.as_str(),
        ] {
            EntityId::new(value)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("player_action_id"))?;
        }
        if metadata.expected_version != 0
            || metadata.requesting_actor_id != request.submitted_by
            || metadata.authority_mode != "human_kp"
            || request.submitted_at_unix_ms == 0
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "player_action_submission",
            ));
        }
        self.ensure_campaign_member(&request.campaign_id, &request.submitted_by)
            .await?;
        let authorized: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.characters AS character
                  JOIN public.scenes AS scene
                    ON scene.scene_id = $1
                   AND scene.campaign_id = character.campaign_id
                 WHERE character.character_id = $2
                   AND character.campaign_id = $3
                   AND character.owner_user_id = $4
                   AND character.state = 'APPROVED'
                   AND scene.state = 'ACTIVE'
            )
            "#,
        )
        .bind(&request.scene_id)
        .bind(&request.character_id)
        .bind(&request.campaign_id)
        .bind(&request.submitted_by)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("authorize_player_action_subjects"))?;
        if !authorized {
            return Err(CoreDomainRepositoryError::Forbidden);
        }

        let intent = serde_json::to_value(&request.intent)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let projection = serde_json::json!({
            "kind": "SUBMIT",
            "action_id": request.action_id,
            "campaign_id": request.campaign_id,
            "character_id": request.character_id,
            "scene_id": request.scene_id,
            "submitted_by": request.submitted_by,
            "action_kind": request.intent.kind_name(),
            "intent": intent,
            "submitted_at_unix_ms": request.submitted_at_unix_ms,
            "visibility_label": metadata.visibility_label,
            "visibility_subject": metadata.visibility_subject,
            "provenance_kind": metadata.provenance_kind,
            "provenance_reference": metadata.provenance_reference,
            "provenance_recorded_by": metadata.provenance_recorded_by,
        });
        let projection_id = self.player_action_projection_id(&projection).await?;
        let event = player_action_event(
            "PlayerActionSubmitted",
            serde_json::json!({
                "schema_version": 1,
                "action_id": request.action_id,
                "campaign_id": request.campaign_id,
                "character_id": request.character_id,
                "scene_id": request.scene_id,
                "submitted_by": request.submitted_by,
                "intent": request.intent,
                "state": "AWAITING_HUMAN_CONFIRMATION",
            }),
            vec![
                projection_target("public.player_actions", &request.action_id),
                projection_target("core_domain.player_action_projection", &projection_id),
            ],
        )?;
        let draft = metadata.to_player_action_draft(
            &request.campaign_id,
            &request.action_id,
            vec![event],
        )?;
        let persisted = self
            .canonical
            .commit_player_action_projection(&draft, &projection)
            .await?;
        self.verify_player_action_commit(
            metadata,
            &request.action_id,
            "AWAITING_HUMAN_CONFIRMATION",
            &persisted,
        )
        .await?;
        Ok(persisted)
    }

    pub async fn load_pending_player_action(
        &self,
        campaign_id: &str,
        action_id: &str,
    ) -> Result<PendingPlayerActionRecord, CoreDomainRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT action.action_id, action.campaign_id, action.character_id,
                   action.scene_id, action.submitted_by, action.intent_json,
                   sheet.sheet_json, character.current_sheet_version
              FROM public.player_actions AS action
              JOIN public.characters AS character
                ON character.character_id = action.character_id
               AND character.campaign_id = action.campaign_id
              JOIN public.character_sheet_versions AS sheet
                ON sheet.character_id = character.character_id
               AND sheet.version = character.current_sheet_version
             WHERE action.campaign_id = $1
               AND action.action_id = $2
               AND action.state = 'AWAITING_HUMAN_CONFIRMATION'
             FOR SHARE OF action, character, sheet
            "#,
        )
        .bind(campaign_id)
        .bind(action_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_pending_player_action"))?
        .ok_or(CoreDomainRepositoryError::NotFound("pending_player_action"))?;
        let intent: sqlx::types::Json<PlayerActionIntentRecord> = row.get("intent_json");
        let sheet: sqlx::types::Json<serde_json::Value> = row.get("sheet_json");
        Ok(PendingPlayerActionRecord {
            action_id: row.get("action_id"),
            campaign_id: row.get("campaign_id"),
            character_id: row.get("character_id"),
            scene_id: row.get("scene_id"),
            submitted_by: row.get("submitted_by"),
            intent: intent.0,
            character_sheet_json: serde_json::to_string(&sheet.0)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?,
            character_sheet_version: row.get("current_sheet_version"),
        })
    }

    pub async fn load_player_action_header(
        &self,
        campaign_id: &str,
        action_id: &str,
    ) -> Result<PlayerActionHeader, CoreDomainRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT action_kind, submitted_by, state
              FROM public.player_actions
             WHERE campaign_id = $1 AND action_id = $2
            "#,
        )
        .bind(campaign_id)
        .bind(action_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_player_action_header"))?
        .ok_or(CoreDomainRepositoryError::NotFound("player_action"))?;
        Ok(PlayerActionHeader {
            action_kind: row.get("action_kind"),
            submitted_by: row.get("submitted_by"),
            state: row.get("state"),
        })
    }

    pub async fn load_resolved_player_action_receipt(
        &self,
        metadata: &CoreCommandMetadata,
        campaign_id: &str,
        action_id: &str,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT formal.commit_id, formal.first_event_sequence,
                   formal.last_event_sequence, formal.first_stream_version,
                   formal.last_stream_version, formal.audit_sequence,
                   formal.witness_prepare_sequence,
                   formal.witness_prepare_hash
              FROM public.formal_commits AS formal
              JOIN public.player_actions AS action
                ON action.campaign_id = formal.campaign_id
               AND action.action_id = formal.stream_id
             WHERE formal.commit_id = $1
               AND formal.idempotency_key = $2
               AND formal.campaign_id = $3
               AND formal.stream_id = $4
               AND formal.expected_version = 1
               AND formal.status = 'committed'
               AND action.state = 'RESOLVED'
               AND action.last_event_sequence BETWEEN
                   formal.first_event_sequence AND formal.last_event_sequence
               AND EXISTS (
                   SELECT 1
                     FROM public.event_store AS event
                    WHERE event.sequence BETWEEN
                          formal.first_event_sequence AND formal.last_event_sequence
                      AND event.command_id = $5
                      AND event.event_type = 'DecisionCommitted'
                      AND event.integrity_status = 'verified_hmac'
               )
            "#,
        )
        .bind(&metadata.commit_id)
        .bind(&metadata.idempotency_key)
        .bind(campaign_id)
        .bind(action_id)
        .bind(&metadata.command_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_resolved_player_action_receipt"))?
        .ok_or(CoreDomainRepositoryError::NotFound(
            "resolved_player_action_receipt",
        ))?;
        Ok(PersistedCommit {
            commit_id: row.get("commit_id"),
            first_event_sequence: row.get("first_event_sequence"),
            last_event_sequence: row.get("last_event_sequence"),
            first_stream_version: row.get("first_stream_version"),
            last_stream_version: row.get("last_stream_version"),
            audit_sequence: row.get("audit_sequence"),
            witness_prepare_sequence: row.get("witness_prepare_sequence"),
            witness_prepare_hash: row.get("witness_prepare_hash"),
        })
    }

    pub async fn commit_investigation_execution(
        &self,
        metadata: &CoreCommandMetadata,
        execution: &InvestigationExecutionRecord,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let pending = match self
            .load_pending_player_action(&execution.campaign_id, &execution.action_id)
            .await
        {
            Ok(pending) => pending,
            Err(CoreDomainRepositoryError::NotFound(_)) => {
                return self
                    .load_resolved_player_action_receipt(
                        metadata,
                        &execution.campaign_id,
                        &execution.action_id,
                    )
                    .await;
            }
            Err(error) => return Err(error),
        };
        let PlayerActionIntentRecord::Investigation {
            skill_name,
            clue_id,
            clue_importance,
            adjustment,
        } = &pending.intent
        else {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "investigation_action_kind",
            ));
        };
        let sheet: serde_json::Value = serde_json::from_str(&pending.character_sheet_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_sheet_json"))?;
        let authoritative_target = sheet
            .get("skills")
            .and_then(|skills| skills.get(skill_name))
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .ok_or(CoreDomainRepositoryError::Integrity(
                "character_skill_missing",
            ))?;
        validate_server_dice_record(&execution.dice, adjustment)?;
        let succeeded = matches!(
            execution.dice.success_level.as_str(),
            "CRITICAL" | "EXTREME" | "HARD" | "REGULAR"
        );
        let expected_clue_outcome = if succeeded {
            "REVEALED"
        } else if clue_importance == "CORE" {
            "REVEALED_WITH_COST"
        } else {
            "NOT_FOUND"
        };
        let expected_cost =
            (expected_clue_outcome == "REVEALED_WITH_COST").then_some("time_or_complication");
        if metadata.expected_version != 1
            || metadata.requesting_actor_id != execution.confirmed_by
            || metadata.requesting_actor_id != metadata.authority_owner
            || metadata.requesting_actor_role != "human_keeper"
            || metadata.provenance_kind != "human_keeper_statement"
            || execution.character_id != pending.character_id
            || execution.skill_name != *skill_name
            || execution.dice.target_value != authoritative_target
            || execution.clue_id != *clue_id
            || execution.clue_importance != *clue_importance
            || execution.clue_outcome != expected_clue_outcome
            || execution.clue_cost.as_deref() != expected_cost
            || execution.revealed_to_party != (expected_clue_outcome != "NOT_FOUND")
            || execution.resolved_at_unix_ms == 0
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "investigation_execution",
            ));
        }
        self.ensure_campaign_admin(&execution.campaign_id, &execution.confirmed_by)
            .await?;
        for value in [
            execution.decision_id.as_str(),
            execution.tool_execution_id.as_str(),
            execution.clue_record_id.as_str(),
        ] {
            EntityId::new(value)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("execution_id"))?;
        }

        let outcome = serde_json::json!({
            "kind": "INVESTIGATION",
            "skill_name": skill_name,
            "success_level": execution.dice.success_level,
            "clue_outcome": execution.clue_outcome,
            "clue_cost": execution.clue_cost,
        });
        let projection = serde_json::json!({
            "kind": "CONFIRM_INVESTIGATION",
            "action_id": execution.action_id,
            "campaign_id": execution.campaign_id,
            "character_id": execution.character_id,
            "decision_id": execution.decision_id,
            "tool_execution_id": execution.tool_execution_id,
            "confirmed_by": execution.confirmed_by,
            "resolved_at_unix_ms": execution.resolved_at_unix_ms,
            "outcome": outcome,
            "roll_id": execution.dice.roll_id,
            "target_value": execution.dice.target_value,
            "rolled_value": execution.dice.rolled_value,
            "success_level": execution.dice.success_level,
            "selected_tens_digit": execution.dice.selected_tens_digit,
            "ones_digit": execution.dice.ones_digit,
            "adjustment": execution.dice.adjustment,
            "clue_record_id": execution.clue_record_id,
            "clue_id": execution.clue_id,
            "clue_importance": execution.clue_importance,
            "clue_outcome": execution.clue_outcome,
            "clue_cost": execution.clue_cost,
            "revealed_to_party": execution.revealed_to_party,
            "visibility_label": metadata.visibility_label,
            "visibility_subject": metadata.visibility_subject,
            "provenance_kind": metadata.provenance_kind,
            "provenance_reference": metadata.provenance_reference,
            "provenance_recorded_by": metadata.provenance_recorded_by,
        });
        let projection_id = self.player_action_projection_id(&projection).await?;
        let synthetic =
            || projection_target("core_domain.player_action_projection", &projection_id);
        let events = vec![
            player_action_event(
                "DiceRolled",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "roll_id": execution.dice.roll_id,
                    "target": execution.dice.target_value,
                    "roll": execution.dice.rolled_value,
                    "success_level": execution.dice.success_level,
                    "adjustment": execution.dice.adjustment,
                    "random_source": "SERVER_OS_CSPRNG",
                }),
                vec![
                    projection_target("public.dice_rolls", &execution.dice.roll_id),
                    synthetic(),
                ],
            )?,
            player_action_event(
                "SkillCheckResolved",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "skill_name": execution.skill_name,
                    "success_level": execution.dice.success_level,
                }),
                vec![synthetic()],
            )?,
            player_action_event(
                "ClueRevealed",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "clue_id": execution.clue_id,
                    "importance": execution.clue_importance,
                    "outcome": execution.clue_outcome,
                    "cost": execution.clue_cost,
                }),
                vec![
                    projection_target("public.clues", &execution.clue_record_id),
                    synthetic(),
                ],
            )?,
            player_action_event(
                "DecisionCommitted",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "tool_execution_id": execution.tool_execution_id,
                    "confirmed_by": execution.confirmed_by,
                    "outcome": outcome,
                }),
                vec![
                    projection_target("public.decision_records", &execution.decision_id),
                    projection_target("public.player_actions", &execution.action_id),
                    synthetic(),
                ],
            )?,
        ];
        let draft = metadata.to_player_action_draft(
            &execution.campaign_id,
            &execution.action_id,
            events,
        )?;
        let persisted = self
            .canonical
            .commit_player_action_projection(&draft, &projection)
            .await?;
        self.verify_player_action_commit(metadata, &execution.action_id, "RESOLVED", &persisted)
            .await?;
        Ok(persisted)
    }

    pub async fn commit_sanity_execution(
        &self,
        metadata: &CoreCommandMetadata,
        execution: &SanityExecutionRecord,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let pending = match self
            .load_pending_player_action(&execution.campaign_id, &execution.action_id)
            .await
        {
            Ok(pending) => pending,
            Err(CoreDomainRepositoryError::NotFound(_)) => {
                return self
                    .load_resolved_player_action_receipt(
                        metadata,
                        &execution.campaign_id,
                        &execution.action_id,
                    )
                    .await;
            }
            Err(error) => return Err(error),
        };
        let PlayerActionIntentRecord::SanityCheck {
            success_loss,
            failure_loss,
            day_key,
        } = &pending.intent
        else {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "sanity_action_kind",
            ));
        };
        let mut sheet: serde_json::Value = serde_json::from_str(&pending.character_sheet_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_sheet_json"))?;
        let power = sheet
            .pointer("/characteristics/power")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .ok_or(CoreDomainRepositoryError::Integrity(
                "character_power_missing",
            ))?;
        let existing_state = sheet.get("sanity_state");
        let same_day = existing_state
            .and_then(|state| state.get("day_key"))
            .and_then(serde_json::Value::as_str)
            == Some(day_key);
        let current_sanity = existing_state
            .and_then(|state| state.get("current_sanity"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(power);
        let day_start_sanity = if same_day {
            existing_state
                .and_then(|state| state.get("day_start_sanity"))
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "sanity_day_start_missing",
                ))?
        } else {
            current_sanity
        };
        let prior_day_loss = if same_day {
            existing_state
                .and_then(|state| state.get("day_loss"))
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "sanity_day_loss_missing",
                ))?
        } else {
            0
        };
        validate_server_dice_record(&execution.dice, "NONE")?;
        let succeeded = matches!(
            execution.dice.success_level.as_str(),
            "CRITICAL" | "EXTREME" | "HARD" | "REGULAR"
        );
        let expected_loss = if succeeded {
            *success_loss
        } else {
            *failure_loss
        };
        let expected_day_loss = prior_day_loss.saturating_add(expected_loss);
        let expected_threshold = (day_start_sanity / 5).max(1);
        let expected_after = current_sanity.saturating_sub(expected_loss);
        let expected_state = if expected_day_loss >= expected_threshold {
            "INDEFINITE_INSANITY"
        } else if expected_loss >= 5 {
            "TEMPORARY_INSANITY"
        } else {
            "STABLE"
        };
        if metadata.expected_version != 1
            || metadata.requesting_actor_id != execution.confirmed_by
            || metadata.requesting_actor_id != metadata.authority_owner
            || metadata.requesting_actor_role != "human_keeper"
            || metadata.provenance_kind != "human_keeper_statement"
            || execution.character_id != pending.character_id
            || execution.day_key != *day_key
            || execution.dice.target_value != current_sanity
            || execution.day_start_sanity != day_start_sanity
            || execution.sanity_before != current_sanity
            || execution.sanity_after != expected_after
            || execution.sanity_loss != expected_loss
            || execution.day_loss != expected_day_loss
            || execution.indefinite_threshold != expected_threshold
            || execution.madness_state != expected_state
            || execution.resolved_at_unix_ms == 0
        {
            return Err(CoreDomainRepositoryError::InvalidInput("sanity_execution"));
        }
        self.ensure_campaign_admin(&execution.campaign_id, &execution.confirmed_by)
            .await?;
        let sheet_version = pending.character_sheet_version.checked_add(1).ok_or(
            CoreDomainRepositoryError::Integrity("character_sheet_version_overflow"),
        )?;
        sheet["sanity_state"] = serde_json::json!({
            "day_key": day_key,
            "day_start_sanity": day_start_sanity,
            "current_sanity": expected_after,
            "day_loss": expected_day_loss,
            "madness_state": expected_state,
        });
        let outcome = serde_json::json!({
            "kind": "SANITY_CHECK",
            "success_level": execution.dice.success_level,
            "sanity_loss": expected_loss,
            "sanity_after": expected_after,
            "madness_state": expected_state,
        });
        let projection = serde_json::json!({
            "kind": "CONFIRM_SANITY",
            "action_id": execution.action_id,
            "campaign_id": execution.campaign_id,
            "character_id": execution.character_id,
            "decision_id": execution.decision_id,
            "tool_execution_id": execution.tool_execution_id,
            "confirmed_by": execution.confirmed_by,
            "resolved_at_unix_ms": execution.resolved_at_unix_ms,
            "outcome": outcome,
            "roll_id": execution.dice.roll_id,
            "target_value": execution.dice.target_value,
            "rolled_value": execution.dice.rolled_value,
            "success_level": execution.dice.success_level,
            "selected_tens_digit": execution.dice.selected_tens_digit,
            "ones_digit": execution.dice.ones_digit,
            "adjustment": execution.dice.adjustment,
            "sanity_event_id": execution.sanity_event_id,
            "sheet_version_id": execution.sheet_version_id,
            "sheet_version": sheet_version,
            "sheet_json": sheet,
            "day_key": execution.day_key,
            "day_start_sanity": execution.day_start_sanity,
            "sanity_before": execution.sanity_before,
            "sanity_after": execution.sanity_after,
            "sanity_loss": execution.sanity_loss,
            "day_loss": execution.day_loss,
            "indefinite_threshold": execution.indefinite_threshold,
            "madness_state": execution.madness_state,
            "visibility_label": metadata.visibility_label,
            "visibility_subject": metadata.visibility_subject,
            "provenance_kind": metadata.provenance_kind,
            "provenance_reference": metadata.provenance_reference,
            "provenance_recorded_by": metadata.provenance_recorded_by,
        });
        let projection_id = self.player_action_projection_id(&projection).await?;
        let synthetic =
            || projection_target("core_domain.player_action_projection", &projection_id);
        let events = vec![
            player_action_event(
                "DiceRolled",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "roll_id": execution.dice.roll_id,
                    "target": execution.dice.target_value,
                    "roll": execution.dice.rolled_value,
                    "success_level": execution.dice.success_level,
                    "random_source": "SERVER_OS_CSPRNG",
                }),
                vec![
                    projection_target("public.dice_rolls", &execution.dice.roll_id),
                    synthetic(),
                ],
            )?,
            player_action_event(
                "SanityLossApplied",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "sanity_event_id": execution.sanity_event_id,
                    "day_key": execution.day_key,
                    "day_start_sanity": execution.day_start_sanity,
                    "sanity_before": execution.sanity_before,
                    "sanity_after": execution.sanity_after,
                    "loss": execution.sanity_loss,
                    "day_loss": execution.day_loss,
                    "indefinite_threshold": execution.indefinite_threshold,
                    "madness_state": execution.madness_state,
                }),
                vec![
                    projection_target("public.sanity_events", &execution.sanity_event_id),
                    projection_target(
                        "public.character_sheet_versions",
                        &execution.sheet_version_id,
                    ),
                    projection_target("public.characters", &execution.character_id),
                    synthetic(),
                ],
            )?,
            player_action_event(
                "DecisionCommitted",
                serde_json::json!({
                    "schema_version": 1,
                    "action_id": execution.action_id,
                    "decision_id": execution.decision_id,
                    "tool_execution_id": execution.tool_execution_id,
                    "confirmed_by": execution.confirmed_by,
                    "outcome": outcome,
                }),
                vec![
                    projection_target("public.decision_records", &execution.decision_id),
                    projection_target("public.player_actions", &execution.action_id),
                    synthetic(),
                ],
            )?,
        ];
        let draft = metadata.to_player_action_draft(
            &execution.campaign_id,
            &execution.action_id,
            events,
        )?;
        let persisted = self
            .canonical
            .commit_player_action_projection(&draft, &projection)
            .await?;
        self.verify_player_action_commit(metadata, &execution.action_id, "RESOLVED", &persisted)
            .await?;
        Ok(persisted)
    }
}

fn projection_target(relation: &str, row_id: &str) -> CanonicalProjectionTarget {
    CanonicalProjectionTarget {
        relation: relation.to_owned(),
        row_id: row_id.to_owned(),
    }
}

fn parse_session_state(value: &str) -> Result<SessionState, CoreDomainRepositoryError> {
    match value {
        "SCHEDULED" => Ok(SessionState::Scheduled),
        "ACTIVE" => Ok(SessionState::Active),
        "PAUSED" => Ok(SessionState::Paused),
        "ENDED" => Ok(SessionState::Ended),
        _ => Err(CoreDomainRepositoryError::Integrity(
            "unknown_session_state",
        )),
    }
}

fn session_state_action(state: SessionState) -> &'static str {
    match state {
        SessionState::Scheduled => "session.schedule",
        SessionState::Active => "session.resume",
        SessionState::Paused => "session.pause",
        SessionState::Ended => "session.end",
    }
}

async fn apply_session_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
        .map_err(|_| CoreDomainRepositoryError::Integrity("session_replay_payload"))?;
    event.validate_schema_version()?;
    match event {
        CoreDomainEvent::SessionStarted {
            session_id,
            campaign_id,
            room_id,
            scenario_id,
            scene_id,
            scene_key,
            scene_name,
            started_at_unix_ms,
            ..
        } => {
            if campaign_id != replay.campaign_id {
                return Err(CoreDomainRepositoryError::Integrity(
                    "session_replay_campaign_mismatch",
                ));
            }
            let started_at = timestamp_from_unix_ms(started_at_unix_ms, "session.started_at")?;
            sqlx::query(
                r#"
                INSERT INTO core_domain.sessions (
                    session_id, campaign_id, room_id, scenario_id, state,
                    active_scene_id, started_at, ended_at, version,
                    visibility_label, visibility_subject,
                    provenance_kind, provenance_reference, provenance_recorded_by,
                    last_event_sequence
                ) VALUES (
                    $1, $2, $3, $4, 'ACTIVE', $5, $6, NULL, 1,
                    $7, $8, $9, $10, $11, $12
                )
                ON CONFLICT (session_id) DO NOTHING
                "#,
            )
            .bind(&session_id)
            .bind(&campaign_id)
            .bind(&room_id)
            .bind(&scenario_id)
            .bind(&scene_id)
            .bind(started_at)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_session_start"))?;
            let persisted_session = sqlx::query(
                r#"
                SELECT campaign_id, room_id, scenario_id, started_at
                  FROM core_domain.sessions
                 WHERE session_id = $1
                "#,
            )
            .bind(&session_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_session_start"))?;
            if persisted_session.get::<String, _>("campaign_id") != campaign_id
                || persisted_session.get::<String, _>("room_id") != room_id
                || persisted_session.get::<String, _>("scenario_id") != scenario_id
                || persisted_session.get::<DateTime<Utc>, _>("started_at") != started_at
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "session_replay_identity_conflict",
                ));
            }
            sqlx::query(
                r#"
                INSERT INTO public.scenes (
                    scene_id, campaign_id, session_id, scenario_id, room_id,
                    scene_key, name, state, version,
                    visibility_label, visibility_subject,
                    provenance_kind, provenance_reference, provenance_recorded_by,
                    last_event_sequence
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, 'ACTIVE', 1,
                    $8, $9, $10, $11, $12, $13
                )
                ON CONFLICT (scene_id) DO NOTHING
                "#,
            )
            .bind(&scene_id)
            .bind(&campaign_id)
            .bind(&session_id)
            .bind(&scenario_id)
            .bind(&room_id)
            .bind(&scene_key)
            .bind(&scene_name)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_opening_scene"))?;
            let persisted_scene = sqlx::query(
                r#"
                SELECT campaign_id, session_id, scenario_id, room_id,
                       scene_key, name
                  FROM public.scenes
                 WHERE scene_id = $1
                "#,
            )
            .bind(&scene_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_opening_scene"))?;
            if persisted_scene.get::<String, _>("campaign_id") != campaign_id
                || persisted_scene.get::<String, _>("session_id") != session_id
                || persisted_scene.get::<String, _>("scenario_id") != scenario_id
                || persisted_scene.get::<String, _>("room_id") != room_id
                || persisted_scene.get::<String, _>("scene_key") != scene_key
                || persisted_scene.get::<String, _>("name") != scene_name
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "scene_replay_identity_conflict",
                ));
            }
        }
        CoreDomainEvent::SessionStateChanged {
            session_id,
            from,
            to,
            changed_at_unix_ms,
            ..
        } => {
            let changed_at = timestamp_from_unix_ms(changed_at_unix_ms, "session.changed_at")?;
            if to == SessionState::Ended {
                sqlx::query(
                    r#"
                    UPDATE public.scenes
                       SET state = 'CLOSED',
                           version = version + 1,
                           visibility_label = $1,
                           visibility_subject = $2,
                           provenance_kind = $3,
                           provenance_reference = $4,
                           provenance_recorded_by = $5,
                           last_event_sequence = $6
                     WHERE scene_id = (
                         SELECT active_scene_id
                           FROM core_domain.sessions
                          WHERE session_id = $7
                     )
                       AND state = 'ACTIVE'
                       AND last_event_sequence < $6
                    "#,
                )
                .bind(&replay.visibility_label)
                .bind(&replay.visibility_subject)
                .bind(&replay.provenance_kind)
                .bind(&replay.provenance_reference)
                .bind(&replay.provenance_recorded_by)
                .bind(replay.sequence)
                .bind(&session_id)
                .execute(&mut **transaction)
                .await
                .map_err(database_error("replay_close_ending_scene"))?;
            }
            let result = sqlx::query(
                r#"
                UPDATE core_domain.sessions
                   SET state = $1,
                       ended_at = CASE WHEN $1 = 'ENDED' THEN $2 ELSE NULL END,
                       version = version + 1,
                       visibility_label = $3,
                       visibility_subject = $4,
                       provenance_kind = $5,
                       provenance_reference = $6,
                       provenance_recorded_by = $7,
                       last_event_sequence = $8
                 WHERE session_id = $9
                   AND state = $10
                   AND last_event_sequence < $8
                "#,
            )
            .bind(to.as_str())
            .bind(changed_at)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .bind(&session_id)
            .bind(from.as_str())
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_session_transition"))?;
            if result.rows_affected() == 0 {
                let existing = sqlx::query(
                    "SELECT state, last_event_sequence \
                     FROM core_domain.sessions WHERE session_id = $1",
                )
                .bind(&session_id)
                .fetch_one(&mut **transaction)
                .await
                .map_err(database_error("verify_replayed_session_transition"))?;
                if existing.get::<i64, _>("last_event_sequence") < replay.sequence {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "session_replay_transition_conflict",
                    ));
                }
            }
        }
        CoreDomainEvent::SceneSwitched {
            session_id,
            previous_scene_id,
            next_scene_id,
            next_scene_key,
            next_scene_name,
            ..
        } => {
            let session = sqlx::query(
                r#"
                SELECT campaign_id, room_id, scenario_id, active_scene_id,
                       last_event_sequence
                  FROM core_domain.sessions
                 WHERE session_id = $1
                "#,
            )
            .bind(&session_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(database_error("replay_load_scene_session"))?
            .ok_or(CoreDomainRepositoryError::Integrity(
                "session_replay_missing_session",
            ))?;
            if session.get::<String, _>("campaign_id") != replay.campaign_id
                || (session.get::<i64, _>("last_event_sequence") < replay.sequence
                    && session
                        .get::<Option<String>, _>("active_scene_id")
                        .as_deref()
                        != Some(previous_scene_id.as_str()))
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "scene_replay_predecessor_mismatch",
                ));
            }
            let closed = sqlx::query(
                r#"
                UPDATE public.scenes
                   SET state = 'CLOSED',
                       version = version + 1,
                       visibility_label = $1,
                       visibility_subject = $2,
                       provenance_kind = $3,
                       provenance_reference = $4,
                       provenance_recorded_by = $5,
                       last_event_sequence = $6
                 WHERE scene_id = $7
                   AND state = 'ACTIVE'
                   AND last_event_sequence < $6
                "#,
            )
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .bind(&previous_scene_id)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_close_previous_scene"))?;
            if closed.rows_affected() == 0 {
                let existing_sequence: Option<i64> = sqlx::query_scalar(
                    "SELECT last_event_sequence FROM public.scenes WHERE scene_id = $1",
                )
                .bind(&previous_scene_id)
                .fetch_optional(&mut **transaction)
                .await
                .map_err(database_error("verify_replayed_previous_scene"))?;
                if existing_sequence.is_none_or(|sequence| sequence < replay.sequence) {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "scene_replay_previous_not_active",
                    ));
                }
            }
            sqlx::query(
                r#"
                INSERT INTO public.scenes (
                    scene_id, campaign_id, session_id, scenario_id, room_id,
                    scene_key, name, state, version,
                    visibility_label, visibility_subject,
                    provenance_kind, provenance_reference, provenance_recorded_by,
                    last_event_sequence
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, 'ACTIVE', 1,
                    $8, $9, $10, $11, $12, $13
                )
                ON CONFLICT (scene_id) DO NOTHING
                "#,
            )
            .bind(&next_scene_id)
            .bind(&replay.campaign_id)
            .bind(&session_id)
            .bind(session.get::<String, _>("scenario_id"))
            .bind(session.get::<String, _>("room_id"))
            .bind(&next_scene_key)
            .bind(&next_scene_name)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_next_scene"))?;
            let persisted_next_scene = sqlx::query(
                r#"
                SELECT campaign_id, session_id, scenario_id, room_id,
                       scene_key, name
                  FROM public.scenes
                 WHERE scene_id = $1
                "#,
            )
            .bind(&next_scene_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_next_scene"))?;
            if persisted_next_scene.get::<String, _>("campaign_id") != replay.campaign_id
                || persisted_next_scene.get::<String, _>("session_id") != session_id
                || persisted_next_scene.get::<String, _>("scenario_id")
                    != session.get::<String, _>("scenario_id")
                || persisted_next_scene.get::<String, _>("room_id")
                    != session.get::<String, _>("room_id")
                || persisted_next_scene.get::<String, _>("scene_key") != next_scene_key
                || persisted_next_scene.get::<String, _>("name") != next_scene_name
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "scene_replay_identity_conflict",
                ));
            }
            let advanced = sqlx::query(
                r#"
                UPDATE core_domain.sessions
                   SET active_scene_id = $1,
                       version = version + 1,
                       visibility_label = $2,
                       visibility_subject = $3,
                       provenance_kind = $4,
                       provenance_reference = $5,
                       provenance_recorded_by = $6,
                       last_event_sequence = $7
                 WHERE session_id = $8
                   AND active_scene_id = $9
                   AND last_event_sequence < $7
                "#,
            )
            .bind(&next_scene_id)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .bind(&session_id)
            .bind(&previous_scene_id)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_active_scene"))?;
            if advanced.rows_affected() == 0 {
                let existing = sqlx::query(
                    "SELECT active_scene_id, last_event_sequence \
                     FROM core_domain.sessions WHERE session_id = $1",
                )
                .bind(&session_id)
                .fetch_one(&mut **transaction)
                .await
                .map_err(database_error("verify_replayed_active_scene"))?;
                if existing.get::<i64, _>("last_event_sequence") < replay.sequence {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "scene_replay_session_conflict",
                    ));
                }
            }
        }
        _ => {
            return Err(CoreDomainRepositoryError::Integrity(
                "non_session_event_in_session_replay",
            ));
        }
    }
    Ok(())
}

fn timestamp_from_unix_ms(
    value: u64,
    field: &'static str,
) -> Result<DateTime<Utc>, CoreDomainRepositoryError> {
    let value = i64::try_from(value).map_err(|_| CoreDomainRepositoryError::InvalidInput(field))?;
    Utc.timestamp_millis_opt(value)
        .single()
        .ok_or(CoreDomainRepositoryError::InvalidInput(field))
}

fn validated_object(source: &str, field: &'static str) -> Result<Value, CoreDomainRepositoryError> {
    let value: Value =
        serde_json::from_str(source).map_err(|_| CoreDomainRepositoryError::InvalidInput(field))?;
    if !value.is_object() {
        return Err(CoreDomainRepositoryError::InvalidInput(field));
    }
    Ok(value)
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn token_digest_matches(supplied: &str, expected: &str) -> bool {
    type HmacSha256 = Hmac<Sha256>;
    const COMPARISON_KEY: &[u8] = b"p06-invite-token-digest-comparison-v1";
    let mut expected_mac =
        HmacSha256::new_from_slice(COMPARISON_KEY).expect("fixed HMAC key is valid");
    expected_mac.update(expected.as_bytes());
    let expected_tag = expected_mac.finalize().into_bytes();
    let mut supplied_mac =
        HmacSha256::new_from_slice(COMPARISON_KEY).expect("fixed HMAC key is valid");
    supplied_mac.update(supplied.as_bytes());
    supplied_mac.verify_slice(expected_tag.as_slice()).is_ok()
}

fn canonical_event_idempotency_matches(stored: &str, command_key: &str) -> bool {
    stored == format!("{command_key}:0000")
}

fn database_error(
    operation: &'static str,
) -> impl FnOnce(sqlx::Error) -> CoreDomainRepositoryError {
    move |_| CoreDomainRepositoryError::Database(operation)
}

impl CoreDomainRepository {
    async fn ensure_user_exists(&self, user_id: &str) -> Result<(), CoreDomainRepositoryError> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.users WHERE user_id = $1)")
                .bind(user_id)
                .fetch_one(&self.primary)
                .await
                .map_err(database_error("load_user"))?;
        if exists {
            Ok(())
        } else {
            Err(CoreDomainRepositoryError::NotFound("user"))
        }
    }

    async fn ensure_campaign_member(
        &self,
        campaign_id: &str,
        user_id: &str,
    ) -> Result<(), CoreDomainRepositoryError> {
        let permitted: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.campaigns
                 WHERE campaign_id = $1
                   AND owner_user_id = $2
                UNION ALL
                SELECT 1
                  FROM public.campaign_memberships
                 WHERE campaign_id = $1
                   AND user_id = $2
                   AND revoked_at IS NULL
            )
            "#,
        )
        .bind(campaign_id)
        .bind(user_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("authorize_campaign_member"))?;
        if permitted {
            Ok(())
        } else {
            Err(CoreDomainRepositoryError::Forbidden)
        }
    }

    async fn ensure_campaign_admin(
        &self,
        campaign_id: &str,
        user_id: &str,
    ) -> Result<(), CoreDomainRepositoryError> {
        let permitted: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.campaigns
                 WHERE campaign_id = $1
                   AND owner_user_id = $2
                UNION ALL
                SELECT 1
                  FROM public.campaign_memberships
                 WHERE campaign_id = $1
                   AND user_id = $2
                   AND role IN ('CAMPAIGN_OWNER', 'HUMAN_KEEPER')
                   AND revoked_at IS NULL
            )
            "#,
        )
        .bind(campaign_id)
        .bind(user_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("authorize_campaign_admin"))?;
        if permitted {
            Ok(())
        } else {
            Err(CoreDomainRepositoryError::Forbidden)
        }
    }

    async fn projection_matches_command(
        &self,
        event_sequence: i64,
        metadata: &CoreCommandMetadata,
    ) -> Result<bool, CoreDomainRepositoryError> {
        sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.event_store AS event
                  JOIN public.formal_commits AS formal
                    ON event.sequence BETWEEN
                       formal.first_event_sequence AND formal.last_event_sequence
                 WHERE event.sequence = $1
                   AND event.command_id = $2
                   AND formal.idempotency_key = $3
                   AND formal.commit_id = $4
                   AND formal.campaign_id = event.campaign_id
                   AND formal.stream_id = event.stream_id
                   AND formal.status = 'committed'
                   AND event.integrity_status = 'verified_hmac'
            )
            "#,
        )
        .bind(event_sequence)
        .bind(&metadata.command_id)
        .bind(&metadata.idempotency_key)
        .bind(&metadata.commit_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("verify_projection_command"))
    }

    pub async fn create_campaign(
        &self,
        metadata: &CoreCommandMetadata,
        request: &CreateCampaignRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0 || metadata.requesting_actor_id != request.owner_user_id {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "campaign_create_metadata",
            ));
        }
        let campaign = CampaignAggregate::new(
            &request.campaign_id,
            &request.owner_user_id,
            &request.authority.contract_id,
            &request.title,
            request.created_at_unix_ms,
        )?;
        let room = Room::new(&request.room_id, &request.campaign_id, &request.room_name)?;
        request.authority.validate(&request.campaign_id, metadata)?;
        self.ensure_user_exists(&request.owner_user_id).await?;
        let created_at = timestamp_from_unix_ms(request.created_at_unix_ms, "campaign.created_at")?;
        let event = CoreDomainEvent::CampaignCreated {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            campaign_id: campaign.campaign_id.to_string(),
            owner_user_id: campaign.owner_user_id.to_string(),
            authority_contract_id: request.authority.contract_id.clone(),
            authority_mode: request.authority.authority_mode.clone(),
            authority_owner: request.authority.authority_owner.clone(),
            title: campaign.title.clone(),
            room_id: room.room_id.to_string(),
            room_name: room.name.clone(),
            created_at_unix_ms: request.created_at_unix_ms,
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.campaign_id,
                ("campaign", "campaign.create"),
                &event,
                vec![
                    projection_target("public.campaigns", &request.campaign_id),
                    projection_target("public.rooms", &request.room_id),
                ],
            )
            .await?;

        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.campaigns WHERE campaign_id = $1",
        )
        .bind(&request.campaign_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_existing_campaign"))?
        {
            if existing_sequence == persisted.last_event_sequence
                && self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Ok(persisted);
            }
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_identity_conflict",
            ));
        }

        let membership_role = match request.authority.authority_mode.as_str() {
            "HUMAN_KP" => "HUMAN_KEEPER",
            "AI_KP" => "CAMPAIGN_OWNER",
            _ => return Err(CoreDomainRepositoryError::InvalidInput("authority_mode")),
        };
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_campaign_create")
            .await?;
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *transaction)
            .await
            .map_err(database_error("defer_campaign_constraints"))?;
        sqlx::query(
            r#"
            INSERT INTO public.campaigns (
                campaign_id, owner_user_id, authority_contract_id, title,
                state, version, created_at,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, 'DRAFT', 1, $5,
                $6, $7, $8, $9, $10, $11
            )
            "#,
        )
        .bind(&request.campaign_id)
        .bind(&request.owner_user_id)
        .bind(&request.authority.contract_id)
        .bind(&request.title)
        .bind(created_at)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign"))?;
        sqlx::query(
            r#"
            INSERT INTO public.campaign_memberships (
                campaign_id, user_id, role, granted_by, granted_at
            ) VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(&request.campaign_id)
        .bind(&request.owner_user_id)
        .bind(membership_role)
        .bind(&metadata.requesting_actor_id)
        .bind(created_at)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign_owner_membership"))?;
        sqlx::query(
            r#"
            INSERT INTO public.authority_contracts (
                contract_id, campaign_id, authority_mode, authority_owner,
                contract_version, ruleset_version, house_rules_version,
                scenario_version, prompt_version, agent_pack_version,
                tool_schema_version, safety_profile_version,
                ai_provider_snapshot, model_route_snapshot,
                character_sheet_template_version, created_at, locked, change_policy
            ) VALUES (
                $1, $2, $3, $4, 1, $5, $6, $7, $8, $9, $10, $11,
                $12, $13, $14, $15, TRUE, 'FORK_ONLY'
            )
            "#,
        )
        .bind(&request.authority.contract_id)
        .bind(&request.campaign_id)
        .bind(&request.authority.authority_mode)
        .bind(&request.authority.authority_owner)
        .bind(&request.authority.ruleset_version)
        .bind(&request.authority.house_rules_version)
        .bind(&request.authority.scenario_version)
        .bind(&request.authority.prompt_version)
        .bind(&request.authority.agent_pack_version)
        .bind(&request.authority.tool_schema_version)
        .bind(&request.authority.safety_profile_version)
        .bind(&request.authority.ai_provider_snapshot)
        .bind(&request.authority.model_route_snapshot)
        .bind(&request.authority.character_sheet_template_version)
        .bind(created_at)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_authority_contract"))?;
        sqlx::query(
            r#"
            INSERT INTO public.rooms (
                room_id, campaign_id, name, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&request.room_id)
        .bind(&request.campaign_id)
        .bind(&request.room_name)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign_room"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_campaign_create"))?;
        Ok(persisted)
    }

    pub async fn issue_invite(
        &self,
        metadata: &CoreCommandMetadata,
        request: &IssueInviteRequest,
    ) -> Result<IssuedCampaignInvite, CoreDomainRepositoryError> {
        if metadata.expected_version != 0 {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "invite_expected_version",
            ));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        self.ensure_user_exists(&request.invited_user_id).await?;

        let raw_token = self
            .canonical
            .derive_campaign_invite_token(
                &request.campaign_id,
                &request.invite_id,
                &request.invited_user_id,
                request.role.as_database_role(),
                request.expires_at_unix_ms,
                &metadata.idempotency_key,
            )
            .map_err(CoreDomainRepositoryError::Canonical)?;
        let token_digest = format!("sha256:{:x}", Sha256::digest(raw_token.as_bytes()));
        let invite = CampaignInvite::new(
            &request.invite_id,
            &request.campaign_id,
            &request.invited_user_id,
            &metadata.requesting_actor_id,
            request.role,
            &token_digest,
            request.expires_at_unix_ms,
            self.clock.now_unix_ms()?,
        )?;
        let event = CoreDomainEvent::CampaignInviteIssued {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            invite_id: invite.invite_id.to_string(),
            campaign_id: invite.campaign_id.to_string(),
            invited_user_id: invite.invited_user_id.to_string(),
            issued_by: invite.issued_by.to_string(),
            role: invite.role,
            token_digest,
            expires_at_unix_ms: invite.expires_at_unix_ms,
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.invite_id,
                ("campaign_invite", "campaign.invite.issue"),
                &event,
                Vec::new(),
            )
            .await?;
        Ok(IssuedCampaignInvite {
            invite_id: request.invite_id.clone(),
            raw_token,
            expires_at_unix_ms: request.expires_at_unix_ms,
            persisted,
        })
    }

    async fn load_campaign_events(
        &self,
        campaign_id: &str,
    ) -> Result<Vec<CanonicalReplayEvent>, CoreDomainRepositoryError> {
        let mut events = Vec::new();
        let mut after_sequence = 0_i64;
        loop {
            let page = self
                .canonical
                .load_replay_page(campaign_id, after_sequence, 500)
                .await?;
            let page_len = page.len();
            if let Some(last) = page.last() {
                after_sequence = last.sequence;
            }
            events.extend(page);
            if page_len < 500 {
                break;
            }
        }
        Ok(events)
    }

    async fn load_idempotent_core_event(
        &self,
        campaign_id: &str,
        stream_id: &str,
        metadata: &CoreCommandMetadata,
        expected_event_type: &str,
    ) -> Result<Option<CoreDomainEvent>, CoreDomainRepositoryError> {
        let mut matched = None;
        for replay in self.load_campaign_events(campaign_id).await? {
            if replay.stream_id != stream_id
                || replay.command_id != metadata.command_id
                || !canonical_event_idempotency_matches(
                    &replay.idempotency_key,
                    &metadata.idempotency_key,
                )
            {
                continue;
            }
            if replay.event_type != expected_event_type || matched.is_some() {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_event_binding_conflict",
                ));
            }
            matched =
                Some(serde_json::from_value(replay.payload).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("idempotent_event_payload")
                })?);
        }
        Ok(matched)
    }

    pub async fn accept_invite(
        &self,
        metadata: &CoreCommandMetadata,
        request: &AcceptInviteRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 1
            || metadata.requesting_actor_id != request.accepting_user_id
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "invite_accept_metadata",
            ));
        }
        let accepting_user = UserId::new(&request.accepting_user_id)?;
        let mut issued = None;
        let mut prior_acceptance = None;
        for replay in self.load_campaign_events(&request.campaign_id).await? {
            if replay.event_type == "CampaignInviteIssued" {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload)
                    .map_err(|_| CoreDomainRepositoryError::Integrity("invite_event_payload"))?;
                if let CoreDomainEvent::CampaignInviteIssued {
                    invite_id,
                    campaign_id,
                    invited_user_id,
                    issued_by,
                    role,
                    token_digest,
                    expires_at_unix_ms,
                    ..
                } = event
                {
                    if invite_id == request.invite_id {
                        issued = Some(CampaignInvite::new(
                            invite_id,
                            campaign_id,
                            invited_user_id,
                            issued_by,
                            role,
                            token_digest,
                            expires_at_unix_ms,
                            expires_at_unix_ms.saturating_sub(1),
                        )?);
                    }
                }
            } else if replay.event_type == "CampaignInviteAccepted" {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload)
                    .map_err(|_| CoreDomainRepositoryError::Integrity("invite_event_payload"))?;
                if let CoreDomainEvent::CampaignInviteAccepted {
                    invite_id,
                    campaign_id,
                    user_id,
                    role,
                    accepted_at_unix_ms,
                    ..
                } = event
                {
                    if invite_id == request.invite_id {
                        if user_id != request.accepting_user_id
                            || !canonical_event_idempotency_matches(
                                &replay.idempotency_key,
                                &metadata.idempotency_key,
                            )
                            || prior_acceptance.is_some()
                        {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "invite_already_consumed",
                            ));
                        }
                        prior_acceptance = Some((campaign_id, role, accepted_at_unix_ms));
                    }
                }
            }
        }
        let issued = issued.ok_or(CoreDomainRepositoryError::NotFound("campaign_invite"))?;
        let accepted_at_unix_ms = match prior_acceptance {
            Some((campaign_id, role, accepted_at_unix_ms))
                if campaign_id == request.campaign_id && role == issued.role =>
            {
                accepted_at_unix_ms
            }
            Some(_) => {
                return Err(CoreDomainRepositoryError::Integrity(
                    "invite_acceptance_binding_conflict",
                ));
            }
            None => self.clock.now_unix_ms()?,
        };
        issued.validate_acceptance(&accepting_user, accepted_at_unix_ms)?;
        let supplied_digest = format!("sha256:{:x}", Sha256::digest(request.raw_token.as_bytes()));
        if !token_digest_matches(&supplied_digest, &issued.token_digest) {
            return Err(CoreDomainRepositoryError::Forbidden);
        }

        let event = CoreDomainEvent::CampaignInviteAccepted {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            invite_id: request.invite_id.clone(),
            campaign_id: request.campaign_id.clone(),
            user_id: request.accepting_user_id.clone(),
            role: issued.role,
            accepted_at_unix_ms,
        };
        let accepted_at = timestamp_from_unix_ms(accepted_at_unix_ms, "invite.accepted_at")?;
        let projection = serde_json::json!({
            "kind": "ACCEPT",
            "invite_id": request.invite_id,
            "campaign_id": request.campaign_id,
            "user_id": request.accepting_user_id,
            "role": issued.role.as_database_role(),
            "granted_by": issued.issued_by.to_string(),
            "granted_at_unix_ms": accepted_at_unix_ms,
        });
        let projection_id = self
            .campaign_invite_acceptance_projection_id(&projection)
            .await?;
        let draft = metadata.to_draft(
            &request.campaign_id,
            &request.invite_id,
            "campaign_invite",
            "campaign.invite.accept",
            &event,
            vec![projection_target(
                "core_domain.campaign_invite_acceptance",
                &projection_id,
            )],
        )?;
        let persisted = self
            .canonical
            .commit_campaign_invite_acceptance(&draft, &projection)
            .await?;
        let membership = sqlx::query(
            r#"
            SELECT role, granted_at, revoked_at IS NULL AS active
              FROM public.campaign_memberships
             WHERE campaign_id = $1 AND user_id = $2
            "#,
        )
        .bind(&request.campaign_id)
        .bind(&request.accepting_user_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("verify_invited_membership"))?
        .ok_or(CoreDomainRepositoryError::Integrity(
            "membership_projection_missing",
        ))?;
        if membership.get::<String, _>("role") != issued.role.as_database_role()
            || !membership.get::<bool, _>("active")
            || membership.get::<DateTime<Utc>, _>("granted_at") != accepted_at
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "membership_role_conflict",
            ));
        }
        Ok(persisted)
    }

    pub async fn create_character(
        &self,
        metadata: &CoreCommandMetadata,
        request: &CreateCharacterRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0 || metadata.requesting_actor_id != request.owner_user_id {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_create_metadata",
            ));
        }
        self.ensure_campaign_member(&request.campaign_id, &request.owner_user_id)
            .await?;
        let character = Character::draft(
            &request.character_id,
            &request.campaign_id,
            &request.owner_user_id,
            &request.display_name,
        )?;
        let sheet_json = validated_object(&request.sheet_json, "character.sheet_json")?;
        let event = CoreDomainEvent::CharacterCreated {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            character_id: character.character_id.to_string(),
            campaign_id: character.campaign_id.to_string(),
            owner_user_id: character.owner_user_id.to_string(),
            display_name: character.display_name.clone(),
            sheet_version_id: request.sheet_version_id.clone(),
            sheet_json: serde_json::to_string(&sheet_json)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?,
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.character_id,
                ("character", "character.create"),
                &event,
                vec![
                    projection_target("public.characters", &request.character_id),
                    projection_target("public.character_sheet_versions", &request.sheet_version_id),
                ],
            )
            .await?;

        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.characters WHERE character_id = $1",
        )
        .bind(&request.character_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_existing_character"))?
        {
            if existing_sequence == persisted.last_event_sequence
                && self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Ok(persisted);
            }
            return Err(CoreDomainRepositoryError::Integrity(
                "character_identity_conflict",
            ));
        }

        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_character_create")
            .await?;
        sqlx::query(
            r#"
            INSERT INTO public.characters (
                character_id, campaign_id, owner_user_id, display_name,
                state, current_sheet_version, initial_version_locked, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, 'DRAFT', 1, FALSE, 1,
                $5, $6, $7, $8, $9, $10
            )
            "#,
        )
        .bind(&request.character_id)
        .bind(&request.campaign_id)
        .bind(&request.owner_user_id)
        .bind(&request.display_name)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_character"))?;
        sqlx::query(
            r#"
            INSERT INTO public.character_sheet_versions (
                sheet_version_id, character_id, version, sheet_json, locked,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                campaign_id, last_event_sequence
            ) VALUES (
                $1, $2, 1, $3, FALSE, $4, $5, $6, $7, $8, $9, $10
            )
            "#,
        )
        .bind(&request.sheet_version_id)
        .bind(&request.character_id)
        .bind(sqlx::types::Json(sheet_json))
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(&request.campaign_id)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_character_sheet_version"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_character_create"))?;
        Ok(persisted)
    }

    pub async fn submit_character(
        &self,
        metadata: &CoreCommandMetadata,
        campaign_id: &str,
        character_id: &str,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT campaign_id, owner_user_id, display_name, state,
                   current_sheet_version, initial_version_locked, version,
                   last_event_sequence
              FROM public.characters
             WHERE character_id = $1
            "#,
        )
        .bind(character_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_character_for_submit"))?
        .ok_or(CoreDomainRepositoryError::NotFound("character"))?;
        if row.get::<String, _>("campaign_id") != campaign_id
            || row.get::<String, _>("owner_user_id") != metadata.requesting_actor_id
        {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        let current_version: i64 = row.get("version");
        let event = CoreDomainEvent::CharacterSubmitted {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            character_id: character_id.to_owned(),
        };
        let current_state: String = row.get("state");
        let current_event_sequence: i64 = row.get("last_event_sequence");
        if current_state != "DRAFT" {
            if current_state == "SUBMITTED"
                && self
                    .projection_matches_command(current_event_sequence, metadata)
                    .await?
            {
                return self
                    .commit_event(
                        metadata,
                        campaign_id,
                        character_id,
                        ("character", "character.submit"),
                        &event,
                        vec![projection_target("public.characters", character_id)],
                    )
                    .await;
            }
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::CharacterSheetAlreadyLocked,
            ));
        }
        if metadata.expected_version != current_version {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_expected_version",
            ));
        }
        let mut character = Character::draft(
            character_id,
            campaign_id,
            row.get::<String, _>("owner_user_id"),
            row.get::<String, _>("display_name"),
        )?;
        character.current_sheet_version = u64::try_from(row.get::<i64, _>("current_sheet_version"))
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_version"))?;
        character.initial_version_locked = row.get("initial_version_locked");
        character.version = u64::try_from(current_version)
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_version"))?;
        character.submit()?;

        let persisted = self
            .commit_event(
                metadata,
                campaign_id,
                character_id,
                ("character", "character.submit"),
                &event,
                vec![projection_target("public.characters", character_id)],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_character_submit")
            .await?;
        let result = sqlx::query(
            r#"
            UPDATE public.characters
               SET state = 'SUBMITTED',
                   version = version + 1,
                   visibility_label = $1,
                   visibility_subject = $2,
                   provenance_kind = $3,
                   provenance_reference = $4,
                   provenance_recorded_by = $5,
                   last_event_sequence = $6
             WHERE character_id = $7
               AND state = 'DRAFT'
               AND version = $8
            "#,
        )
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(character_id)
        .bind(current_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_character_submit"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "character_submit_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_character_submit"))?;
        Ok(persisted)
    }

    pub async fn approve_character_initial_version(
        &self,
        metadata: &CoreCommandMetadata,
        campaign_id: &str,
        character_id: &str,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        self.ensure_campaign_admin(campaign_id, &metadata.requesting_actor_id)
            .await?;
        let row = sqlx::query(
            r#"
            SELECT campaign_id, owner_user_id, display_name, state,
                   current_sheet_version, initial_version_locked, version,
                   last_event_sequence,
                   (
                       SELECT sheet_version_id
                         FROM public.character_sheet_versions
                        WHERE character_id = $1 AND version = 1
                   ) AS initial_sheet_version_id
              FROM public.characters
             WHERE character_id = $1
            "#,
        )
        .bind(character_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_character_for_review"))?
        .ok_or(CoreDomainRepositoryError::NotFound("character"))?;
        if row.get::<String, _>("campaign_id") != campaign_id {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        let current_version: i64 = row.get("version");
        let event = CoreDomainEvent::CharacterInitialVersionApproved {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            character_id: character_id.to_owned(),
            reviewed_by: metadata.requesting_actor_id.clone(),
        };
        let initial_sheet_version_id = row
            .get::<Option<String>, _>("initial_sheet_version_id")
            .ok_or(CoreDomainRepositoryError::Integrity(
                "initial_character_sheet_missing",
            ))?;
        let projection_targets = || {
            vec![
                projection_target("public.characters", character_id),
                projection_target("public.character_sheet_versions", &initial_sheet_version_id),
            ]
        };
        let current_state: String = row.get("state");
        let current_event_sequence: i64 = row.get("last_event_sequence");
        if current_state != "SUBMITTED" {
            if current_state == "APPROVED"
                && row.get::<bool, _>("initial_version_locked")
                && self
                    .projection_matches_command(current_event_sequence, metadata)
                    .await?
            {
                return self
                    .commit_event(
                        metadata,
                        campaign_id,
                        character_id,
                        ("character", "character.review_initial"),
                        &event,
                        projection_targets(),
                    )
                    .await;
            }
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::CharacterSheetNotSubmitted,
            ));
        }
        if metadata.expected_version != current_version {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "character_expected_version",
            ));
        }
        let mut character = Character::draft(
            character_id,
            campaign_id,
            row.get::<String, _>("owner_user_id"),
            row.get::<String, _>("display_name"),
        )?;
        character.state = CharacterState::Submitted;
        character.current_sheet_version = u64::try_from(row.get::<i64, _>("current_sheet_version"))
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_version"))?;
        character.initial_version_locked = row.get("initial_version_locked");
        character.version = u64::try_from(current_version)
            .map_err(|_| CoreDomainRepositoryError::Integrity("character_version"))?;
        character.approve_initial_version()?;

        let persisted = self
            .commit_event(
                metadata,
                campaign_id,
                character_id,
                ("character", "character.review_initial"),
                &event,
                projection_targets(),
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_character_review")
            .await?;
        let result = sqlx::query(
            r#"
            UPDATE public.characters
               SET state = 'APPROVED',
                   initial_version_locked = TRUE,
                   version = version + 1,
                   visibility_label = $1,
                   visibility_subject = $2,
                   provenance_kind = $3,
                   provenance_reference = $4,
                   provenance_recorded_by = $5,
                   last_event_sequence = $6
             WHERE character_id = $7
               AND state = 'SUBMITTED'
               AND initial_version_locked = FALSE
               AND version = $8
            "#,
        )
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(character_id)
        .bind(current_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_character_review"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "character_review_projection_conflict",
            ));
        }
        sqlx::query(
            r#"
            UPDATE public.character_sheet_versions
               SET locked = TRUE,
                   visibility_label = $1,
                   visibility_subject = $2,
                   provenance_kind = $3,
                   provenance_reference = $4,
                   provenance_recorded_by = $5,
                   last_event_sequence = $6
             WHERE character_id = $7
               AND version = 1
               AND locked = FALSE
            "#,
        )
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(character_id)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("lock_initial_character_sheet"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_character_review"))?;
        Ok(persisted)
    }

    pub async fn import_scenario(
        &self,
        metadata: &CoreCommandMetadata,
        request: &ImportScenarioRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || request.ruleset_id.trim().is_empty()
            || request.format_version.trim().is_empty()
            || !valid_sha256(&request.content_hash)
        {
            return Err(CoreDomainRepositoryError::InvalidInput("scenario"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let document = validated_object(&request.document_json, "scenario.document_json")?;
        let canonical_document = serde_json::to_string(&document)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let actual_hash = format!("sha256:{:x}", Sha256::digest(canonical_document.as_bytes()));
        if actual_hash != request.content_hash {
            return Err(CoreDomainRepositoryError::Integrity(
                "scenario_content_hash_mismatch",
            ));
        }
        let event = CoreDomainEvent::ScenarioImported {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            scenario_id: request.scenario_id.clone(),
            campaign_id: request.campaign_id.clone(),
            ruleset_id: request.ruleset_id.clone(),
            format_version: request.format_version.clone(),
            content_hash: request.content_hash.clone(),
            document_json: canonical_document,
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.scenario_id,
                ("scenario", "scenario.import"),
                &event,
                vec![projection_target("public.scenarios", &request.scenario_id)],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_scenario_import")
            .await?;
        let result = sqlx::query(
            r#"
            INSERT INTO public.scenarios (
                scenario_id, campaign_id, ruleset_id, format_version,
                content_hash, document_json, validated, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, TRUE, 1,
                $7, $8, $9, $10, $11, $12
            )
            ON CONFLICT (scenario_id) DO NOTHING
            "#,
        )
        .bind(&request.scenario_id)
        .bind(&request.campaign_id)
        .bind(&request.ruleset_id)
        .bind(&request.format_version)
        .bind(&request.content_hash)
        .bind(sqlx::types::Json(document))
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_scenario"))?;
        if result.rows_affected() == 0 {
            let existing_sequence: i64 = sqlx::query_scalar(
                "SELECT last_event_sequence FROM public.scenarios WHERE scenario_id = $1",
            )
            .bind(&request.scenario_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error("load_existing_scenario"))?;
            if existing_sequence != persisted.last_event_sequence
                || !self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "scenario_identity_conflict",
                ));
            }
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_scenario_import"))?;
        Ok(persisted)
    }

    pub async fn start_session(
        &self,
        metadata: &CoreCommandMetadata,
        request: &StartSessionRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || request.scene_key.trim().is_empty()
            || request.scene_name.trim().is_empty()
        {
            return Err(CoreDomainRepositoryError::InvalidInput("session_start"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let references_exist: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.rooms AS room
                  JOIN public.scenarios AS scenario
                    ON scenario.campaign_id = room.campaign_id
                 WHERE room.room_id = $1
                   AND scenario.scenario_id = $2
                   AND room.campaign_id = $3
            )
            "#,
        )
        .bind(&request.room_id)
        .bind(&request.scenario_id)
        .bind(&request.campaign_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("load_session_references"))?;
        if !references_exist {
            return Err(CoreDomainRepositoryError::NotFound(
                "session_room_or_scenario",
            ));
        }
        let mut session = Session::scheduled(
            &request.session_id,
            &request.campaign_id,
            &request.room_id,
            &request.scenario_id,
        )?;
        session.transition(SessionState::Active)?;
        let started_at = timestamp_from_unix_ms(request.started_at_unix_ms, "session.started_at")?;
        let event = CoreDomainEvent::SessionStarted {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            session_id: request.session_id.clone(),
            campaign_id: request.campaign_id.clone(),
            room_id: request.room_id.clone(),
            scenario_id: request.scenario_id.clone(),
            scene_id: request.scene_id.clone(),
            scene_key: request.scene_key.clone(),
            scene_name: request.scene_name.clone(),
            started_at_unix_ms: request.started_at_unix_ms,
        };

        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_session_start")
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "core-session:{}:{}",
                request.campaign_id, request.room_id
            ))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_session_room"))?;
        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM core_domain.sessions WHERE session_id = $1",
        )
        .bind(&request.session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_existing_session"))?
        {
            if self
                .projection_matches_command(existing_sequence, metadata)
                .await?
            {
                return self
                    .commit_event(
                        metadata,
                        &request.campaign_id,
                        &request.session_id,
                        ("session", "session.start"),
                        &event,
                        vec![
                            projection_target("core_domain.sessions", &request.session_id),
                            projection_target("public.scenes", &request.scene_id),
                        ],
                    )
                    .await;
            }
            return Err(CoreDomainRepositoryError::ConcurrentStart);
        }
        let live_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM core_domain.sessions
                 WHERE campaign_id = $1
                   AND room_id = $2
                   AND state IN ('ACTIVE', 'PAUSED')
            )
            "#,
        )
        .bind(&request.campaign_id)
        .bind(&request.room_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error("check_live_session"))?;
        if live_exists {
            return Err(CoreDomainRepositoryError::ConcurrentStart);
        }
        let scene_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.scenes WHERE scene_id = $1)")
                .bind(&request.scene_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(database_error("check_scene_identity"))?;
        if scene_exists {
            return Err(CoreDomainRepositoryError::Integrity(
                "scene_identity_conflict",
            ));
        }
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.session_id,
                ("session", "session.start"),
                &event,
                vec![
                    projection_target("core_domain.sessions", &request.session_id),
                    projection_target("public.scenes", &request.scene_id),
                ],
            )
            .await?;
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *transaction)
            .await
            .map_err(database_error("defer_session_constraints"))?;
        sqlx::query(
            r#"
            INSERT INTO core_domain.sessions (
                session_id, campaign_id, room_id, scenario_id, state,
                active_scene_id, started_at, ended_at, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, 'ACTIVE', $5, $6, NULL, 1,
                $7, $8, $9, $10, $11, $12
            )
            "#,
        )
        .bind(&request.session_id)
        .bind(&request.campaign_id)
        .bind(&request.room_id)
        .bind(&request.scenario_id)
        .bind(&request.scene_id)
        .bind(started_at)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_session"))?;
        sqlx::query(
            r#"
            INSERT INTO public.scenes (
                scene_id, campaign_id, session_id, scenario_id, room_id,
                scene_key, name, state, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, 'ACTIVE', 1,
                $8, $9, $10, $11, $12, $13
            )
            "#,
        )
        .bind(&request.scene_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(&request.scenario_id)
        .bind(&request.room_id)
        .bind(&request.scene_key)
        .bind(&request.scene_name)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_opening_scene"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_session_start"))?;
        Ok(persisted)
    }

    pub async fn change_session_state(
        &self,
        metadata: &CoreCommandMetadata,
        campaign_id: &str,
        session_id: &str,
        next_state: SessionState,
        changed_at_unix_ms: u64,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        self.ensure_campaign_admin(campaign_id, &metadata.requesting_actor_id)
            .await?;
        let changed_at = timestamp_from_unix_ms(changed_at_unix_ms, "session.changed_at")?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_session_transition")
            .await?;
        let row = sqlx::query(
            r#"
            SELECT campaign_id, room_id, scenario_id, state, active_scene_id,
                   version, last_event_sequence
              FROM core_domain.sessions
             WHERE session_id = $1
             FOR UPDATE
            "#,
        )
        .bind(session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_session_for_transition"))?
        .ok_or(CoreDomainRepositoryError::NotFound("session"))?;
        if row.get::<String, _>("campaign_id") != campaign_id {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        let current_version: i64 = row.get("version");
        let current_state = parse_session_state(&row.get::<String, _>("state"))?;
        let current_event_sequence: i64 = row.get("last_event_sequence");
        if current_state == next_state
            && self
                .projection_matches_command(current_event_sequence, metadata)
                .await?
        {
            let existing_event = self
                .load_idempotent_core_event(
                    campaign_id,
                    session_id,
                    metadata,
                    "SessionStateChanged",
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_session_event_missing",
                ))?;
            if !matches!(
                &existing_event,
                CoreDomainEvent::SessionStateChanged {
                    session_id: event_session_id,
                    to,
                    changed_at_unix_ms: event_changed_at,
                    ..
                } if event_session_id == session_id
                    && *to == next_state
                    && *event_changed_at == changed_at_unix_ms
            ) {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_session_request_conflict",
                ));
            }
            let mut targets = vec![projection_target("core_domain.sessions", session_id)];
            if next_state == SessionState::Ended {
                let active_scene_id = row
                    .get::<Option<String>, _>("active_scene_id")
                    .ok_or(CoreDomainRepositoryError::Integrity("active_scene_missing"))?;
                targets.push(projection_target("public.scenes", &active_scene_id));
            }
            return self
                .commit_event(
                    metadata,
                    campaign_id,
                    session_id,
                    ("session", session_state_action(next_state)),
                    &existing_event,
                    targets,
                )
                .await;
        }
        if metadata.expected_version != current_version {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "session_expected_version",
            ));
        }
        let mut session = Session::scheduled(
            session_id,
            campaign_id,
            row.get::<String, _>("room_id"),
            row.get::<String, _>("scenario_id"),
        )?;
        session.state = current_state;
        session.active_scene_id = row
            .get::<Option<String>, _>("active_scene_id")
            .map(trpg_domain_core::domain_entities_value_objects::SceneId::new)
            .transpose()?;
        session.version = u64::try_from(current_version)
            .map_err(|_| CoreDomainRepositoryError::Integrity("session_version"))?;
        session.transition(next_state)?;
        let event = CoreDomainEvent::SessionStateChanged {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            session_id: session_id.to_owned(),
            from: current_state,
            to: next_state,
            changed_at_unix_ms,
        };
        let mut projection_targets = vec![projection_target("core_domain.sessions", session_id)];
        if next_state == SessionState::Ended {
            let active_scene_id = row
                .get::<Option<String>, _>("active_scene_id")
                .ok_or(CoreDomainRepositoryError::Integrity("active_scene_missing"))?;
            projection_targets.push(projection_target("public.scenes", &active_scene_id));
        }
        let persisted = self
            .commit_event(
                metadata,
                campaign_id,
                session_id,
                ("session", session_state_action(next_state)),
                &event,
                projection_targets,
            )
            .await?;
        if next_state == SessionState::Ended {
            sqlx::query(
                r#"
                UPDATE public.scenes
                   SET state = 'CLOSED',
                       version = version + 1,
                       visibility_label = $1,
                       visibility_subject = $2,
                       provenance_kind = $3,
                       provenance_reference = $4,
                       provenance_recorded_by = $5,
                       last_event_sequence = $6
                 WHERE scene_id = $7
                   AND state = 'ACTIVE'
                "#,
            )
            .bind(&metadata.visibility_label)
            .bind(&metadata.visibility_subject)
            .bind(&metadata.provenance_kind)
            .bind(&metadata.provenance_reference)
            .bind(&metadata.provenance_recorded_by)
            .bind(persisted.last_event_sequence)
            .bind(row.get::<Option<String>, _>("active_scene_id"))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("close_ending_scene"))?;
        }
        let result = sqlx::query(
            r#"
            UPDATE core_domain.sessions
               SET state = $1,
                   ended_at = CASE WHEN $1 = 'ENDED' THEN $2 ELSE NULL END,
                   version = version + 1,
                   visibility_label = $3,
                   visibility_subject = $4,
                   provenance_kind = $5,
                   provenance_reference = $6,
                   provenance_recorded_by = $7,
                   last_event_sequence = $8
             WHERE session_id = $9
               AND state = $10
               AND version = $11
            "#,
        )
        .bind(next_state.as_str())
        .bind(changed_at)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(session_id)
        .bind(current_state.as_str())
        .bind(current_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_session_transition"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "session_transition_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_session_transition"))?;
        Ok(persisted)
    }

    pub async fn switch_scene(
        &self,
        metadata: &CoreCommandMetadata,
        request: &SwitchSceneRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if request.next_scene_key.trim().is_empty() || request.next_scene_name.trim().is_empty() {
            return Err(CoreDomainRepositoryError::InvalidInput("next_scene"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let _switched_at =
            timestamp_from_unix_ms(request.switched_at_unix_ms, "scene.switched_at")?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_scene_switch")
            .await?;
        let session_row = sqlx::query(
            r#"
            SELECT campaign_id, room_id, scenario_id, state, active_scene_id,
                   version, last_event_sequence
              FROM core_domain.sessions
             WHERE session_id = $1
             FOR UPDATE
            "#,
        )
        .bind(&request.session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_session_for_scene_switch"))?
        .ok_or(CoreDomainRepositoryError::NotFound("session"))?;
        if session_row.get::<String, _>("campaign_id") != request.campaign_id {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        if session_row.get::<String, _>("state") != "ACTIVE" {
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::InvalidTransition {
                    aggregate: "scene",
                    from: "INACTIVE_SESSION",
                    to: "ACTIVE",
                },
            ));
        }
        let current_version: i64 = session_row.get("version");
        let current_active_scene_id = session_row.get::<Option<String>, _>("active_scene_id");
        let current_event_sequence: i64 = session_row.get("last_event_sequence");
        if current_active_scene_id.as_deref() == Some(request.next_scene_id.as_str())
            && self
                .projection_matches_command(current_event_sequence, metadata)
                .await?
        {
            let existing_event = self
                .load_idempotent_core_event(
                    &request.campaign_id,
                    &request.session_id,
                    metadata,
                    "SceneSwitched",
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_scene_event_missing",
                ))?;
            let previous_scene_id = match &existing_event {
                CoreDomainEvent::SceneSwitched {
                    session_id,
                    previous_scene_id,
                    next_scene_id,
                    next_scene_key,
                    next_scene_name,
                    switched_at_unix_ms,
                    ..
                } if session_id == &request.session_id
                    && next_scene_id == &request.next_scene_id
                    && next_scene_key == &request.next_scene_key
                    && next_scene_name == &request.next_scene_name
                    && *switched_at_unix_ms == request.switched_at_unix_ms =>
                {
                    previous_scene_id.clone()
                }
                _ => {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "idempotent_scene_request_conflict",
                    ))
                }
            };
            return self
                .commit_event(
                    metadata,
                    &request.campaign_id,
                    &request.session_id,
                    ("session", "scene.switch"),
                    &existing_event,
                    vec![
                        projection_target("core_domain.sessions", &request.session_id),
                        projection_target("public.scenes", &previous_scene_id),
                        projection_target("public.scenes", &request.next_scene_id),
                    ],
                )
                .await;
        }
        if metadata.expected_version != current_version {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "session_expected_version",
            ));
        }
        let previous_scene_id = current_active_scene_id
            .ok_or(CoreDomainRepositoryError::Integrity("active_scene_missing"))?;
        let identity_conflict: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM public.scenes
                 WHERE scene_id = $1
                    OR (session_id = $2 AND scene_key = $3)
            )
            "#,
        )
        .bind(&request.next_scene_id)
        .bind(&request.session_id)
        .bind(&request.next_scene_key)
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error("check_next_scene_identity"))?;
        if identity_conflict {
            return Err(CoreDomainRepositoryError::Integrity(
                "scene_identity_conflict",
            ));
        }
        let event = CoreDomainEvent::SceneSwitched {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            session_id: request.session_id.clone(),
            previous_scene_id: previous_scene_id.clone(),
            next_scene_id: request.next_scene_id.clone(),
            next_scene_key: request.next_scene_key.clone(),
            next_scene_name: request.next_scene_name.clone(),
            switched_at_unix_ms: request.switched_at_unix_ms,
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.session_id,
                ("session", "scene.switch"),
                &event,
                vec![
                    projection_target("core_domain.sessions", &request.session_id),
                    projection_target("public.scenes", &previous_scene_id),
                    projection_target("public.scenes", &request.next_scene_id),
                ],
            )
            .await?;
        let closed = sqlx::query(
            r#"
            UPDATE public.scenes
               SET state = 'CLOSED',
                   version = version + 1,
                   visibility_label = $1,
                   visibility_subject = $2,
                   provenance_kind = $3,
                   provenance_reference = $4,
                   provenance_recorded_by = $5,
                   last_event_sequence = $6
             WHERE scene_id = $7
               AND session_id = $8
               AND state = 'ACTIVE'
            "#,
        )
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(&previous_scene_id)
        .bind(&request.session_id)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("close_previous_scene"))?;
        if closed.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "previous_scene_not_active",
            ));
        }
        sqlx::query(
            r#"
            INSERT INTO public.scenes (
                scene_id, campaign_id, session_id, scenario_id, room_id,
                scene_key, name, state, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, 'ACTIVE', 1,
                $8, $9, $10, $11, $12, $13
            )
            "#,
        )
        .bind(&request.next_scene_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(session_row.get::<String, _>("scenario_id"))
        .bind(session_row.get::<String, _>("room_id"))
        .bind(&request.next_scene_key)
        .bind(&request.next_scene_name)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_next_scene"))?;
        sqlx::query(
            r#"
            UPDATE core_domain.sessions
               SET active_scene_id = $1,
                   version = version + 1,
                   visibility_label = $2,
                   visibility_subject = $3,
                   provenance_kind = $4,
                   provenance_reference = $5,
                   provenance_recorded_by = $6,
                   last_event_sequence = $7
             WHERE session_id = $8
               AND active_scene_id = $9
               AND version = $10
            "#,
        )
        .bind(&request.next_scene_id)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(&request.session_id)
        .bind(&previous_scene_id)
        .bind(current_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_active_scene"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_scene_switch"))?;
        Ok(persisted)
    }

    pub async fn rebuild_session_scene_projection(
        &self,
        campaign_id: &str,
    ) -> Result<SessionProjectionRebuildReport, CoreDomainRepositoryError> {
        let replay = self.load_campaign_events(campaign_id).await?;
        let session_events = replay
            .into_iter()
            .filter(|event| {
                matches!(
                    event.event_type.as_str(),
                    "SessionStarted" | "SessionStateChanged" | "SceneSwitched"
                )
            })
            .collect::<Vec<_>>();
        let last_event_sequence = session_events
            .last()
            .map(|event| event.sequence)
            .unwrap_or(0);
        let mut transaction = self
            .primary
            .begin()
            .await
            .map_err(database_error("begin_session_projection_rebuild"))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("core-session-rebuild:{campaign_id}"))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_session_projection_rebuild"))?;
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *transaction)
            .await
            .map_err(database_error("defer_rebuild_constraints"))?;
        for replay_event in &session_events {
            let commit_id: String = sqlx::query_scalar(
                r#"
                SELECT commit_id
                  FROM public.formal_commits
                 WHERE $1 BETWEEN first_event_sequence AND last_event_sequence
                   AND campaign_id = $2
                   AND status = 'committed'
                "#,
            )
            .bind(replay_event.sequence)
            .bind(campaign_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error("load_rebuild_projection_commit"))?;
            self.set_projection_capability(
                &mut transaction,
                &commit_id,
                "set_rebuild_projection_capability",
            )
            .await?;
            apply_session_replay_event(&mut transaction, replay_event).await?;
        }
        let restored_sessions: i64 =
            sqlx::query_scalar("SELECT count(*) FROM core_domain.sessions WHERE campaign_id = $1")
                .bind(campaign_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(database_error("count_rebuilt_sessions"))?;
        let restored_scenes: i64 =
            sqlx::query_scalar("SELECT count(*) FROM public.scenes WHERE campaign_id = $1")
                .bind(campaign_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(database_error("count_rebuilt_scenes"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_session_projection_rebuild"))?;
        Ok(SessionProjectionRebuildReport {
            campaign_id: campaign_id.to_owned(),
            replayed_events: session_events.len(),
            restored_sessions,
            restored_scenes,
            last_event_sequence,
        })
    }

    pub async fn record_campaign_fork(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordCampaignForkRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || request.parent_campaign_id == request.child_campaign_id
            || request.reason.trim().is_empty()
            || request.reason.len() > 512
            || !valid_sha256(&request.snapshot_hash)
        {
            return Err(CoreDomainRepositoryError::InvalidInput("campaign_fork"));
        }
        self.ensure_campaign_admin(&request.parent_campaign_id, &metadata.requesting_actor_id)
            .await?;
        let references_exist: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM core_domain.sessions AS source_session
                  JOIN public.campaigns AS child
                    ON child.campaign_id = $1
                 WHERE source_session.session_id = $2
                   AND source_session.campaign_id = $3
            )
            "#,
        )
        .bind(&request.child_campaign_id)
        .bind(&request.source_session_id)
        .bind(&request.parent_campaign_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("load_campaign_fork_references"))?;
        if !references_exist {
            return Err(CoreDomainRepositoryError::NotFound(
                "fork_campaign_or_session",
            ));
        }
        let event = CoreDomainEvent::CampaignForkRecorded {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            fork_id: request.fork_id.clone(),
            parent_campaign_id: request.parent_campaign_id.clone(),
            child_campaign_id: request.child_campaign_id.clone(),
            source_session_id: request.source_session_id.clone(),
            snapshot_hash: request.snapshot_hash.clone(),
            reason: request.reason.clone(),
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.parent_campaign_id,
                &request.fork_id,
                ("campaign_fork", "campaign.fork.record"),
                &event,
                vec![projection_target("public.campaign_forks", &request.fork_id)],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_campaign_fork")
            .await?;
        let result = sqlx::query(
            r#"
            INSERT INTO public.campaign_forks (
                fork_id, campaign_id, parent_campaign_id, child_campaign_id,
                source_session_id, source_snapshot_hash, reason, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $2, $3, $4, $5, $6, 1,
                $7, $8, $9, $10, $11, $12
            )
            ON CONFLICT (fork_id) DO NOTHING
            "#,
        )
        .bind(&request.fork_id)
        .bind(&request.parent_campaign_id)
        .bind(&request.child_campaign_id)
        .bind(&request.source_session_id)
        .bind(&request.snapshot_hash)
        .bind(&request.reason)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_campaign_fork"))?;
        if result.rows_affected() == 0 {
            let existing_sequence: i64 = sqlx::query_scalar(
                "SELECT last_event_sequence FROM public.campaign_forks WHERE fork_id = $1",
            )
            .bind(&request.fork_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error("load_existing_campaign_fork"))?;
            if existing_sequence != persisted.last_event_sequence
                || !self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "campaign_fork_identity_conflict",
                ));
            }
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_campaign_fork"))?;
        Ok(persisted)
    }

    pub async fn request_reconsideration(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RequestReconsiderationRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || metadata.requesting_actor_id != request.requested_by
            || request.original_event_sequence <= 0
            || request.reason.trim().is_empty()
            || request.reason.len() > 512
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "reconsideration_request",
            ));
        }
        self.ensure_campaign_member(&request.campaign_id, &request.requested_by)
            .await?;
        let original_is_canonical: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM public.event_store
                 WHERE sequence = $1
                   AND campaign_id = $2
                   AND integrity_status = 'verified_hmac'
                   AND request_hash_source = 'formal_commit'
            )
            "#,
        )
        .bind(request.original_event_sequence)
        .bind(&request.campaign_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("load_reconsideration_source_event"))?;
        if !original_is_canonical {
            return Err(CoreDomainRepositoryError::NotFound(
                "reconsideration_source_event",
            ));
        }
        let event = CoreDomainEvent::ReconsiderationRequested {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            reconsideration_id: request.reconsideration_id.clone(),
            campaign_id: request.campaign_id.clone(),
            original_event_sequence: u64::try_from(request.original_event_sequence)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("original_event_sequence"))?,
            requested_by: request.requested_by.clone(),
            reason: request.reason.clone(),
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.reconsideration_id,
                ("reconsideration", "reconsideration.request"),
                &event,
                vec![projection_target(
                    "public.reconsiderations",
                    &request.reconsideration_id,
                )],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_reconsideration_request")
            .await?;
        let event_chain = Value::Array(vec![Value::String(metadata.command_id.clone())]);
        let result = sqlx::query(
            r#"
            INSERT INTO public.reconsiderations (
                reconsideration_id, campaign_id, original_event_sequence,
                requested_by, reason, state, resolution, event_chain, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, 'REQUESTED', NULL, $6, 1,
                $7, $8, $9, $10, $11, $12
            )
            ON CONFLICT (reconsideration_id) DO NOTHING
            "#,
        )
        .bind(&request.reconsideration_id)
        .bind(&request.campaign_id)
        .bind(request.original_event_sequence)
        .bind(&request.requested_by)
        .bind(&request.reason)
        .bind(sqlx::types::Json(event_chain))
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("insert_reconsideration"))?;
        if result.rows_affected() == 0 {
            let existing_sequence: i64 = sqlx::query_scalar(
                "SELECT last_event_sequence FROM public.reconsiderations \
                 WHERE reconsideration_id = $1",
            )
            .bind(&request.reconsideration_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error("load_existing_reconsideration"))?;
            if existing_sequence != persisted.last_event_sequence
                || !self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_identity_conflict",
                ));
            }
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_reconsideration_request"))?;
        Ok(persisted)
    }

    pub async fn review_reconsideration(
        &self,
        metadata: &CoreCommandMetadata,
        request: &ReviewReconsiderationRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if request.review_event_id.trim().is_empty()
            || (request.resolved && request.resolution.trim().is_empty())
            || request.resolution.len() > 512
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "reconsideration_review",
            ));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let row = sqlx::query(
            r#"
            SELECT campaign_id, state, version, last_event_sequence
              FROM public.reconsiderations
             WHERE reconsideration_id = $1
            "#,
        )
        .bind(&request.reconsideration_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_reconsideration_for_review"))?
        .ok_or(CoreDomainRepositoryError::NotFound("reconsideration"))?;
        if row.get::<String, _>("campaign_id") != request.campaign_id {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        let current_version: i64 = row.get("version");
        let current_event_sequence: i64 = row.get("last_event_sequence");
        if self
            .projection_matches_command(current_event_sequence, metadata)
            .await?
        {
            let existing_event = self
                .load_idempotent_core_event(
                    &request.campaign_id,
                    &request.reconsideration_id,
                    metadata,
                    "ReconsiderationReviewed",
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_reconsideration_event_missing",
                ))?;
            if !matches!(
                &existing_event,
                CoreDomainEvent::ReconsiderationReviewed {
                    reconsideration_id,
                    review_event_id,
                    resolved,
                    resolution,
                    ..
                } if reconsideration_id == &request.reconsideration_id
                    && review_event_id == &request.review_event_id
                    && *resolved == request.resolved
                    && resolution == &request.resolution
            ) {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_reconsideration_request_conflict",
                ));
            }
            return self
                .commit_event(
                    metadata,
                    &request.campaign_id,
                    &request.reconsideration_id,
                    ("reconsideration", "reconsideration.review"),
                    &existing_event,
                    vec![projection_target(
                        "public.reconsiderations",
                        &request.reconsideration_id,
                    )],
                )
                .await;
        }
        if metadata.expected_version != current_version {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "reconsideration_expected_version",
            ));
        }
        if row.get::<String, _>("state") == "RESOLVED" {
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::EventChainInvalid,
            ));
        }
        let event = CoreDomainEvent::ReconsiderationReviewed {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            reconsideration_id: request.reconsideration_id.clone(),
            review_event_id: request.review_event_id.clone(),
            resolved: request.resolved,
            resolution: request.resolution.clone(),
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.reconsideration_id,
                ("reconsideration", "reconsideration.review"),
                &event,
                vec![projection_target(
                    "public.reconsiderations",
                    &request.reconsideration_id,
                )],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_reconsideration_review")
            .await?;
        let state = if request.resolved {
            "RESOLVED"
        } else {
            "REVIEWED"
        };
        let resolution = request
            .resolved
            .then(|| request.resolution.trim().to_owned());
        let result = sqlx::query(
            r#"
            UPDATE public.reconsiderations
               SET state = $1,
                   resolution = $2,
                   event_chain = event_chain || jsonb_build_array($3::TEXT),
                   version = version + 1,
                   visibility_label = $4,
                   visibility_subject = $5,
                   provenance_kind = $6,
                   provenance_reference = $7,
                   provenance_recorded_by = $8,
                   last_event_sequence = $9
             WHERE reconsideration_id = $10
               AND state <> 'RESOLVED'
               AND version = $11
            "#,
        )
        .bind(state)
        .bind(resolution)
        .bind(&request.review_event_id)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(&request.reconsideration_id)
        .bind(current_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_reconsideration_review"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "reconsideration_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_reconsideration_review"))?;
        Ok(persisted)
    }
}
