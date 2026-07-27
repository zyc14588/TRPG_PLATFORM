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

use std::collections::BTreeMap;
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
use trpg_domain_core::canonical_gameplay_state::{
    inspect_chase_state, inspect_combat_state, validate_chase_server_roll_evidence,
    validate_chase_state_transition, validate_combat_server_roll_evidence,
    validate_combat_state_transition,
};
pub use trpg_domain_core::domain_entities_value_objects::MembershipRole;
use trpg_domain_core::domain_entities_value_objects::{
    CampaignAggregate, CampaignForkMaterializedRow, CampaignInvite, Character, CharacterState,
    CoreDomainEvent, CoreEntityError, ReconsiderationOutcome, Room, Session, SessionState, UserId,
};
use trpg_domain_core::fork_canon_lineage::{CopyScope, DEFAULT_PUBLIC_COPY_SCOPES};
use trpg_shared_kernel::{
    EntityId, EventActorOriginWire, ServerDamageRoll, ServerGrowthRollEvidence,
    ServerPercentileRoll,
};

use crate::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, CanonicalEventVisibility, CanonicalProjectionTarget,
    CanonicalReplayEvent, CanonicalStoreError, PersistedCommit, PolicyAuditDraft,
    PostgresCanonicalStore,
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
                visibility: None,
                projection_targets,
            }],
            audit: self.audit.clone(),
        })
    }

    fn to_multi_event_draft(
        &self,
        campaign_id: &str,
        stream_id: &str,
        resource_type: &str,
        action: &str,
        events: Vec<(CoreDomainEvent, Vec<CanonicalProjectionTarget>)>,
    ) -> Result<AtomicCommitDraft, CoreDomainRepositoryError> {
        let mut events = events.into_iter();
        let (first_event, first_targets) = events
            .next()
            .ok_or(CoreDomainRepositoryError::InvalidInput("canonical_events"))?;
        let mut draft = self.to_draft(
            campaign_id,
            stream_id,
            resource_type,
            action,
            &first_event,
            first_targets,
        )?;
        for (event, projection_targets) in events {
            event.validate_schema_version()?;
            draft.events.push(CanonicalEventDraft {
                event_type: event.event_type().to_owned(),
                payload_json: serde_json::to_string(&event)
                    .map_err(|_| CoreDomainRepositoryError::Serialization)?,
                visibility: None,
                projection_targets,
            });
        }
        Ok(draft)
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

#[cfg(test)]
mod fork_materialization_tests {
    use super::*;

    #[test]
    fn content_address_reference_and_batches_bound_large_snapshots() {
        let oversized_snapshot = serde_json::json!({
            "schema_version": 1,
            "state": {
                "public_events": [{
                    "payload": "x".repeat(1_200_000)
                }]
            }
        });
        let oversized_snapshot_json = serde_json::to_string(&oversized_snapshot).unwrap();
        assert!(oversized_snapshot_json.len() > 1_048_576);
        let snapshot_hash = format!(
            "sha256:{:x}",
            Sha256::digest(oversized_snapshot_json.as_bytes())
        );
        let reference = fork_snapshot_reference_json(&snapshot_hash).unwrap();
        assert!(reference.len() < 1_024);
        assert!(!reference.contains(&"x".repeat(1_024)));
        assert_eq!(
            serde_json::from_str::<Value>(&reference).unwrap()["content_address"],
            snapshot_hash
        );

        let rows = (0..8)
            .map(|index| CampaignForkMaterializedRow::PublicEvent {
                fork_event_id: format!("public_event_{index}"),
                source_event_sequence: index + 1,
                source_event_type: "PublicFactRecorded".to_owned(),
                source_resource_type: "scene".to_owned(),
                source_resource_id: format!("scene_{index}"),
                source_payload_json: serde_json::to_string(&serde_json::json!({
                    "payload": "y".repeat(200_000)
                }))
                .unwrap(),
                source_event_integrity_hash: format!("hmac-sha256:{}", "a".repeat(64)),
                visibility_label: "party_visible".to_owned(),
                visibility_subject: "not_applicable".to_owned(),
            })
            .collect::<Vec<_>>();
        let batches = fork_materialization_batches(&rows).unwrap();
        assert!(batches.len() > 1);
        assert_eq!(
            batches.iter().map(|batch| batch.rows.len()).sum::<usize>(),
            rows.len()
        );
        assert!(batches.iter().all(|batch| {
            batch.data_subject_id == "not_applicable"
                && serde_json::to_vec(&batch.rows).unwrap().len() <= 786_432
        }));
    }

    #[test]
    fn private_materialization_uses_the_player_as_data_subject() {
        let rows = vec![CampaignForkMaterializedRow::Character {
            character_id: "character_private_fork".to_owned(),
            owner_user_id: "player_private_fork".to_owned(),
            display_name: "Private Investigator".to_owned(),
            state: "APPROVED".to_owned(),
            initial_version_locked: true,
            sheet_version_id: "sheet_private_fork".to_owned(),
            sheet_json: "{}".to_owned(),
            sheet_locked: true,
            visibility_label: "private_to_player".to_owned(),
            visibility_subject: "player_private_fork".to_owned(),
        }];
        let batches = fork_materialization_batches(&rows).unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].data_subject_id, "player_private_fork");
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
pub struct P08ProjectionRebuildReport {
    pub campaign_id: String,
    pub replayed_events: usize,
    pub combat_states: i64,
    pub chase_states: i64,
    pub reconsiderations: i64,
    pub campaign_forks: i64,
    pub fork_materializations: i64,
    pub fork_public_events: i64,
    pub fork_clues: i64,
    pub fork_npc_states: i64,
    pub ending_events: i64,
    pub growth_events: i64,
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
    pub copy_scopes: Vec<CopyScope>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignForkSnapshotPreview {
    pub canonical_snapshot_json: String,
    pub snapshot_hash: String,
    pub copy_scopes: Vec<CopyScope>,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotEnvelope {
    state: ForkSnapshotState,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotState {
    source_campaign_id: String,
    source_session_id: String,
    session_state: ForkSnapshotSession,
    character_state: Vec<ForkSnapshotCharacter>,
    public_events: Vec<ForkSnapshotPublicEvent>,
    discovered_clues: Vec<ForkSnapshotClue>,
    scene_state: Vec<ForkSnapshotScene>,
    world_state: ForkSnapshotWorld,
    combat_state: Vec<ForkSnapshotCombat>,
    chase_state: Vec<ForkSnapshotChase>,
    conclusion_state: Vec<ForkSnapshotConclusion>,
    npc_state: Vec<ForkSnapshotNpcState>,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotSession {
    state: String,
    active_scene_id: Option<String>,
    started_at_unix_ms: u64,
    ended_at_unix_ms: u64,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ForkSnapshotCharacter {
    character_id: String,
    owner_user_id: String,
    display_name: String,
    state: String,
    initial_version_locked: bool,
    visibility_label: String,
    visibility_subject: String,
    current_sheet: Option<ForkSnapshotSheet>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ForkSnapshotSheet {
    sheet_json: Value,
    locked: bool,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotScene {
    scene_id: String,
    scene_key: String,
    name: String,
    state: String,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotWorld {
    ruleset_id: String,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ForkSnapshotPublicEvent {
    sequence: u64,
    event_type: String,
    resource_type: String,
    resource_id: String,
    payload: Value,
    event_integrity_hash: String,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotClue {
    clue_id: String,
    importance: String,
    outcome: String,
    cost: Option<String>,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotCombat {
    combat_id: String,
    status: String,
    round: u64,
    current_turn_index: u64,
    state: Value,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotChase {
    chase_id: String,
    status: String,
    range_band: u8,
    segment: u64,
    state: Value,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotConclusion {
    ending_event_id: String,
    ending_id: String,
    summary: String,
    ended_at_unix_ms: u64,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotNpcState {
    npc_id: String,
    state: Value,
    visibility_label: String,
    visibility_subject: String,
}

struct CampaignForkMaterializationBatch {
    rows: Vec<CampaignForkMaterializedRow>,
    visibility_label: String,
    visibility_subject: String,
    data_subject_id: String,
}

struct CampaignForkMaterialization {
    child_session_id: String,
    child_scenario_id: String,
    child_state_json: String,
    child_snapshot_hash: String,
    rows: Vec<CampaignForkMaterializedRow>,
    batches: Vec<CampaignForkMaterializationBatch>,
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
    pub review_summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolveReconsiderationRequest {
    pub reconsideration_id: String,
    pub campaign_id: String,
    pub resolution_event_id: String,
    pub outcome: ReconsiderationOutcome,
    pub resolution: String,
    pub corrected_event_type: Option<String>,
    pub corrected_payload_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordCombatStateRequest {
    pub campaign_id: String,
    pub session_id: String,
    pub state_json: String,
    pub attacker_roll: Option<ServerPercentileRoll>,
    pub defender_roll: Option<ServerPercentileRoll>,
    pub damage_roll: Option<ServerDamageRoll>,
    pub medical_roll: Option<ServerPercentileRoll>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordChaseStateRequest {
    pub campaign_id: String,
    pub session_id: String,
    pub state_json: String,
    pub participant_rolls: Vec<ServerPercentileRoll>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordEndingRequest {
    pub ending_event_id: String,
    pub campaign_id: String,
    pub session_id: String,
    pub ending_id: String,
    pub summary: String,
    pub ended_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordGrowthRequest {
    pub growth_event_id: String,
    pub campaign_id: String,
    pub session_id: String,
    pub ending_event_id: String,
    pub character_id: String,
    pub source_sheet_version_id: String,
    pub new_sheet_version_id: String,
    pub skill_name: String,
    pub growth_rolls: ServerGrowthRollEvidence,
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
        visibility: None,
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

fn fork_child_id(
    fork_id: &str,
    kind: &str,
    source_id: &str,
) -> Result<String, CoreDomainRepositoryError> {
    let digest = format!(
        "{:x}",
        Sha256::digest(format!("{fork_id}:{kind}:{source_id}").as_bytes())
    );
    let value = format!("{kind}_{}", &digest[..32]);
    EntityId::new(&value)
        .map(|id| id.as_str().to_owned())
        .map_err(|_| CoreDomainRepositoryError::InvalidInput("fork_materialized_id"))
}

fn fork_snapshot_reference_json(snapshot_hash: &str) -> Result<String, CoreDomainRepositoryError> {
    serde_json::to_string(&serde_json::json!({
        "schema_version": 1,
        "kind": "CONTENT_ADDRESSED_FORK_SNAPSHOT",
        "content_address": snapshot_hash,
        "representation": "CAMPAIGN_FORK_MATERIALIZED_ROWS_V1",
        "excluded_private_scopes": [
            CopyScope::KeeperNotes,
            CopyScope::HiddenClues,
            CopyScope::PrivateMessages,
            CopyScope::AiInternalMemory
        ]
    }))
    .map_err(|_| CoreDomainRepositoryError::Serialization)
}

fn fork_row_projection_targets(
    row: &CampaignForkMaterializedRow,
) -> Vec<CanonicalProjectionTarget> {
    match row {
        CampaignForkMaterializedRow::Scenario { scenario_id, .. } => {
            vec![projection_target("public.scenarios", scenario_id)]
        }
        CampaignForkMaterializedRow::Character {
            character_id,
            sheet_version_id,
            ..
        } => vec![
            projection_target("public.characters", character_id),
            projection_target("public.character_sheet_versions", sheet_version_id),
        ],
        CampaignForkMaterializedRow::Session { session_id, .. } => {
            vec![projection_target("core_domain.sessions", session_id)]
        }
        CampaignForkMaterializedRow::Scene { scene_id, .. } => {
            vec![projection_target("public.scenes", scene_id)]
        }
        CampaignForkMaterializedRow::PublicEvent { fork_event_id, .. } => {
            vec![projection_target(
                "public.campaign_fork_public_events",
                fork_event_id,
            )]
        }
        CampaignForkMaterializedRow::DiscoveredClue { fork_clue_id, .. } => {
            vec![projection_target(
                "public.campaign_fork_clues",
                fork_clue_id,
            )]
        }
        CampaignForkMaterializedRow::NpcState { npc_state_id, .. } => {
            vec![projection_target(
                "public.campaign_fork_npc_states",
                npc_state_id,
            )]
        }
        CampaignForkMaterializedRow::Combat { combat_id, .. } => {
            vec![projection_target("public.combat_states", combat_id)]
        }
        CampaignForkMaterializedRow::Chase { chase_id, .. } => {
            vec![projection_target("public.chase_states", chase_id)]
        }
        CampaignForkMaterializedRow::Conclusion {
            ending_event_id, ..
        } => vec![projection_target("public.ending_events", ending_event_id)],
    }
}

fn fork_row_visibility(row: &CampaignForkMaterializedRow) -> (&str, &str) {
    match row {
        CampaignForkMaterializedRow::Scenario {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Character {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Session {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Scene {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::PublicEvent {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::DiscoveredClue {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::NpcState {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Combat {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Chase {
            visibility_label,
            visibility_subject,
            ..
        }
        | CampaignForkMaterializedRow::Conclusion {
            visibility_label,
            visibility_subject,
            ..
        } => (visibility_label, visibility_subject),
    }
}

fn fork_row_data_subject(row: &CampaignForkMaterializedRow) -> String {
    let (visibility_label, visibility_subject) = fork_row_visibility(row);
    if matches!(
        visibility_label,
        "private_to_player" | "private_to_group" | "investigator_private"
    ) {
        visibility_subject.to_owned()
    } else {
        "not_applicable".to_owned()
    }
}

fn fork_materialization_batches(
    rows: &[CampaignForkMaterializedRow],
) -> Result<Vec<CampaignForkMaterializationBatch>, CoreDomainRepositoryError> {
    const MAX_TARGETS_PER_EVENT: usize = 32;
    const MAX_ROWS_JSON_BYTES_PER_EVENT: usize = 786_432;
    let mut visibility_groups =
        BTreeMap::<(String, String, String), Vec<CampaignForkMaterializedRow>>::new();
    for row in rows {
        let (label, subject) = fork_row_visibility(row);
        visibility_groups
            .entry((
                label.to_owned(),
                subject.to_owned(),
                fork_row_data_subject(row),
            ))
            .or_default()
            .push(row.clone());
    }
    let mut batches = Vec::new();
    for ((visibility_label, visibility_subject, data_subject_id), grouped_rows) in visibility_groups
    {
        let mut current = Vec::new();
        let mut current_targets = 0_usize;
        for row in grouped_rows {
            let row_targets = row.projection_target_count();
            if row_targets == 0 || row_targets > MAX_TARGETS_PER_EVENT {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_projection_target_shape",
                ));
            }
            let mut candidate = current.clone();
            candidate.push(row.clone());
            let candidate_size = serde_json::to_vec(&candidate)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?
                .len();
            if !current.is_empty()
                && (current_targets + row_targets > MAX_TARGETS_PER_EVENT
                    || candidate_size > MAX_ROWS_JSON_BYTES_PER_EVENT)
            {
                batches.push(CampaignForkMaterializationBatch {
                    rows: std::mem::take(&mut current),
                    visibility_label: visibility_label.clone(),
                    visibility_subject: visibility_subject.clone(),
                    data_subject_id: data_subject_id.clone(),
                });
                current_targets = 0;
            }
            if serde_json::to_vec(&row)
                .map_err(|_| CoreDomainRepositoryError::Serialization)?
                .len()
                > MAX_ROWS_JSON_BYTES_PER_EVENT
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_materialized_row_payload_limit",
                ));
            }
            current.push(row);
            current_targets += row_targets;
        }
        if !current.is_empty() {
            batches.push(CampaignForkMaterializationBatch {
                rows: current,
                visibility_label,
                visibility_subject,
                data_subject_id,
            });
        }
    }
    if batches.is_empty() {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_materialization_empty",
        ));
    }
    Ok(batches)
}

fn fork_character_visibility_is_copyable(
    visibility_label: &str,
    visibility_subject: &str,
    owner_user_id: &str,
) -> bool {
    match visibility_label {
        "public" | "party_visible" => visibility_subject == "not_applicable",
        "private_to_player" | "investigator_private" => visibility_subject == owner_user_id,
        _ => false,
    }
}

fn derive_fork_character_visibility(
    character: &ForkSnapshotCharacter,
    sheet: &ForkSnapshotSheet,
) -> Result<(String, String), CoreDomainRepositoryError> {
    if !fork_character_visibility_is_copyable(
        &character.visibility_label,
        &character.visibility_subject,
        &character.owner_user_id,
    ) || !fork_character_visibility_is_copyable(
        &sheet.visibility_label,
        &sheet.visibility_subject,
        &character.owner_user_id,
    ) {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_character_visibility",
        ));
    }
    // Character and current Sheet share one canonical materialization event.
    // If their source labels differ, derive the least-visible envelope so the
    // fork can never widen either row's audience.
    if matches!(
        sheet.visibility_label.as_str(),
        "private_to_player" | "investigator_private"
    ) {
        return Ok((
            sheet.visibility_label.clone(),
            character.owner_user_id.clone(),
        ));
    }
    if matches!(
        character.visibility_label.as_str(),
        "private_to_player" | "investigator_private"
    ) {
        return Ok((
            character.visibility_label.clone(),
            character.owner_user_id.clone(),
        ));
    }
    if character.visibility_label == "party_visible" || sheet.visibility_label == "party_visible" {
        Ok(("party_visible".to_owned(), "not_applicable".to_owned()))
    } else {
        Ok(("public".to_owned(), "not_applicable".to_owned()))
    }
}

fn reconstruct_fork_characters(
    replay_events: &[CanonicalReplayEvent],
    campaign_id: &str,
    cutoff_event_sequence: i64,
) -> Result<Vec<ForkSnapshotCharacter>, CoreDomainRepositoryError> {
    let mut characters = BTreeMap::<String, ForkSnapshotCharacter>::new();
    let mut action_characters = BTreeMap::<String, String>::new();
    for replay in replay_events
        .iter()
        .filter(|event| event.sequence <= cutoff_event_sequence)
    {
        if replay.campaign_id != campaign_id {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_character_replay_campaign",
            ));
        }
        if matches!(
            replay.event_type.as_str(),
            "CharacterCreated"
                | "CharacterSubmitted"
                | "CharacterInitialVersionApproved"
                | "PlayerActionSubmitted"
                | "SanityLossApplied"
                | "CharacterGrowthApplied"
                | "CampaignForkMaterialized"
        ) && (replay.integrity_status != "verified_hmac"
            || replay.request_hash_source != "formal_commit"
            || replay.event_integrity_hash.is_none())
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_character_replay_unverified",
            ));
        }
        match replay.event_type.as_str() {
            "CharacterCreated" => {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
                    .map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_character_create_payload")
                    })?;
                let CoreDomainEvent::CharacterCreated {
                    character_id,
                    campaign_id: event_campaign_id,
                    owner_user_id,
                    display_name,
                    sheet_json,
                    ..
                } = event
                else {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_character_create_event",
                    ));
                };
                if event_campaign_id != campaign_id || characters.contains_key(&character_id) {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_character_create_chain",
                    ));
                }
                let sheet_json: Value = serde_json::from_str(&sheet_json).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("fork_character_sheet_payload")
                })?;
                if !sheet_json.is_object() {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_character_sheet_shape",
                    ));
                }
                characters.insert(
                    character_id.clone(),
                    ForkSnapshotCharacter {
                        character_id,
                        owner_user_id,
                        display_name,
                        state: "DRAFT".to_owned(),
                        initial_version_locked: false,
                        visibility_label: replay.visibility_label.clone(),
                        visibility_subject: replay.visibility_subject.clone(),
                        current_sheet: Some(ForkSnapshotSheet {
                            sheet_json,
                            locked: false,
                            visibility_label: replay.visibility_label.clone(),
                            visibility_subject: replay.visibility_subject.clone(),
                        }),
                    },
                );
            }
            "CharacterSubmitted" | "CharacterInitialVersionApproved" => {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
                    .map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_character_state_payload")
                    })?;
                let (character_id, approved) = match event {
                    CoreDomainEvent::CharacterSubmitted { character_id, .. } => {
                        (character_id, false)
                    }
                    CoreDomainEvent::CharacterInitialVersionApproved { character_id, .. } => {
                        (character_id, true)
                    }
                    _ => {
                        return Err(CoreDomainRepositoryError::Integrity(
                            "fork_character_state_event",
                        ))
                    }
                };
                let character = characters.get_mut(&character_id).ok_or(
                    CoreDomainRepositoryError::Integrity("fork_character_state_chain"),
                )?;
                character.state = if approved { "APPROVED" } else { "SUBMITTED" }.to_owned();
                character.visibility_label = replay.visibility_label.clone();
                character.visibility_subject = replay.visibility_subject.clone();
                if approved {
                    character.initial_version_locked = true;
                    let sheet = character.current_sheet.as_mut().ok_or(
                        CoreDomainRepositoryError::Integrity("fork_character_sheet_missing"),
                    )?;
                    sheet.locked = true;
                    sheet.visibility_label = replay.visibility_label.clone();
                    sheet.visibility_subject = replay.visibility_subject.clone();
                }
            }
            "PlayerActionSubmitted" => {
                let action_id = replay
                    .payload
                    .get("action_id")
                    .and_then(Value::as_str)
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_player_action_id",
                    ))?;
                let character_id = replay
                    .payload
                    .get("character_id")
                    .and_then(Value::as_str)
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_player_action_character",
                    ))?;
                action_characters.insert(action_id.to_owned(), character_id.to_owned());
            }
            "SanityLossApplied" => {
                let action_id = replay
                    .payload
                    .get("action_id")
                    .and_then(Value::as_str)
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_sanity_action_id",
                    ))?;
                let character_id = action_characters.get(action_id).ok_or(
                    CoreDomainRepositoryError::Integrity("fork_sanity_action_chain"),
                )?;
                let character = characters.get_mut(character_id).ok_or(
                    CoreDomainRepositoryError::Integrity("fork_sanity_character_chain"),
                )?;
                let sheet = character.current_sheet.as_mut().ok_or(
                    CoreDomainRepositoryError::Integrity("fork_sanity_sheet_missing"),
                )?;
                let number = |field: &'static str| {
                    replay
                        .payload
                        .get(field)
                        .and_then(Value::as_u64)
                        .ok_or(CoreDomainRepositoryError::Integrity("fork_sanity_payload"))
                };
                let day_key = replay
                    .payload
                    .get("day_key")
                    .and_then(Value::as_str)
                    .ok_or(CoreDomainRepositoryError::Integrity("fork_sanity_payload"))?;
                let madness_state = replay
                    .payload
                    .get("madness_state")
                    .and_then(Value::as_str)
                    .ok_or(CoreDomainRepositoryError::Integrity("fork_sanity_payload"))?;
                let sanity_before = number("sanity_before")?;
                let prior_sanity = sheet
                    .sheet_json
                    .pointer("/sanity_state/current_sanity")
                    .and_then(Value::as_u64)
                    .or_else(|| {
                        sheet
                            .sheet_json
                            .pointer("/characteristics/power")
                            .and_then(Value::as_u64)
                    })
                    .ok_or(CoreDomainRepositoryError::Integrity("fork_sanity_source"))?;
                if prior_sanity != sanity_before {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_sanity_source_mismatch",
                    ));
                }
                sheet.sheet_json["sanity_state"] = serde_json::json!({
                    "day_key": day_key,
                    "day_start_sanity": number("day_start_sanity")?,
                    "current_sanity": number("sanity_after")?,
                    "day_loss": number("day_loss")?,
                    "madness_state": madness_state,
                });
                sheet.locked = true;
                sheet.visibility_label = replay.visibility_label.clone();
                sheet.visibility_subject = replay.visibility_subject.clone();
                character.visibility_label = replay.visibility_label.clone();
                character.visibility_subject = replay.visibility_subject.clone();
            }
            "CharacterGrowthApplied" => {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
                    .map_err(|_| CoreDomainRepositoryError::Integrity("fork_growth_payload"))?;
                let CoreDomainEvent::CharacterGrowthApplied {
                    campaign_id: event_campaign_id,
                    character_id,
                    skill_name,
                    skill_before,
                    skill_after,
                    ..
                } = event
                else {
                    return Err(CoreDomainRepositoryError::Integrity("fork_growth_event"));
                };
                if event_campaign_id != campaign_id {
                    return Err(CoreDomainRepositoryError::Integrity("fork_growth_campaign"));
                }
                let character = characters.get_mut(&character_id).ok_or(
                    CoreDomainRepositoryError::Integrity("fork_growth_character_chain"),
                )?;
                let sheet = character.current_sheet.as_mut().ok_or(
                    CoreDomainRepositoryError::Integrity("fork_growth_sheet_missing"),
                )?;
                let skill = sheet
                    .sheet_json
                    .get_mut("skills")
                    .and_then(Value::as_object_mut)
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_growth_skills_missing",
                    ))?;
                if skill.get(&skill_name).and_then(Value::as_u64) != Some(u64::from(skill_before)) {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_growth_source_mismatch",
                    ));
                }
                skill.insert(skill_name, Value::from(skill_after));
                sheet.locked = true;
                sheet.visibility_label = replay.visibility_label.clone();
                sheet.visibility_subject = replay.visibility_subject.clone();
                character.visibility_label = replay.visibility_label.clone();
                character.visibility_subject = replay.visibility_subject.clone();
            }
            "CampaignForkMaterialized" => {
                let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
                    .map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_nested_materialization_payload")
                    })?;
                let CoreDomainEvent::CampaignForkMaterialized {
                    child_campaign_id,
                    rows,
                    ..
                } = event
                else {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_nested_materialization_event",
                    ));
                };
                if child_campaign_id != campaign_id {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_nested_materialization_campaign",
                    ));
                }
                for row in rows {
                    let CampaignForkMaterializedRow::Character {
                        character_id,
                        owner_user_id,
                        display_name,
                        state,
                        initial_version_locked,
                        sheet_json,
                        sheet_locked,
                        visibility_label,
                        visibility_subject,
                        ..
                    } = row
                    else {
                        continue;
                    };
                    if visibility_label != replay.visibility_label
                        || visibility_subject != replay.visibility_subject
                        || characters.contains_key(&character_id)
                    {
                        return Err(CoreDomainRepositoryError::Integrity(
                            "fork_nested_character_chain",
                        ));
                    }
                    let sheet_json: Value = serde_json::from_str(&sheet_json).map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_nested_character_sheet")
                    })?;
                    characters.insert(
                        character_id.clone(),
                        ForkSnapshotCharacter {
                            character_id,
                            owner_user_id,
                            display_name,
                            state,
                            initial_version_locked,
                            visibility_label: visibility_label.clone(),
                            visibility_subject: visibility_subject.clone(),
                            current_sheet: Some(ForkSnapshotSheet {
                                sheet_json,
                                locked: sheet_locked,
                                visibility_label,
                                visibility_subject,
                            }),
                        },
                    );
                }
            }
            _ => {}
        }
    }
    characters.retain(|_, character| {
        fork_character_visibility_is_copyable(
            &character.visibility_label,
            &character.visibility_subject,
            &character.owner_user_id,
        ) && character.current_sheet.as_ref().is_some_and(|sheet| {
            fork_character_visibility_is_copyable(
                &sheet.visibility_label,
                &sheet.visibility_subject,
                &character.owner_user_id,
            )
        })
    });
    Ok(characters.into_values().collect())
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

async fn apply_combat_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    event: &CoreDomainEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let CoreDomainEvent::CombatStateRecorded {
        combat_id,
        campaign_id,
        session_id,
        status,
        round,
        turn_index,
        version,
        state_json,
        ..
    } = event
    else {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_replay_event_type",
        ));
    };
    if campaign_id != &replay.campaign_id
        || !matches!(status.as_str(), "ONGOING" | "ENDED")
        || *version == 0
        || *round == 0
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_replay_event_shape",
        ));
    }
    let version = i64::try_from(*version)
        .map_err(|_| CoreDomainRepositoryError::Integrity("combat_replay_version"))?;
    let round = i64::try_from(*round)
        .map_err(|_| CoreDomainRepositoryError::Integrity("combat_replay_round"))?;
    let turn_index = i64::try_from(*turn_index)
        .map_err(|_| CoreDomainRepositoryError::Integrity("combat_replay_turn"))?;
    let state_value: Value = serde_json::from_str(state_json)
        .map_err(|_| CoreDomainRepositoryError::Integrity("combat_replay_state"))?;
    if state_value.get("combat_id").and_then(Value::as_str) != Some(combat_id)
        || state_value.get("status").and_then(Value::as_str) != Some(status)
        || state_value.get("round").and_then(Value::as_u64) != u64::try_from(round).ok()
        || state_value
            .get("current_turn_index")
            .and_then(Value::as_u64)
            != u64::try_from(turn_index).ok()
        || state_value.get("version").and_then(Value::as_u64) != u64::try_from(version).ok()
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_replay_state_mismatch",
        ));
    }
    let current: Option<(i64, Value)> =
        sqlx::query_as("SELECT version, state_json FROM public.combat_states WHERE combat_id = $1")
            .bind(combat_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(database_error("load_combat_replay_state"))?;
    if current
        .as_ref()
        .is_some_and(|(current, _)| *current > version)
    {
        return Ok(());
    }
    if current
        .as_ref()
        .is_some_and(|(current, _)| *current < version - 1)
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_replay_sequence_gap",
        ));
    }
    if current.as_ref().map(|(current, _)| *current) != Some(version) {
        let previous_json = current
            .as_ref()
            .map(|(_, value)| serde_json::to_string(value))
            .transpose()
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        validate_combat_state_transition(previous_json.as_deref(), state_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("combat_replay_transition"))?;
        sqlx::query(
            r#"
            INSERT INTO public.combat_states (
                combat_id, campaign_id, session_id, status, round,
                current_turn_index, state_json, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7::JSONB, $8,
                $9, $10, $11, $12, $13, $14
            )
            ON CONFLICT (combat_id) DO UPDATE
               SET status = EXCLUDED.status,
                   round = EXCLUDED.round,
                   current_turn_index = EXCLUDED.current_turn_index,
                   state_json = EXCLUDED.state_json,
                   version = EXCLUDED.version,
                   visibility_label = EXCLUDED.visibility_label,
                   visibility_subject = EXCLUDED.visibility_subject,
                   provenance_kind = EXCLUDED.provenance_kind,
                   provenance_reference = EXCLUDED.provenance_reference,
                   provenance_recorded_by = EXCLUDED.provenance_recorded_by,
                   last_event_sequence = EXCLUDED.last_event_sequence
             WHERE combat_states.campaign_id = EXCLUDED.campaign_id
               AND combat_states.session_id = EXCLUDED.session_id
               AND combat_states.version = EXCLUDED.version - 1
               AND combat_states.status = 'ONGOING'
            "#,
        )
        .bind(combat_id)
        .bind(campaign_id)
        .bind(session_id)
        .bind(status)
        .bind(round)
        .bind(turn_index)
        .bind(state_json)
        .bind(version)
        .bind(&replay.visibility_label)
        .bind(&replay.visibility_subject)
        .bind(&replay.provenance_kind)
        .bind(&replay.provenance_reference)
        .bind(&replay.provenance_recorded_by)
        .bind(replay.sequence)
        .execute(&mut **transaction)
        .await
        .map_err(database_error("replay_combat_state"))?;
    }
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.combat_states
             WHERE combat_id = $1 AND campaign_id = $2 AND session_id = $3
               AND status = $4 AND round = $5 AND current_turn_index = $6
               AND state_json = $7::JSONB AND version = $8
               AND last_event_sequence = $9
        )
        "#,
    )
    .bind(combat_id)
    .bind(campaign_id)
    .bind(session_id)
    .bind(status)
    .bind(round)
    .bind(turn_index)
    .bind(state_json)
    .bind(version)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_combat_state"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "combat_replay_projection_mismatch",
        ));
    }
    Ok(())
}

async fn apply_chase_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    event: &CoreDomainEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let CoreDomainEvent::ChaseStateRecorded {
        chase_id,
        campaign_id,
        session_id,
        status,
        range_band,
        segment,
        version,
        state_json,
        ..
    } = event
    else {
        return Err(CoreDomainRepositoryError::Integrity(
            "chase_replay_event_type",
        ));
    };
    if campaign_id != &replay.campaign_id
        || !matches!(status.as_str(), "ONGOING" | "ESCAPED" | "CAUGHT")
        || *range_band > 5
        || *segment == 0
        || *version == 0
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "chase_replay_event_shape",
        ));
    }
    let version = i64::try_from(*version)
        .map_err(|_| CoreDomainRepositoryError::Integrity("chase_replay_version"))?;
    let segment = i64::try_from(*segment)
        .map_err(|_| CoreDomainRepositoryError::Integrity("chase_replay_segment"))?;
    let range_band = i16::from(*range_band);
    let state_value: Value = serde_json::from_str(state_json)
        .map_err(|_| CoreDomainRepositoryError::Integrity("chase_replay_state"))?;
    if state_value.get("chase_id").and_then(Value::as_str) != Some(chase_id)
        || state_value.get("status").and_then(Value::as_str) != Some(status)
        || state_value.get("range").and_then(Value::as_i64) != Some(i64::from(range_band))
        || state_value.get("segment").and_then(Value::as_u64) != u64::try_from(segment).ok()
        || state_value.get("version").and_then(Value::as_u64) != u64::try_from(version).ok()
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "chase_replay_state_mismatch",
        ));
    }
    let current: Option<(i64, Value)> =
        sqlx::query_as("SELECT version, state_json FROM public.chase_states WHERE chase_id = $1")
            .bind(chase_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(database_error("load_chase_replay_state"))?;
    if current
        .as_ref()
        .is_some_and(|(current, _)| *current > version)
    {
        return Ok(());
    }
    if current
        .as_ref()
        .is_some_and(|(current, _)| *current < version - 1)
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "chase_replay_sequence_gap",
        ));
    }
    if current.as_ref().map(|(current, _)| *current) != Some(version) {
        let previous_json = current
            .as_ref()
            .map(|(_, value)| serde_json::to_string(value))
            .transpose()
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        validate_chase_state_transition(previous_json.as_deref(), state_json)
            .map_err(|_| CoreDomainRepositoryError::Integrity("chase_replay_transition"))?;
        sqlx::query(
            r#"
            INSERT INTO public.chase_states (
                chase_id, campaign_id, session_id, status, range_band,
                segment, state_json, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7::JSONB, $8,
                $9, $10, $11, $12, $13, $14
            )
            ON CONFLICT (chase_id) DO UPDATE
               SET status = EXCLUDED.status,
                   range_band = EXCLUDED.range_band,
                   segment = EXCLUDED.segment,
                   state_json = EXCLUDED.state_json,
                   version = EXCLUDED.version,
                   visibility_label = EXCLUDED.visibility_label,
                   visibility_subject = EXCLUDED.visibility_subject,
                   provenance_kind = EXCLUDED.provenance_kind,
                   provenance_reference = EXCLUDED.provenance_reference,
                   provenance_recorded_by = EXCLUDED.provenance_recorded_by,
                   last_event_sequence = EXCLUDED.last_event_sequence
             WHERE chase_states.campaign_id = EXCLUDED.campaign_id
               AND chase_states.session_id = EXCLUDED.session_id
               AND chase_states.version = EXCLUDED.version - 1
               AND chase_states.status = 'ONGOING'
            "#,
        )
        .bind(chase_id)
        .bind(campaign_id)
        .bind(session_id)
        .bind(status)
        .bind(range_band)
        .bind(segment)
        .bind(state_json)
        .bind(version)
        .bind(&replay.visibility_label)
        .bind(&replay.visibility_subject)
        .bind(&replay.provenance_kind)
        .bind(&replay.provenance_reference)
        .bind(&replay.provenance_recorded_by)
        .bind(replay.sequence)
        .execute(&mut **transaction)
        .await
        .map_err(database_error("replay_chase_state"))?;
    }
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.chase_states
             WHERE chase_id = $1 AND campaign_id = $2 AND session_id = $3
               AND status = $4 AND range_band = $5 AND segment = $6
               AND state_json = $7::JSONB AND version = $8
               AND last_event_sequence = $9
        )
        "#,
    )
    .bind(chase_id)
    .bind(campaign_id)
    .bind(session_id)
    .bind(status)
    .bind(range_band)
    .bind(segment)
    .bind(state_json)
    .bind(version)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_chase_state"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "chase_replay_projection_mismatch",
        ));
    }
    Ok(())
}

async fn apply_reconsideration_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    event: &CoreDomainEvent,
) -> Result<(), CoreDomainRepositoryError> {
    match event {
        CoreDomainEvent::ReconsiderationRequested {
            reconsideration_id,
            campaign_id,
            original_event_sequence,
            requested_by,
            reason,
            ..
        } => {
            if campaign_id != &replay.campaign_id || *original_event_sequence == 0 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_replay_request_shape",
                ));
            }
            let current_version: Option<i64> = sqlx::query_scalar(
                "SELECT version FROM public.reconsiderations WHERE reconsideration_id = $1",
            )
            .bind(reconsideration_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(database_error("load_reconsideration_replay_request"))?;
            if current_version.is_some_and(|current| current > 1) {
                return Ok(());
            }
            if current_version.is_none() {
                sqlx::query(
                    r#"
                    INSERT INTO public.reconsiderations (
                        reconsideration_id, campaign_id, original_event_sequence,
                        requested_by, reason, state, resolution, event_chain, version,
                        review_workflow_version,
                        visibility_label, visibility_subject,
                        provenance_kind, provenance_reference, provenance_recorded_by,
                        last_event_sequence
                    ) VALUES (
                        $1, $2, $3, $4, $5, 'REQUESTED', NULL,
                        jsonb_build_array($6::TEXT), 1, 2,
                        $7, $8, $9, $10, $11, $12
                    )
                    ON CONFLICT (reconsideration_id) DO NOTHING
                    "#,
                )
                .bind(reconsideration_id)
                .bind(campaign_id)
                .bind(i64::try_from(*original_event_sequence).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("reconsideration_source_sequence")
                })?)
                .bind(requested_by)
                .bind(reason)
                .bind(&replay.command_id)
                .bind(&replay.visibility_label)
                .bind(&replay.visibility_subject)
                .bind(&replay.provenance_kind)
                .bind(&replay.provenance_reference)
                .bind(&replay.provenance_recorded_by)
                .bind(replay.sequence)
                .execute(&mut **transaction)
                .await
                .map_err(database_error("replay_reconsideration_request"))?;
            }
            let matches: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM public.reconsiderations
                     WHERE reconsideration_id = $1 AND campaign_id = $2
                       AND original_event_sequence = $3 AND requested_by = $4
                       AND reason = $5 AND state = 'REQUESTED'
                       AND event_chain = jsonb_build_array($6::TEXT)
                       AND version = 1 AND review_workflow_version = 2
                       AND last_event_sequence = $7
                )
                "#,
            )
            .bind(reconsideration_id)
            .bind(campaign_id)
            .bind(i64::try_from(*original_event_sequence).map_err(|_| {
                CoreDomainRepositoryError::Integrity("reconsideration_source_sequence")
            })?)
            .bind(requested_by)
            .bind(reason)
            .bind(&replay.command_id)
            .bind(replay.sequence)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_reconsideration_request"))?;
            if !matches {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_request_replay_mismatch",
                ));
            }
        }
        CoreDomainEvent::ReconsiderationReviewed {
            reconsideration_id,
            review_event_id,
            review_summary,
            ..
        } => {
            let current_version: i64 = sqlx::query_scalar(
                "SELECT version FROM public.reconsiderations WHERE reconsideration_id = $1",
            )
            .bind(reconsideration_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("load_reconsideration_replay_review"))?;
            if current_version > 2 {
                return Ok(());
            }
            if current_version == 1 {
                sqlx::query(
                    r#"
                    UPDATE public.reconsiderations
                       SET state = 'REVIEWED',
                           review_summary = $1,
                           event_chain = event_chain || jsonb_build_array($2::TEXT),
                           version = 2,
                           visibility_label = $3,
                           visibility_subject = $4,
                           provenance_kind = $5,
                           provenance_reference = $6,
                           provenance_recorded_by = $7,
                           last_event_sequence = $8
                     WHERE reconsideration_id = $9
                       AND state = 'REQUESTED' AND version = 1
                    "#,
                )
                .bind(review_summary)
                .bind(review_event_id)
                .bind(&replay.visibility_label)
                .bind(&replay.visibility_subject)
                .bind(&replay.provenance_kind)
                .bind(&replay.provenance_reference)
                .bind(&replay.provenance_recorded_by)
                .bind(replay.sequence)
                .bind(reconsideration_id)
                .execute(&mut **transaction)
                .await
                .map_err(database_error("replay_reconsideration_review"))?;
            }
            let matches: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM public.reconsiderations
                     WHERE reconsideration_id = $1 AND state = 'REVIEWED'
                       AND review_summary = $2
                       AND event_chain ->> 1 = $3
                       AND jsonb_array_length(event_chain) = 2
                       AND version = 2 AND last_event_sequence = $4
                )
                "#,
            )
            .bind(reconsideration_id)
            .bind(review_summary)
            .bind(review_event_id)
            .bind(replay.sequence)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_reconsideration_review"))?;
            if !matches {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_review_replay_mismatch",
                ));
            }
        }
        CoreDomainEvent::ReconsiderationUpheld {
            reconsideration_id,
            resolution_event_id,
            original_event_sequence,
            resolution,
            ..
        }
        | CoreDomainEvent::ReconsiderationCorrected {
            reconsideration_id,
            resolution_event_id,
            original_event_sequence,
            resolution,
            ..
        } => {
            let current_version: i64 = sqlx::query_scalar(
                "SELECT version FROM public.reconsiderations WHERE reconsideration_id = $1",
            )
            .bind(reconsideration_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("load_reconsideration_replay_resolution"))?;
            let (outcome, corrected_event_type, corrected_payload_json) = match event {
                CoreDomainEvent::ReconsiderationUpheld { .. } => ("UPHELD", None, None),
                CoreDomainEvent::ReconsiderationCorrected {
                    corrected_event_type,
                    corrected_payload_json,
                    ..
                } => {
                    let payload: Value =
                        serde_json::from_str(corrected_payload_json).map_err(|_| {
                            CoreDomainRepositoryError::Integrity(
                                "reconsideration_corrected_payload",
                            )
                        })?;
                    if !payload.is_object() {
                        return Err(CoreDomainRepositoryError::Integrity(
                            "reconsideration_corrected_payload",
                        ));
                    }
                    (
                        "CORRECTED",
                        Some(corrected_event_type.as_str()),
                        Some(corrected_payload_json.as_str()),
                    )
                }
                _ => unreachable!("matched reconsideration resolution above"),
            };
            if current_version == 2 {
                sqlx::query(
                    r#"
                    UPDATE public.reconsiderations
                       SET state = 'RESOLVED', outcome = $1, resolution = $2,
                           corrected_event_type = $3,
                           corrected_payload = $4::JSONB,
                           event_chain = event_chain || jsonb_build_array($5::TEXT),
                           version = 3,
                           visibility_label = $6,
                           visibility_subject = $7,
                           provenance_kind = $8,
                           provenance_reference = $9,
                           provenance_recorded_by = $10,
                           last_event_sequence = $11
                     WHERE reconsideration_id = $12
                       AND state = 'REVIEWED' AND version = 2
                       AND original_event_sequence = $13
                    "#,
                )
                .bind(outcome)
                .bind(resolution)
                .bind(corrected_event_type)
                .bind(corrected_payload_json)
                .bind(resolution_event_id)
                .bind(&replay.visibility_label)
                .bind(&replay.visibility_subject)
                .bind(&replay.provenance_kind)
                .bind(&replay.provenance_reference)
                .bind(&replay.provenance_recorded_by)
                .bind(replay.sequence)
                .bind(reconsideration_id)
                .bind(i64::try_from(*original_event_sequence).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("reconsideration_source_sequence")
                })?)
                .execute(&mut **transaction)
                .await
                .map_err(database_error("replay_reconsideration_resolution"))?;
            } else if current_version != 3 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_replay_sequence_gap",
                ));
            }
            let matches: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM public.reconsiderations
                     WHERE reconsideration_id = $1 AND state = 'RESOLVED'
                       AND outcome = $2 AND resolution = $3
                       AND corrected_event_type IS NOT DISTINCT FROM $4
                       AND corrected_payload IS NOT DISTINCT FROM $5::JSONB
                       AND event_chain ->> 2 = $6
                       AND jsonb_array_length(event_chain) = 3
                       AND version = 3 AND last_event_sequence = $7
                )
                "#,
            )
            .bind(reconsideration_id)
            .bind(outcome)
            .bind(resolution)
            .bind(corrected_event_type)
            .bind(corrected_payload_json)
            .bind(resolution_event_id)
            .bind(replay.sequence)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_reconsideration_resolution"))?;
            if !matches {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_resolution_replay_mismatch",
                ));
            }
        }
        _ => {
            return Err(CoreDomainRepositoryError::Integrity(
                "reconsideration_replay_event_type",
            ))
        }
    }
    Ok(())
}

async fn apply_ending_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    event: &CoreDomainEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let CoreDomainEvent::EndingRecorded {
        ending_event_id,
        campaign_id,
        session_id,
        ending_id,
        summary,
        ended_at_unix_ms,
        ..
    } = event
    else {
        return Err(CoreDomainRepositoryError::Integrity(
            "ending_replay_event_type",
        ));
    };
    if campaign_id != &replay.campaign_id
        || ending_id.trim().is_empty()
        || summary.trim().is_empty()
        || summary.len() > 1_024
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "ending_replay_event_shape",
        ));
    }
    let ended_at = timestamp_from_unix_ms(*ended_at_unix_ms, "ending_replay_timestamp")?;
    sqlx::query(
        r#"
        INSERT INTO public.ending_events (
            ending_event_id, campaign_id, session_id, ending_id, summary,
            ended_at, version, visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, $5, $6, 1, $7, $8, $9, $10, $11, $12
        )
        ON CONFLICT (ending_event_id) DO NOTHING
        "#,
    )
    .bind(ending_event_id)
    .bind(campaign_id)
    .bind(session_id)
    .bind(ending_id)
    .bind(summary)
    .bind(ended_at)
    .bind(&replay.visibility_label)
    .bind(&replay.visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_ending"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.ending_events
             WHERE ending_event_id = $1 AND campaign_id = $2
               AND session_id = $3 AND ending_id = $4
               AND summary = $5 AND ended_at = $6
               AND version = 1 AND last_event_sequence = $7
        )
        "#,
    )
    .bind(ending_event_id)
    .bind(campaign_id)
    .bind(session_id)
    .bind(ending_id)
    .bind(summary)
    .bind(ended_at)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_ending"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "ending_replay_projection_mismatch",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn apply_growth_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    growth_event_id: &str,
    campaign_id: &str,
    session_id: &str,
    ending_event_id: &str,
    character_id: &str,
    source_sheet_version_id: &str,
    new_sheet_version_id: &str,
    skill_name: &str,
    skill_before: u8,
    improvement_check_roll: u8,
    increase_roll: Option<u8>,
    skill_after: u8,
    server_roll_id: &str,
    increase_roll_id: Option<&str>,
) -> Result<(), CoreDomainRepositoryError> {
    let qualifies = skill_before < 99
        && (improvement_check_roll > skill_before || improvement_check_roll >= 96);
    let outcome_valid = (1..=100).contains(&improvement_check_roll)
        && match increase_roll {
            Some(increase) => {
                qualifies
                    && (1..=10).contains(&increase)
                    && skill_after == skill_before.saturating_add(increase).min(99)
            }
            None => !qualifies && skill_after == skill_before,
        };
    let evidence_ids_valid = server_roll_id.len() <= 128
        && match (increase_roll, increase_roll_id) {
            (Some(_), Some(increase_id)) => {
                !increase_id.trim().is_empty()
                    && increase_id.len() <= 128
                    && increase_id != server_roll_id
            }
            (None, None) => true,
            _ => false,
        };
    if campaign_id != replay.campaign_id
        || skill_name.trim().is_empty()
        || skill_name.len() > 128
        || server_roll_id.trim().is_empty()
        || !evidence_ids_valid
        || !outcome_valid
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "growth_replay_event_shape",
        ));
    }
    let existing_growth: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.growth_events WHERE growth_event_id = $1)",
    )
    .bind(growth_event_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("load_growth_replay_projection"))?;
    if !existing_growth {
        let source = sqlx::query(
            r#"
            SELECT character.current_sheet_version,
                   character.version AS character_version,
                   sheet.version AS sheet_version,
                   sheet.sheet_json
              FROM public.characters AS character
              JOIN public.character_sheet_versions AS sheet
                ON sheet.character_id = character.character_id
               AND sheet.sheet_version_id = $1
             WHERE character.character_id = $2
               AND character.campaign_id = $3
               AND sheet.campaign_id = $3
               AND sheet.locked
            "#,
        )
        .bind(source_sheet_version_id)
        .bind(character_id)
        .bind(campaign_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(database_error("load_growth_replay_source"))?
        .ok_or(CoreDomainRepositoryError::Integrity(
            "growth_replay_source_missing",
        ))?;
        let source_version: i64 = source.get("sheet_version");
        if source.get::<i64, _>("current_sheet_version") != source_version {
            return Err(CoreDomainRepositoryError::Integrity(
                "growth_replay_source_not_current",
            ));
        }
        let mut sheet_json: Value = source.get("sheet_json");
        if sheet_json
            .get("skills")
            .and_then(Value::as_object)
            .and_then(|skills| skills.get(skill_name))
            .and_then(Value::as_u64)
            != Some(u64::from(skill_before))
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "growth_replay_skill_source",
            ));
        }
        sheet_json
            .get_mut("skills")
            .and_then(Value::as_object_mut)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "growth_replay_skills_missing",
            ))?
            .insert(skill_name.to_owned(), Value::from(skill_after));
        let new_sheet_version =
            source_version
                .checked_add(1)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "growth_replay_sheet_version",
                ))?;
        sqlx::query(
            r#"
            INSERT INTO public.character_sheet_versions (
                sheet_version_id, character_id, version, sheet_json, locked,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                campaign_id, last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, TRUE, $5, $6, $7, $8, $9, $10, $11
            )
            ON CONFLICT (sheet_version_id) DO NOTHING
            "#,
        )
        .bind(new_sheet_version_id)
        .bind(character_id)
        .bind(new_sheet_version)
        .bind(sqlx::types::Json(&sheet_json))
        .bind(&replay.visibility_label)
        .bind(&replay.visibility_subject)
        .bind(&replay.provenance_kind)
        .bind(&replay.provenance_reference)
        .bind(&replay.provenance_recorded_by)
        .bind(campaign_id)
        .bind(replay.sequence)
        .execute(&mut **transaction)
        .await
        .map_err(database_error("replay_growth_sheet"))?;
        sqlx::query(
            r#"
            UPDATE public.characters
               SET current_sheet_version = $1,
                   version = version + 1,
                   visibility_label = $2,
                   visibility_subject = $3,
                   provenance_kind = $4,
                   provenance_reference = $5,
                   provenance_recorded_by = $6,
                   last_event_sequence = $7
             WHERE character_id = $8 AND campaign_id = $9
               AND current_sheet_version = $10 AND version = $11
            "#,
        )
        .bind(new_sheet_version)
        .bind(&replay.visibility_label)
        .bind(&replay.visibility_subject)
        .bind(&replay.provenance_kind)
        .bind(&replay.provenance_reference)
        .bind(&replay.provenance_recorded_by)
        .bind(replay.sequence)
        .bind(character_id)
        .bind(campaign_id)
        .bind(source_version)
        .bind(source.get::<i64, _>("character_version"))
        .execute(&mut **transaction)
        .await
        .map_err(database_error("replay_growth_character"))?;
        sqlx::query(
            r#"
            INSERT INTO public.growth_events (
                growth_event_id, campaign_id, session_id, ending_event_id,
                character_id, source_sheet_version_id, new_sheet_version_id,
                skill_name, skill_before, improvement_check_roll,
                increase_roll, skill_after, server_roll_id, increase_roll_id, random_source,
                version, visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, 'SERVER_OS_CSPRNG', 1, $15, $16,
                $17, $18, $19, $20
            )
            ON CONFLICT (growth_event_id) DO NOTHING
            "#,
        )
        .bind(growth_event_id)
        .bind(campaign_id)
        .bind(session_id)
        .bind(ending_event_id)
        .bind(character_id)
        .bind(source_sheet_version_id)
        .bind(new_sheet_version_id)
        .bind(skill_name)
        .bind(i16::from(skill_before))
        .bind(i16::from(improvement_check_roll))
        .bind(increase_roll.map(i16::from))
        .bind(i16::from(skill_after))
        .bind(server_roll_id)
        .bind(increase_roll_id)
        .bind(&replay.visibility_label)
        .bind(&replay.visibility_subject)
        .bind(&replay.provenance_kind)
        .bind(&replay.provenance_reference)
        .bind(&replay.provenance_recorded_by)
        .bind(replay.sequence)
        .execute(&mut **transaction)
        .await
        .map_err(database_error("replay_growth_event"))?;
    }
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
              FROM public.growth_events AS growth
              JOIN public.characters AS character
                ON character.character_id = growth.character_id
               AND character.campaign_id = growth.campaign_id
              JOIN public.character_sheet_versions AS sheet
                ON sheet.sheet_version_id = growth.new_sheet_version_id
               AND sheet.character_id = growth.character_id
             WHERE growth.growth_event_id = $1
               AND growth.campaign_id = $2 AND growth.session_id = $3
               AND growth.ending_event_id = $4 AND growth.character_id = $5
               AND growth.source_sheet_version_id = $6
               AND growth.new_sheet_version_id = $7
               AND growth.skill_name = $8 AND growth.skill_before = $9
               AND growth.improvement_check_roll = $10
               AND growth.increase_roll IS NOT DISTINCT FROM $11
               AND growth.skill_after = $12 AND growth.server_roll_id = $13
               AND growth.increase_roll_id IS NOT DISTINCT FROM $14
               AND growth.random_source = 'SERVER_OS_CSPRNG'
               AND growth.last_event_sequence = $15
               AND character.current_sheet_version = sheet.version
               AND character.last_event_sequence = $15
               AND sheet.sheet_json -> 'skills' ->> $8 = $12::TEXT
               AND sheet.last_event_sequence = $15
        )
        "#,
    )
    .bind(growth_event_id)
    .bind(campaign_id)
    .bind(session_id)
    .bind(ending_event_id)
    .bind(character_id)
    .bind(source_sheet_version_id)
    .bind(new_sheet_version_id)
    .bind(skill_name)
    .bind(i16::from(skill_before))
    .bind(i16::from(improvement_check_roll))
    .bind(increase_roll.map(i16::from))
    .bind(i16::from(skill_after))
    .bind(server_roll_id)
    .bind(increase_roll_id)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_growth"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "growth_replay_projection_mismatch",
        ));
    }
    Ok(())
}

async fn apply_p08_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
        .map_err(|_| CoreDomainRepositoryError::Integrity("p08_replay_payload"))?;
    event.validate_schema_version()?;
    match &event {
        CoreDomainEvent::CombatStateRecorded { .. } => {
            apply_combat_replay_event(transaction, replay, &event).await
        }
        CoreDomainEvent::ChaseStateRecorded { .. } => {
            apply_chase_replay_event(transaction, replay, &event).await
        }
        CoreDomainEvent::ReconsiderationRequested { .. }
        | CoreDomainEvent::ReconsiderationReviewed { .. }
        | CoreDomainEvent::ReconsiderationUpheld { .. }
        | CoreDomainEvent::ReconsiderationCorrected { .. } => {
            apply_reconsideration_replay_event(transaction, replay, &event).await
        }
        CoreDomainEvent::CampaignForkRecorded { .. }
        | CoreDomainEvent::CampaignForkMaterializationRecorded { .. }
        | CoreDomainEvent::CampaignForkMaterialized { .. } => {
            apply_campaign_fork_replay_event(transaction, replay).await
        }
        CoreDomainEvent::EndingRecorded { .. } => {
            apply_ending_replay_event(transaction, replay, &event).await
        }
        CoreDomainEvent::CharacterGrowthApplied {
            growth_event_id,
            campaign_id,
            session_id,
            ending_event_id,
            character_id,
            source_sheet_version_id,
            new_sheet_version_id,
            skill_name,
            skill_before,
            improvement_check_roll,
            increase_roll,
            skill_after,
            server_roll_id,
            increase_roll_id,
            ..
        } => {
            apply_growth_replay_event(
                transaction,
                replay,
                growth_event_id,
                campaign_id,
                session_id,
                ending_event_id,
                character_id,
                source_sheet_version_id,
                new_sheet_version_id,
                skill_name,
                *skill_before,
                *improvement_check_roll,
                *increase_roll,
                *skill_after,
                server_roll_id,
                increase_roll_id.as_deref(),
            )
            .await
        }
        _ => Err(CoreDomainRepositoryError::Integrity(
            "p08_replay_event_type",
        )),
    }
}

async fn apply_campaign_fork_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
        .map_err(|_| CoreDomainRepositoryError::Integrity("campaign_fork_replay_payload"))?;
    event.validate_schema_version()?;
    match event {
        CoreDomainEvent::CampaignForkRecorded {
            fork_id,
            parent_campaign_id,
            child_campaign_id,
            source_session_id,
            snapshot_hash,
            child_snapshot_hash,
            copy_scopes,
            canonical_snapshot_json,
            reason,
            ..
        } => {
            if replay.campaign_id != child_campaign_id {
                return Err(CoreDomainRepositoryError::Integrity(
                    "campaign_fork_replay_campaign_mismatch",
                ));
            }
            sqlx::query(
                r#"
                INSERT INTO public.campaign_forks (
                    fork_id, campaign_id, parent_campaign_id, child_campaign_id,
                    source_session_id, source_snapshot_hash, reason, version,
                    child_snapshot_hash, copy_scope_json, snapshot_json,
                    materialization_version,
                    visibility_label, visibility_subject,
                    provenance_kind, provenance_reference, provenance_recorded_by,
                    last_event_sequence
                ) VALUES (
                    $1, $2, $3, $2, $4, $5, $6, 1,
                    $7, $8, $9::JSONB, 2,
                    $10, $11, $12, $13, $14, $15
                )
                ON CONFLICT (fork_id) DO NOTHING
                "#,
            )
            .bind(&fork_id)
            .bind(&child_campaign_id)
            .bind(&parent_campaign_id)
            .bind(&source_session_id)
            .bind(&snapshot_hash)
            .bind(reason.trim())
            .bind(&child_snapshot_hash)
            .bind(sqlx::types::Json(&copy_scopes))
            .bind(&canonical_snapshot_json)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_campaign_fork"))?;
            let matches: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM public.campaign_forks
                     WHERE fork_id = $1
                       AND campaign_id = $2
                       AND parent_campaign_id = $3
                       AND child_campaign_id = $2
                       AND source_session_id = $4
                       AND source_snapshot_hash = $5
                       AND child_snapshot_hash = $6
                       AND copy_scope_json = $7
                       AND snapshot_json = $8::JSONB
                       AND materialization_version = 2
                       AND last_event_sequence = $9
                )
                "#,
            )
            .bind(&fork_id)
            .bind(&child_campaign_id)
            .bind(&parent_campaign_id)
            .bind(&source_session_id)
            .bind(&snapshot_hash)
            .bind(&child_snapshot_hash)
            .bind(sqlx::types::Json(&copy_scopes))
            .bind(&canonical_snapshot_json)
            .bind(replay.sequence)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_campaign_fork"))?;
            if !matches {
                return Err(CoreDomainRepositoryError::Integrity(
                    "campaign_fork_replay_identity_conflict",
                ));
            }
        }
        CoreDomainEvent::CampaignForkMaterializationRecorded {
            fork_id,
            child_campaign_id,
            child_session_id,
            child_scenario_id,
            child_snapshot_hash,
            child_state_json,
            materialized_row_count,
            batch_count,
            ..
        } => {
            if replay.campaign_id != child_campaign_id {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_manifest_replay_campaign_mismatch",
                ));
            }
            let row_count = i64::try_from(materialized_row_count)
                .map_err(|_| CoreDomainRepositoryError::Integrity("fork_row_count"))?;
            let batch_count = i64::try_from(batch_count)
                .map_err(|_| CoreDomainRepositoryError::Integrity("fork_batch_count"))?;
            sqlx::query(
                r#"
                INSERT INTO public.campaign_fork_materializations (
                    fork_id, campaign_id, parent_campaign_id, source_session_id,
                    child_session_id, child_scenario_id,
                    source_snapshot_hash, child_snapshot_hash, child_state_json,
                    materialized_row_count, batch_count, version,
                    visibility_label, visibility_subject,
                    provenance_kind, provenance_reference, provenance_recorded_by,
                    last_event_sequence
                )
                SELECT fork.fork_id, fork.child_campaign_id,
                       fork.parent_campaign_id, fork.source_session_id,
                       $2, $3, fork.source_snapshot_hash, $4, $5,
                       $6, $7, 1, $8, $9, $10, $11, $12, $13
                  FROM public.campaign_forks AS fork
                 WHERE fork.fork_id = $1
                   AND fork.child_campaign_id = $14
                ON CONFLICT (fork_id) DO NOTHING
                "#,
            )
            .bind(&fork_id)
            .bind(&child_session_id)
            .bind(&child_scenario_id)
            .bind(&child_snapshot_hash)
            .bind(&child_state_json)
            .bind(row_count)
            .bind(batch_count)
            .bind(&replay.visibility_label)
            .bind(&replay.visibility_subject)
            .bind(&replay.provenance_kind)
            .bind(&replay.provenance_reference)
            .bind(&replay.provenance_recorded_by)
            .bind(replay.sequence)
            .bind(&child_campaign_id)
            .execute(&mut **transaction)
            .await
            .map_err(database_error("replay_campaign_fork_manifest"))?;
            let matches: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM public.campaign_fork_materializations
                     WHERE fork_id = $1
                       AND campaign_id = $2
                       AND child_session_id = $3
                       AND child_scenario_id = $4
                       AND child_snapshot_hash = $5
                       AND child_state_json = $6
                       AND materialized_row_count = $7
                       AND batch_count = $8
                       AND last_event_sequence = $9
                )
                "#,
            )
            .bind(&fork_id)
            .bind(&child_campaign_id)
            .bind(&child_session_id)
            .bind(&child_scenario_id)
            .bind(&child_snapshot_hash)
            .bind(&child_state_json)
            .bind(row_count)
            .bind(batch_count)
            .bind(replay.sequence)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_campaign_fork_manifest"))?;
            if !matches {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_manifest_replay_identity_conflict",
                ));
            }
        }
        CoreDomainEvent::CampaignForkMaterialized {
            fork_id,
            child_campaign_id,
            batch_index,
            batch_count,
            rows,
            ..
        } => {
            if replay.campaign_id != child_campaign_id
                || batch_index == 0
                || batch_index > batch_count
                || rows.is_empty()
                || rows
                    .iter()
                    .map(CampaignForkMaterializedRow::projection_target_count)
                    .sum::<usize>()
                    > 32
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_batch_replay_shape",
                ));
            }
            let manifest_matches: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM public.campaign_fork_materializations
                     WHERE fork_id = $1
                       AND campaign_id = $2
                       AND batch_count = $3
                )
                "#,
            )
            .bind(&fork_id)
            .bind(&child_campaign_id)
            .bind(
                i64::try_from(batch_count)
                    .map_err(|_| CoreDomainRepositoryError::Integrity("fork_batch_count"))?,
            )
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("load_fork_manifest_for_batch"))?;
            if !manifest_matches {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_batch_manifest_mismatch",
                ));
            }
            let expected_data_subject_id = rows.first().map(fork_row_data_subject).ok_or(
                CoreDomainRepositoryError::Integrity("fork_batch_replay_shape"),
            )?;
            let event_data_subject_id: String = sqlx::query_scalar(
                "SELECT data_subject_id FROM public.event_store WHERE sequence = $1",
            )
            .bind(replay.sequence)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("load_fork_event_data_subject"))?;
            if event_data_subject_id != expected_data_subject_id {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_event_data_subject_mismatch",
                ));
            }
            for row in rows {
                let (row_visibility_label, row_visibility_subject) = fork_row_visibility(&row);
                if row_visibility_label != replay.visibility_label
                    || row_visibility_subject != replay.visibility_subject
                {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_row_visibility_mismatch",
                    ));
                }
                match row {
                    CampaignForkMaterializedRow::Scenario {
                        scenario_id,
                        ruleset_id,
                        format_version,
                        content_hash,
                        document_json,
                        visibility_label,
                        visibility_subject,
                    } => {
                        sqlx::query(
                            r#"
                            INSERT INTO public.scenarios (
                                scenario_id, campaign_id, ruleset_id, format_version,
                                content_hash, document_json, validated, version,
                                visibility_label, visibility_subject,
                                provenance_kind, provenance_reference,
                                provenance_recorded_by, last_event_sequence
                            ) VALUES (
                                $1, $2, $3, $4, $5, $6::JSONB, TRUE, 1,
                                $7, $8, $9, $10, $11, $12
                            )
                            ON CONFLICT (scenario_id) DO NOTHING
                            "#,
                        )
                        .bind(&scenario_id)
                        .bind(&child_campaign_id)
                        .bind(&ruleset_id)
                        .bind(&format_version)
                        .bind(&content_hash)
                        .bind(&document_json)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(&replay.provenance_kind)
                        .bind(&replay.provenance_reference)
                        .bind(&replay.provenance_recorded_by)
                        .bind(replay.sequence)
                        .execute(&mut **transaction)
                        .await
                        .map_err(database_error("replay_fork_scenario"))?;
                        let matches: bool = sqlx::query_scalar(
                            r#"
                            SELECT EXISTS(
                                SELECT 1 FROM public.scenarios
                                 WHERE scenario_id = $1 AND campaign_id = $2
                                   AND content_hash = $3
                                   AND document_json = $4::JSONB
                                   AND visibility_label::TEXT = $5
                                   AND visibility_subject = $6
                                   AND last_event_sequence = $7
                            )
                            "#,
                        )
                        .bind(&scenario_id)
                        .bind(&child_campaign_id)
                        .bind(&content_hash)
                        .bind(&document_json)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(replay.sequence)
                        .fetch_one(&mut **transaction)
                        .await
                        .map_err(database_error("verify_replayed_fork_scenario"))?;
                        if !matches {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_scenario_identity_conflict",
                            ));
                        }
                    }
                    CampaignForkMaterializedRow::Character {
                        character_id,
                        owner_user_id,
                        display_name,
                        state,
                        initial_version_locked,
                        sheet_version_id,
                        sheet_json,
                        sheet_locked,
                        visibility_label,
                        visibility_subject,
                    } => {
                        if !matches!(state.as_str(), "DRAFT" | "SUBMITTED" | "APPROVED") {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_character_state",
                            ));
                        }
                        sqlx::query(
                            r#"
                            INSERT INTO public.characters (
                                character_id, campaign_id, owner_user_id, display_name,
                                state, current_sheet_version, initial_version_locked,
                                version, visibility_label, visibility_subject,
                                provenance_kind, provenance_reference,
                                provenance_recorded_by, last_event_sequence
                            ) VALUES (
                                $1, $2, $3, $4, $5, 1, $6, 1, $7, $8,
                                $9, $10, $11, $12
                            )
                            ON CONFLICT (character_id) DO NOTHING
                            "#,
                        )
                        .bind(&character_id)
                        .bind(&child_campaign_id)
                        .bind(&owner_user_id)
                        .bind(&display_name)
                        .bind(&state)
                        .bind(initial_version_locked)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(&replay.provenance_kind)
                        .bind(&replay.provenance_reference)
                        .bind(&replay.provenance_recorded_by)
                        .bind(replay.sequence)
                        .execute(&mut **transaction)
                        .await
                        .map_err(database_error("replay_fork_character"))?;
                        sqlx::query(
                            r#"
                            INSERT INTO public.character_sheet_versions (
                                sheet_version_id, character_id, version, sheet_json,
                                locked, visibility_label, visibility_subject,
                                provenance_kind, provenance_reference,
                                provenance_recorded_by, campaign_id, last_event_sequence
                            ) VALUES (
                                $1, $2, 1, $3::JSONB, $4, $5, $6, $7, $8, $9, $10, $11
                            )
                            ON CONFLICT (sheet_version_id) DO NOTHING
                            "#,
                        )
                        .bind(&sheet_version_id)
                        .bind(&character_id)
                        .bind(&sheet_json)
                        .bind(sheet_locked)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(&replay.provenance_kind)
                        .bind(&replay.provenance_reference)
                        .bind(&replay.provenance_recorded_by)
                        .bind(&child_campaign_id)
                        .bind(replay.sequence)
                        .execute(&mut **transaction)
                        .await
                        .map_err(database_error("replay_fork_character_sheet"))?;
                        let matches: bool = sqlx::query_scalar(
                            r#"
                            SELECT EXISTS(
                                SELECT 1
                                  FROM public.characters AS character
                                  JOIN public.character_sheet_versions AS sheet
                                    ON sheet.character_id = character.character_id
                                   AND sheet.version = 1
                                 WHERE character.character_id = $1
                                   AND character.campaign_id = $2
                                   AND character.owner_user_id = $3
                                   AND character.display_name = $4
                                   AND character.state = $5
                                   AND character.initial_version_locked = $6
                                   AND character.last_event_sequence = $7
                                   AND sheet.sheet_version_id = $8
                                   AND sheet.sheet_json = $9::JSONB
                                   AND sheet.locked = $10
                                   AND character.visibility_label::TEXT = $11
                                   AND character.visibility_subject = $12
                                   AND sheet.visibility_label::TEXT = $11
                                   AND sheet.visibility_subject = $12
                                   AND sheet.last_event_sequence = $7
                            )
                            "#,
                        )
                        .bind(&character_id)
                        .bind(&child_campaign_id)
                        .bind(&owner_user_id)
                        .bind(&display_name)
                        .bind(&state)
                        .bind(initial_version_locked)
                        .bind(replay.sequence)
                        .bind(&sheet_version_id)
                        .bind(&sheet_json)
                        .bind(sheet_locked)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .fetch_one(&mut **transaction)
                        .await
                        .map_err(database_error("verify_replayed_fork_character"))?;
                        if !matches {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_character_identity_conflict",
                            ));
                        }
                    }
                    CampaignForkMaterializedRow::Session {
                        session_id,
                        room_id,
                        scenario_id,
                        state,
                        active_scene_id,
                        started_at_unix_ms,
                        ended_at_unix_ms,
                        visibility_label,
                        visibility_subject,
                    } => {
                        if state != "ENDED" || ended_at_unix_ms < started_at_unix_ms {
                            return Err(CoreDomainRepositoryError::Integrity("fork_session_state"));
                        }
                        let started_at =
                            timestamp_from_unix_ms(started_at_unix_ms, "fork_session.started_at")?;
                        let ended_at =
                            timestamp_from_unix_ms(ended_at_unix_ms, "fork_session.ended_at")?;
                        sqlx::query(
                            r#"
                            INSERT INTO core_domain.sessions (
                                session_id, campaign_id, room_id, scenario_id, state,
                                active_scene_id, started_at, ended_at, version,
                                visibility_label, visibility_subject,
                                provenance_kind, provenance_reference,
                                provenance_recorded_by, last_event_sequence
                            ) VALUES (
                                $1, $2, $3, $4, 'ENDED', $5, $6, $7, 1,
                                $8, $9, $10, $11, $12, $13
                            )
                            ON CONFLICT (session_id) DO NOTHING
                            "#,
                        )
                        .bind(&session_id)
                        .bind(&child_campaign_id)
                        .bind(&room_id)
                        .bind(&scenario_id)
                        .bind(&active_scene_id)
                        .bind(started_at)
                        .bind(ended_at)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(&replay.provenance_kind)
                        .bind(&replay.provenance_reference)
                        .bind(&replay.provenance_recorded_by)
                        .bind(replay.sequence)
                        .execute(&mut **transaction)
                        .await
                        .map_err(database_error("replay_fork_session"))?;
                        let matches: bool = sqlx::query_scalar(
                            r#"
                            SELECT EXISTS(
                                SELECT 1 FROM core_domain.sessions
                                 WHERE session_id = $1 AND campaign_id = $2
                                   AND room_id = $3 AND scenario_id = $4
                                   AND state = 'ENDED'
                                   AND active_scene_id IS NOT DISTINCT FROM $5
                                   AND started_at = $6 AND ended_at = $7
                                   AND visibility_label::TEXT = $8
                                   AND visibility_subject = $9
                                   AND last_event_sequence = $10
                            )
                            "#,
                        )
                        .bind(&session_id)
                        .bind(&child_campaign_id)
                        .bind(&room_id)
                        .bind(&scenario_id)
                        .bind(&active_scene_id)
                        .bind(started_at)
                        .bind(ended_at)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(replay.sequence)
                        .fetch_one(&mut **transaction)
                        .await
                        .map_err(database_error("verify_replayed_fork_session"))?;
                        if !matches {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_session_identity_conflict",
                            ));
                        }
                    }
                    CampaignForkMaterializedRow::Scene {
                        scene_id,
                        session_id,
                        scenario_id,
                        room_id,
                        scene_key,
                        name,
                        state,
                        visibility_label,
                        visibility_subject,
                    } => {
                        if !matches!(state.as_str(), "READY" | "ACTIVE" | "CLOSED") {
                            return Err(CoreDomainRepositoryError::Integrity("fork_scene_state"));
                        }
                        sqlx::query(
                            r#"
                            INSERT INTO public.scenes (
                                scene_id, campaign_id, session_id, scenario_id, room_id,
                                scene_key, name, state, version,
                                visibility_label, visibility_subject,
                                provenance_kind, provenance_reference,
                                provenance_recorded_by, last_event_sequence
                            ) VALUES (
                                $1, $2, $3, $4, $5, $6, $7, $8, 1,
                                $9, $10, $11, $12, $13, $14
                            )
                            ON CONFLICT (scene_id) DO NOTHING
                            "#,
                        )
                        .bind(&scene_id)
                        .bind(&child_campaign_id)
                        .bind(&session_id)
                        .bind(&scenario_id)
                        .bind(&room_id)
                        .bind(&scene_key)
                        .bind(&name)
                        .bind(&state)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(&replay.provenance_kind)
                        .bind(&replay.provenance_reference)
                        .bind(&replay.provenance_recorded_by)
                        .bind(replay.sequence)
                        .execute(&mut **transaction)
                        .await
                        .map_err(database_error("replay_fork_scene"))?;
                        let matches: bool = sqlx::query_scalar(
                            r#"
                            SELECT EXISTS(
                                SELECT 1 FROM public.scenes
                                 WHERE scene_id = $1 AND campaign_id = $2
                                   AND session_id = $3 AND scenario_id = $4
                                   AND room_id = $5 AND scene_key = $6
                                   AND name = $7 AND state = $8
                                   AND visibility_label::TEXT = $9
                                   AND visibility_subject = $10
                                   AND last_event_sequence = $11
                            )
                            "#,
                        )
                        .bind(&scene_id)
                        .bind(&child_campaign_id)
                        .bind(&session_id)
                        .bind(&scenario_id)
                        .bind(&room_id)
                        .bind(&scene_key)
                        .bind(&name)
                        .bind(&state)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(replay.sequence)
                        .fetch_one(&mut **transaction)
                        .await
                        .map_err(database_error("verify_replayed_fork_scene"))?;
                        if !matches {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_scene_identity_conflict",
                            ));
                        }
                    }
                    CampaignForkMaterializedRow::PublicEvent {
                        fork_event_id,
                        source_event_sequence,
                        source_event_type,
                        source_resource_type,
                        source_resource_id,
                        source_payload_json,
                        source_event_integrity_hash,
                        visibility_label,
                        visibility_subject,
                    } => {
                        let source_event_sequence =
                            i64::try_from(source_event_sequence).map_err(|_| {
                                CoreDomainRepositoryError::Integrity("fork_public_event_sequence")
                            })?;
                        let source_payload: Value = serde_json::from_str(&source_payload_json)
                            .map_err(|_| {
                                CoreDomainRepositoryError::Integrity("fork_public_event_payload")
                            })?;
                        if !source_payload.is_object()
                            || !matches!(visibility_label.as_str(), "public" | "party_visible")
                            || visibility_subject != "not_applicable"
                            || !source_event_integrity_hash.starts_with("hmac-sha256:")
                        {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_public_event_shape",
                            ));
                        }
                        let source_matches: bool = sqlx::query_scalar(
                            r#"
                            SELECT EXISTS(
                                SELECT 1
                                  FROM public.campaign_forks AS fork
                                  JOIN public.event_store AS source_event
                                    ON source_event.campaign_id =
                                       fork.parent_campaign_id
                                   AND source_event.sequence = $2
                                 WHERE fork.fork_id = $1
                                   AND fork.child_campaign_id = $3
                                   AND source_event.event_type = $4
                                   AND source_event.resource_type = $5
                                   AND source_event.resource_id = $6
                                   AND source_event.event_integrity_hash = $7
                                   AND source_event.visibility_label = $8
                                   AND source_event.visibility_subject = $9
                                   AND source_event.integrity_status = 'verified_hmac'
                                   AND source_event.request_hash_source = 'formal_commit'
                            )
                            "#,
                        )
                        .bind(&fork_id)
                        .bind(source_event_sequence)
                        .bind(&child_campaign_id)
                        .bind(&source_event_type)
                        .bind(&source_resource_type)
                        .bind(&source_resource_id)
                        .bind(&source_event_integrity_hash)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .fetch_one(&mut **transaction)
                        .await
                        .map_err(database_error("verify_fork_public_event_source"))?;
                        if !source_matches {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_public_event_source_mismatch",
                            ));
                        }
                        sqlx::query(
                            r#"
                            INSERT INTO public.campaign_fork_public_events (
                                fork_event_id, fork_id, campaign_id,
                                source_event_sequence, source_event_type,
                                source_resource_type, source_resource_id,
                                source_payload_json, source_event_integrity_hash,
                                version, visibility_label, visibility_subject,
                                provenance_kind, provenance_reference,
                                provenance_recorded_by, last_event_sequence
                            ) VALUES (
                                $1, $2, $3, $4, $5, $6, $7, $8::JSONB, $9,
                                1, $10, $11, $12, $13, $14, $15
                            )
                            ON CONFLICT (fork_event_id) DO NOTHING
                            "#,
                        )
                        .bind(&fork_event_id)
                        .bind(&fork_id)
                        .bind(&child_campaign_id)
                        .bind(source_event_sequence)
                        .bind(&source_event_type)
                        .bind(&source_resource_type)
                        .bind(&source_resource_id)
                        .bind(&source_payload_json)
                        .bind(&source_event_integrity_hash)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(&replay.provenance_kind)
                        .bind(&replay.provenance_reference)
                        .bind(&replay.provenance_recorded_by)
                        .bind(replay.sequence)
                        .execute(&mut **transaction)
                        .await
                        .map_err(database_error("replay_fork_public_event"))?;
                        let matches: bool = sqlx::query_scalar(
                            r#"
                            SELECT EXISTS(
                                SELECT 1
                                  FROM public.campaign_fork_public_events
                                 WHERE fork_event_id = $1
                                   AND fork_id = $2
                                   AND campaign_id = $3
                                   AND source_event_sequence = $4
                                   AND source_event_type = $5
                                   AND source_resource_type = $6
                                   AND source_resource_id = $7
                                   AND source_payload_json = $8::JSONB
                                   AND source_event_integrity_hash = $9
                                   AND visibility_label::TEXT = $10
                                   AND visibility_subject = $11
                                   AND last_event_sequence = $12
                            )
                            "#,
                        )
                        .bind(&fork_event_id)
                        .bind(&fork_id)
                        .bind(&child_campaign_id)
                        .bind(source_event_sequence)
                        .bind(&source_event_type)
                        .bind(&source_resource_type)
                        .bind(&source_resource_id)
                        .bind(&source_payload_json)
                        .bind(&source_event_integrity_hash)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(replay.sequence)
                        .fetch_one(&mut **transaction)
                        .await
                        .map_err(database_error("verify_replayed_fork_public_event"))?;
                        if !matches {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_public_event_identity_conflict",
                            ));
                        }
                    }
                    CampaignForkMaterializedRow::DiscoveredClue {
                        fork_clue_id,
                        source_clue_id,
                        importance,
                        outcome,
                        cost,
                        visibility_label,
                        visibility_subject,
                    } => {
                        if !matches!(importance.as_str(), "CORE" | "OPTIONAL")
                            || !matches!(outcome.as_str(), "REVEALED" | "REVEALED_WITH_COST")
                            || !matches!(visibility_label.as_str(), "public" | "party_visible")
                            || visibility_subject != "not_applicable"
                        {
                            return Err(CoreDomainRepositoryError::Integrity("fork_clue_shape"));
                        }
                        sqlx::query(
                            r#"
                            INSERT INTO public.campaign_fork_clues (
                                fork_clue_id, fork_id, campaign_id, source_clue_id,
                                importance, outcome, cost, version,
                                visibility_label, visibility_subject,
                                provenance_kind, provenance_reference,
                                provenance_recorded_by, last_event_sequence
                            ) VALUES (
                                $1, $2, $3, $4, $5, $6, $7, 1,
                                $8, $9, $10, $11, $12, $13
                            )
                            ON CONFLICT (fork_clue_id) DO NOTHING
                            "#,
                        )
                        .bind(&fork_clue_id)
                        .bind(&fork_id)
                        .bind(&child_campaign_id)
                        .bind(&source_clue_id)
                        .bind(&importance)
                        .bind(&outcome)
                        .bind(&cost)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(&replay.provenance_kind)
                        .bind(&replay.provenance_reference)
                        .bind(&replay.provenance_recorded_by)
                        .bind(replay.sequence)
                        .execute(&mut **transaction)
                        .await
                        .map_err(database_error("replay_fork_clue"))?;
                        let matches: bool = sqlx::query_scalar(
                            r#"
                            SELECT EXISTS(
                                SELECT 1 FROM public.campaign_fork_clues
                                 WHERE fork_clue_id = $1
                                   AND fork_id = $2
                                   AND campaign_id = $3
                                   AND source_clue_id = $4
                                   AND importance = $5
                                   AND outcome = $6
                                   AND cost IS NOT DISTINCT FROM $7
                                   AND visibility_label::TEXT = $8
                                   AND visibility_subject = $9
                                   AND last_event_sequence = $10
                            )
                            "#,
                        )
                        .bind(&fork_clue_id)
                        .bind(&fork_id)
                        .bind(&child_campaign_id)
                        .bind(&source_clue_id)
                        .bind(&importance)
                        .bind(&outcome)
                        .bind(&cost)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(replay.sequence)
                        .fetch_one(&mut **transaction)
                        .await
                        .map_err(database_error("verify_replayed_fork_clue"))?;
                        if !matches {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_clue_identity_conflict",
                            ));
                        }
                    }
                    CampaignForkMaterializedRow::NpcState {
                        npc_state_id,
                        source_npc_id,
                        state_json,
                        visibility_label,
                        visibility_subject,
                    } => {
                        let state: Value = serde_json::from_str(&state_json).map_err(|_| {
                            CoreDomainRepositoryError::Integrity("fork_npc_state_json")
                        })?;
                        if !state.is_object()
                            || !matches!(visibility_label.as_str(), "public" | "party_visible")
                            || visibility_subject != "not_applicable"
                        {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_npc_state_shape",
                            ));
                        }
                        sqlx::query(
                            r#"
                            INSERT INTO public.campaign_fork_npc_states (
                                npc_state_id, fork_id, campaign_id, source_npc_id,
                                state_json, version,
                                visibility_label, visibility_subject,
                                provenance_kind, provenance_reference,
                                provenance_recorded_by, last_event_sequence
                            ) VALUES (
                                $1, $2, $3, $4, $5::JSONB, 1,
                                $6, $7, $8, $9, $10, $11
                            )
                            ON CONFLICT (npc_state_id) DO NOTHING
                            "#,
                        )
                        .bind(&npc_state_id)
                        .bind(&fork_id)
                        .bind(&child_campaign_id)
                        .bind(&source_npc_id)
                        .bind(&state_json)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(&replay.provenance_kind)
                        .bind(&replay.provenance_reference)
                        .bind(&replay.provenance_recorded_by)
                        .bind(replay.sequence)
                        .execute(&mut **transaction)
                        .await
                        .map_err(database_error("replay_fork_npc_state"))?;
                        let matches: bool = sqlx::query_scalar(
                            r#"
                            SELECT EXISTS(
                                SELECT 1 FROM public.campaign_fork_npc_states
                                 WHERE npc_state_id = $1
                                   AND fork_id = $2
                                   AND campaign_id = $3
                                   AND source_npc_id = $4
                                   AND state_json = $5::JSONB
                                   AND visibility_label::TEXT = $6
                                   AND visibility_subject = $7
                                   AND last_event_sequence = $8
                            )
                            "#,
                        )
                        .bind(&npc_state_id)
                        .bind(&fork_id)
                        .bind(&child_campaign_id)
                        .bind(&source_npc_id)
                        .bind(&state_json)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(replay.sequence)
                        .fetch_one(&mut **transaction)
                        .await
                        .map_err(database_error("verify_replayed_fork_npc_state"))?;
                        if !matches {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_npc_state_identity_conflict",
                            ));
                        }
                    }
                    CampaignForkMaterializedRow::Combat {
                        combat_id,
                        session_id,
                        status,
                        round,
                        current_turn_index,
                        state_json,
                        visibility_label,
                        visibility_subject,
                    } => {
                        let inspected = inspect_combat_state(&state_json).map_err(|_| {
                            CoreDomainRepositoryError::Integrity("fork_combat_state_json")
                        })?;
                        if inspected.combat_id() != combat_id
                            || inspected.status() != status
                            || u64::from(inspected.round()) != round
                            || u64::try_from(inspected.current_turn_index()).ok()
                                != Some(current_turn_index)
                            || inspected.version() != 1
                        {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_combat_state_shape",
                            ));
                        }
                        let round = i64::try_from(round).map_err(|_| {
                            CoreDomainRepositoryError::Integrity("fork_combat_round")
                        })?;
                        let current_turn_index =
                            i64::try_from(current_turn_index).map_err(|_| {
                                CoreDomainRepositoryError::Integrity("fork_combat_turn")
                            })?;
                        sqlx::query(
                            r#"
                            INSERT INTO public.combat_states (
                                combat_id, campaign_id, session_id, status,
                                round, current_turn_index, state_json, version,
                                visibility_label, visibility_subject,
                                provenance_kind, provenance_reference,
                                provenance_recorded_by, last_event_sequence
                            ) VALUES (
                                $1, $2, $3, $4, $5, $6, $7::JSONB, 1,
                                $8, $9, $10, $11, $12, $13
                            )
                            ON CONFLICT (combat_id) DO NOTHING
                            "#,
                        )
                        .bind(&combat_id)
                        .bind(&child_campaign_id)
                        .bind(&session_id)
                        .bind(&status)
                        .bind(round)
                        .bind(current_turn_index)
                        .bind(&state_json)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(&replay.provenance_kind)
                        .bind(&replay.provenance_reference)
                        .bind(&replay.provenance_recorded_by)
                        .bind(replay.sequence)
                        .execute(&mut **transaction)
                        .await
                        .map_err(database_error("replay_fork_combat"))?;
                        let matches: bool = sqlx::query_scalar(
                            r#"
                            SELECT EXISTS(
                                SELECT 1 FROM public.combat_states
                                 WHERE combat_id = $1
                                   AND campaign_id = $2
                                   AND session_id = $3
                                   AND status = $4
                                   AND round = $5
                                   AND current_turn_index = $6
                                   AND state_json = $7::JSONB
                                   AND version = 1
                                   AND visibility_label::TEXT = $8
                                   AND visibility_subject = $9
                                   AND last_event_sequence = $10
                            )
                            "#,
                        )
                        .bind(&combat_id)
                        .bind(&child_campaign_id)
                        .bind(&session_id)
                        .bind(&status)
                        .bind(round)
                        .bind(current_turn_index)
                        .bind(&state_json)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(replay.sequence)
                        .fetch_one(&mut **transaction)
                        .await
                        .map_err(database_error("verify_replayed_fork_combat"))?;
                        if !matches {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_combat_identity_conflict",
                            ));
                        }
                    }
                    CampaignForkMaterializedRow::Chase {
                        chase_id,
                        session_id,
                        status,
                        range_band,
                        segment,
                        state_json,
                        visibility_label,
                        visibility_subject,
                    } => {
                        let inspected = inspect_chase_state(&state_json).map_err(|_| {
                            CoreDomainRepositoryError::Integrity("fork_chase_state_json")
                        })?;
                        if inspected.chase_id() != chase_id
                            || inspected.status() != status
                            || u8::try_from(inspected.range()).ok() != Some(range_band)
                            || u64::from(inspected.segment()) != segment
                            || inspected.version() != 1
                        {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_chase_state_shape",
                            ));
                        }
                        let range_band = i16::from(range_band);
                        let segment = i64::try_from(segment).map_err(|_| {
                            CoreDomainRepositoryError::Integrity("fork_chase_segment")
                        })?;
                        sqlx::query(
                            r#"
                            INSERT INTO public.chase_states (
                                chase_id, campaign_id, session_id, status,
                                range_band, segment, state_json, version,
                                visibility_label, visibility_subject,
                                provenance_kind, provenance_reference,
                                provenance_recorded_by, last_event_sequence
                            ) VALUES (
                                $1, $2, $3, $4, $5, $6, $7::JSONB, 1,
                                $8, $9, $10, $11, $12, $13
                            )
                            ON CONFLICT (chase_id) DO NOTHING
                            "#,
                        )
                        .bind(&chase_id)
                        .bind(&child_campaign_id)
                        .bind(&session_id)
                        .bind(&status)
                        .bind(range_band)
                        .bind(segment)
                        .bind(&state_json)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(&replay.provenance_kind)
                        .bind(&replay.provenance_reference)
                        .bind(&replay.provenance_recorded_by)
                        .bind(replay.sequence)
                        .execute(&mut **transaction)
                        .await
                        .map_err(database_error("replay_fork_chase"))?;
                        let matches: bool = sqlx::query_scalar(
                            r#"
                            SELECT EXISTS(
                                SELECT 1 FROM public.chase_states
                                 WHERE chase_id = $1
                                   AND campaign_id = $2
                                   AND session_id = $3
                                   AND status = $4
                                   AND range_band = $5
                                   AND segment = $6
                                   AND state_json = $7::JSONB
                                   AND version = 1
                                   AND visibility_label::TEXT = $8
                                   AND visibility_subject = $9
                                   AND last_event_sequence = $10
                            )
                            "#,
                        )
                        .bind(&chase_id)
                        .bind(&child_campaign_id)
                        .bind(&session_id)
                        .bind(&status)
                        .bind(range_band)
                        .bind(segment)
                        .bind(&state_json)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(replay.sequence)
                        .fetch_one(&mut **transaction)
                        .await
                        .map_err(database_error("verify_replayed_fork_chase"))?;
                        if !matches {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_chase_identity_conflict",
                            ));
                        }
                    }
                    CampaignForkMaterializedRow::Conclusion {
                        ending_event_id,
                        session_id,
                        ending_id,
                        summary,
                        ended_at_unix_ms,
                        visibility_label,
                        visibility_subject,
                    } => {
                        if ending_id.trim().is_empty()
                            || summary.trim().is_empty()
                            || summary.len() > 1_024
                        {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_conclusion_shape",
                            ));
                        }
                        let ended_at =
                            timestamp_from_unix_ms(ended_at_unix_ms, "fork_ending.ended_at")?;
                        sqlx::query(
                            r#"
                            INSERT INTO public.ending_events (
                                ending_event_id, campaign_id, session_id,
                                ending_id, summary, ended_at, version,
                                visibility_label, visibility_subject,
                                provenance_kind, provenance_reference,
                                provenance_recorded_by, last_event_sequence
                            ) VALUES (
                                $1, $2, $3, $4, $5, $6, 1,
                                $7, $8, $9, $10, $11, $12
                            )
                            ON CONFLICT (ending_event_id) DO NOTHING
                            "#,
                        )
                        .bind(&ending_event_id)
                        .bind(&child_campaign_id)
                        .bind(&session_id)
                        .bind(&ending_id)
                        .bind(summary.trim())
                        .bind(ended_at)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(&replay.provenance_kind)
                        .bind(&replay.provenance_reference)
                        .bind(&replay.provenance_recorded_by)
                        .bind(replay.sequence)
                        .execute(&mut **transaction)
                        .await
                        .map_err(database_error("replay_fork_conclusion"))?;
                        let matches: bool = sqlx::query_scalar(
                            r#"
                            SELECT EXISTS(
                                SELECT 1 FROM public.ending_events
                                 WHERE ending_event_id = $1
                                   AND campaign_id = $2
                                   AND session_id = $3
                                   AND ending_id = $4
                                   AND summary = $5
                                   AND ended_at = $6
                                   AND visibility_label::TEXT = $7
                                   AND visibility_subject = $8
                                   AND last_event_sequence = $9
                            )
                            "#,
                        )
                        .bind(&ending_event_id)
                        .bind(&child_campaign_id)
                        .bind(&session_id)
                        .bind(&ending_id)
                        .bind(summary.trim())
                        .bind(ended_at)
                        .bind(&visibility_label)
                        .bind(&visibility_subject)
                        .bind(replay.sequence)
                        .fetch_one(&mut **transaction)
                        .await
                        .map_err(database_error("verify_replayed_fork_conclusion"))?;
                        if !matches {
                            return Err(CoreDomainRepositoryError::Integrity(
                                "fork_conclusion_identity_conflict",
                            ));
                        }
                    }
                }
                if batch_index == batch_count {
                    let expected_rows: i64 = sqlx::query_scalar(
                        "SELECT materialized_row_count \
                           FROM public.campaign_fork_materializations \
                          WHERE fork_id = $1 AND campaign_id = $2",
                    )
                    .bind(&fork_id)
                    .bind(&child_campaign_id)
                    .fetch_one(&mut **transaction)
                    .await
                    .map_err(database_error("load_fork_expected_row_count"))?;
                    let actual_rows: i64 = sqlx::query_scalar(
                        r#"
                        SELECT
                            (SELECT count(*) FROM public.scenarios
                              WHERE campaign_id = $1)
                          + (SELECT count(*) FROM public.characters
                              WHERE campaign_id = $1)
                          + (SELECT count(*) FROM core_domain.sessions
                              WHERE campaign_id = $1)
                          + (SELECT count(*) FROM public.scenes
                              WHERE campaign_id = $1)
                          + (SELECT count(*) FROM public.campaign_fork_public_events
                              WHERE campaign_id = $1 AND fork_id = $2)
                          + (SELECT count(*) FROM public.campaign_fork_clues
                              WHERE campaign_id = $1 AND fork_id = $2)
                          + (SELECT count(*) FROM public.campaign_fork_npc_states
                              WHERE campaign_id = $1 AND fork_id = $2)
                          + (SELECT count(*) FROM public.combat_states
                              WHERE campaign_id = $1)
                          + (SELECT count(*) FROM public.chase_states
                              WHERE campaign_id = $1)
                          + (SELECT count(*) FROM public.ending_events
                              WHERE campaign_id = $1)
                        "#,
                    )
                    .bind(&child_campaign_id)
                    .bind(&fork_id)
                    .fetch_one(&mut **transaction)
                    .await
                    .map_err(database_error("count_fork_materialized_rows"))?;
                    if actual_rows != expected_rows {
                        return Err(CoreDomainRepositoryError::Integrity(
                            "fork_materialized_row_count_mismatch",
                        ));
                    }
                }
            }
        }
        _ => {
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_fork_replay_event_type",
            ))
        }
    }
    Ok(())
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

    pub async fn rebuild_p08_projections(
        &self,
        campaign_id: &str,
    ) -> Result<P08ProjectionRebuildReport, CoreDomainRepositoryError> {
        EntityId::new(campaign_id)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("campaign_id"))?;
        let replay_events = self
            .load_campaign_events(campaign_id)
            .await?
            .into_iter()
            .filter(|event| {
                matches!(
                    event.event_type.as_str(),
                    "CombatStateRecorded"
                        | "ChaseStateRecorded"
                        | "ReconsiderationRequested"
                        | "ReconsiderationReviewed"
                        | "ReconsiderationUpheld"
                        | "ReconsiderationCorrected"
                        | "CampaignForkRecorded"
                        | "CampaignForkMaterializationRecorded"
                        | "CampaignForkMaterialized"
                        | "EndingRecorded"
                        | "CharacterGrowthApplied"
                )
            })
            .collect::<Vec<_>>();
        let last_event_sequence = replay_events
            .last()
            .map(|event| event.sequence)
            .unwrap_or(0);
        let mut transaction = self
            .primary
            .begin()
            .await
            .map_err(database_error("begin_p08_projection_rebuild"))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("p08-projection-rebuild:{campaign_id}"))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_p08_projection_rebuild"))?;
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *transaction)
            .await
            .map_err(database_error("defer_p08_rebuild_constraints"))?;
        for replay_event in &replay_events {
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
            .map_err(database_error("load_p08_rebuild_commit"))?;
            self.set_projection_capability(
                &mut transaction,
                &commit_id,
                "set_p08_rebuild_projection_capability",
            )
            .await?;
            apply_p08_replay_event(&mut transaction, replay_event).await?;
        }
        let counts: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                (SELECT count(*) FROM public.combat_states WHERE campaign_id = $1),
                (SELECT count(*) FROM public.chase_states WHERE campaign_id = $1),
                (SELECT count(*) FROM public.reconsiderations WHERE campaign_id = $1),
                (SELECT count(*) FROM public.campaign_forks WHERE campaign_id = $1),
                (SELECT count(*) FROM public.campaign_fork_materializations
                  WHERE campaign_id = $1),
                (SELECT count(*) FROM public.campaign_fork_public_events
                  WHERE campaign_id = $1),
                (SELECT count(*) FROM public.campaign_fork_clues
                  WHERE campaign_id = $1),
                (SELECT count(*) FROM public.campaign_fork_npc_states
                  WHERE campaign_id = $1),
                (SELECT count(*) FROM public.ending_events WHERE campaign_id = $1),
                (SELECT count(*) FROM public.growth_events WHERE campaign_id = $1)
            "#,
        )
        .bind(campaign_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error("count_p08_rebuilt_projections"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_p08_projection_rebuild"))?;
        Ok(P08ProjectionRebuildReport {
            campaign_id: campaign_id.to_owned(),
            replayed_events: replay_events.len(),
            combat_states: counts.0,
            chase_states: counts.1,
            reconsiderations: counts.2,
            campaign_forks: counts.3,
            fork_materializations: counts.4,
            fork_public_events: counts.5,
            fork_clues: counts.6,
            fork_npc_states: counts.7,
            ending_events: counts.8,
            growth_events: counts.9,
            last_event_sequence,
        })
    }

    pub async fn preview_campaign_fork(
        &self,
        parent_campaign_id: &str,
        source_session_id: &str,
        requesting_actor_id: &str,
    ) -> Result<CampaignForkSnapshotPreview, CoreDomainRepositoryError> {
        self.ensure_campaign_admin(parent_campaign_id, requesting_actor_id)
            .await?;
        self.load_public_campaign_fork_snapshot(parent_campaign_id, source_session_id)
            .await
    }

    async fn load_public_campaign_fork_snapshot(
        &self,
        parent_campaign_id: &str,
        source_session_id: &str,
    ) -> Result<CampaignForkSnapshotPreview, CoreDomainRepositoryError> {
        let mut state: Value = sqlx::query_scalar(
            r#"
            WITH snapshot_gameplay AS (
                SELECT source_session.*,
                       GREATEST(
                           source_session.last_event_sequence,
                           COALESCE((
                               SELECT max(combat.last_event_sequence)
                                 FROM public.combat_states AS combat
                                WHERE combat.session_id = source_session.session_id
                           ), 0),
                           COALESCE((
                               SELECT max(chase.last_event_sequence)
                                 FROM public.chase_states AS chase
                                WHERE chase.session_id = source_session.session_id
                           ), 0),
                           COALESCE((
                               SELECT max(ending.last_event_sequence)
                                 FROM public.ending_events AS ending
                                WHERE ending.session_id = source_session.session_id
                           ), 0),
                           COALESCE((
                               SELECT max(growth.last_event_sequence)
                                 FROM public.growth_events AS growth
                                WHERE growth.session_id = source_session.session_id
                           ), 0)
                       ) AS gameplay_cutoff_event_sequence
                 FROM core_domain.sessions AS source_session
                 WHERE source_session.session_id = $1
                   AND source_session.campaign_id = $2
                   AND source_session.state = 'ENDED'
            ),
            snapshot_source AS (
                SELECT snapshot_gameplay.*,
                       GREATEST(
                           snapshot_gameplay.gameplay_cutoff_event_sequence,
                           COALESCE((
                               SELECT max(reconsideration.last_event_sequence)
                                 FROM public.reconsiderations AS reconsideration
                                WHERE reconsideration.campaign_id =
                                      snapshot_gameplay.campaign_id
                                  AND reconsideration.original_event_sequence
                                      <= snapshot_gameplay.gameplay_cutoff_event_sequence
                           ), 0)
                       ) AS snapshot_cutoff_event_sequence
                  FROM snapshot_gameplay
            )
            SELECT jsonb_build_object(
                'source_campaign_id', source_session.campaign_id,
                'source_session_id', source_session.session_id,
                'source_cutoff_event_sequence',
                    source_session.snapshot_cutoff_event_sequence,
                'session_state', jsonb_build_object(
                    'state', source_session.state,
                    'active_scene_id', source_session.active_scene_id,
                    'version', source_session.version,
                    'visibility_label', source_session.visibility_label,
                    'visibility_subject', source_session.visibility_subject,
                    'started_at_unix_ms',
                        floor(extract(epoch FROM source_session.started_at) * 1000)::BIGINT,
                    'ended_at_unix_ms',
                        floor(extract(epoch FROM source_session.ended_at) * 1000)::BIGINT
                ),
                'character_state', '[]'::JSONB,
                'public_events', '[]'::JSONB,
                'discovered_clues', COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'clue_id', clue.clue_id,
                            'importance', clue.importance,
                            'outcome', clue.outcome,
                            'cost', clue.cost,
                            'version', clue.version,
                            'visibility_label', clue.visibility_label,
                            'visibility_subject', clue.visibility_subject
                        )
                        ORDER BY clue.clue_id
                    )
                      FROM public.clues AS clue
                     WHERE clue.campaign_id = source_session.campaign_id
                       AND clue.last_event_sequence
                           <= source_session.snapshot_cutoff_event_sequence
                       AND clue.revealed_to_party
                       AND clue.outcome <> 'NOT_FOUND'
                       AND clue.visibility_label::TEXT
                           IN ('public', 'party_visible')
                ), '[]'::JSONB),
                'scene_state', COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'scene_id', scene.scene_id,
                            'scene_key', scene.scene_key,
                            'name', scene.name,
                            'state', scene.state,
                            'version', scene.version,
                            'visibility_label', scene.visibility_label,
                            'visibility_subject', scene.visibility_subject
                        )
                        ORDER BY scene.scene_id
                    )
                      FROM public.scenes AS scene
                     WHERE scene.session_id = source_session.session_id
                       AND scene.last_event_sequence
                           <= source_session.snapshot_cutoff_event_sequence
                       AND scene.visibility_label::TEXT
                           IN ('public', 'party_visible')
                ), '[]'::JSONB),
                'world_state', jsonb_build_object(
                    'room_id', source_session.room_id,
                    'scenario_id', source_session.scenario_id,
                    'ruleset_id', (
                        SELECT scenario.ruleset_id
                          FROM public.scenarios AS scenario
                         WHERE scenario.scenario_id = source_session.scenario_id
                           AND scenario.campaign_id = source_session.campaign_id
                    ),
                    'visibility_label', (
                        SELECT scenario.visibility_label
                          FROM public.scenarios AS scenario
                         WHERE scenario.scenario_id = source_session.scenario_id
                           AND scenario.campaign_id = source_session.campaign_id
                    ),
                    'visibility_subject', (
                        SELECT scenario.visibility_subject
                          FROM public.scenarios AS scenario
                         WHERE scenario.scenario_id = source_session.scenario_id
                           AND scenario.campaign_id = source_session.campaign_id
                    )
                ),
                'combat_state', COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'combat_id', combat.combat_id,
                            'status', combat.status,
                            'round', combat.round,
                            'current_turn_index', combat.current_turn_index,
                            'state', combat.state_json,
                            'version', combat.version,
                            'visibility_label', combat.visibility_label,
                            'visibility_subject', combat.visibility_subject
                        )
                        ORDER BY combat.combat_id
                    )
                      FROM public.combat_states AS combat
                     WHERE combat.session_id = source_session.session_id
                       AND combat.last_event_sequence
                           <= source_session.snapshot_cutoff_event_sequence
                       AND combat.visibility_label::TEXT
                           IN ('public', 'party_visible')
                ), '[]'::JSONB),
                'chase_state', COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'chase_id', chase.chase_id,
                            'status', chase.status,
                            'range_band', chase.range_band,
                            'segment', chase.segment,
                            'state', chase.state_json,
                            'version', chase.version,
                            'visibility_label', chase.visibility_label,
                            'visibility_subject', chase.visibility_subject
                        )
                        ORDER BY chase.chase_id
                    )
                      FROM public.chase_states AS chase
                     WHERE chase.session_id = source_session.session_id
                       AND chase.last_event_sequence
                           <= source_session.snapshot_cutoff_event_sequence
                       AND chase.visibility_label::TEXT
                           IN ('public', 'party_visible')
                ), '[]'::JSONB),
                'conclusion_state', COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object(
                            'ending_event_id', ending.ending_event_id,
                            'ending_id', ending.ending_id,
                            'summary', ending.summary,
                            'version', ending.version,
                            'ended_at_unix_ms',
                                floor(extract(epoch FROM ending.ended_at) * 1000)::BIGINT,
                            'visibility_label', ending.visibility_label,
                            'visibility_subject', ending.visibility_subject
                        )
                        ORDER BY ending.ending_event_id
                    )
                      FROM public.ending_events AS ending
                     WHERE ending.session_id = source_session.session_id
                       AND ending.last_event_sequence
                           <= source_session.snapshot_cutoff_event_sequence
                       AND ending.visibility_label::TEXT
                           IN ('public', 'party_visible')
                ), '[]'::JSONB),
                'npc_state', '[]'::JSONB
            )
              FROM snapshot_source AS source_session
            "#,
        )
        .bind(source_session_id)
        .bind(parent_campaign_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_campaign_fork_snapshot"))?
        .ok_or(CoreDomainRepositoryError::NotFound("fork_source_session"))?;
        let cutoff_event_sequence = state
            .get("source_cutoff_event_sequence")
            .and_then(Value::as_i64)
            .filter(|sequence| *sequence > 0)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "fork_snapshot_cutoff_sequence",
            ))?;
        let replay_events = self.load_campaign_events(parent_campaign_id).await?;
        let public_events = replay_events
            .iter()
            .filter(|event| {
                event.sequence <= cutoff_event_sequence
                    && matches!(event.visibility_label.as_str(), "public" | "party_visible")
            })
            .map(|event| {
                if event.visibility_subject != "not_applicable"
                    || event.integrity_status != "verified_hmac"
                    || event.request_hash_source != "formal_commit"
                {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "fork_public_event_integrity",
                    ));
                }
                Ok(ForkSnapshotPublicEvent {
                    sequence: u64::try_from(event.sequence).map_err(|_| {
                        CoreDomainRepositoryError::Integrity("fork_public_event_sequence")
                    })?,
                    event_type: event.event_type.clone(),
                    resource_type: event.resource_type.clone(),
                    resource_id: event.resource_id.clone(),
                    payload: event.payload.clone(),
                    event_integrity_hash: event.event_integrity_hash.clone().ok_or(
                        CoreDomainRepositoryError::Integrity("fork_public_event_integrity_hash"),
                    )?,
                    visibility_label: event.visibility_label.clone(),
                    visibility_subject: event.visibility_subject.clone(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        state["public_events"] = serde_json::to_value(public_events)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        state["character_state"] = serde_json::to_value(reconstruct_fork_characters(
            &replay_events,
            parent_campaign_id,
            cutoff_event_sequence,
        )?)
        .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let snapshot = serde_json::json!({
            "schema_version": 1,
            "copy_scopes": DEFAULT_PUBLIC_COPY_SCOPES,
            "excluded_private_scopes": [
                CopyScope::KeeperNotes,
                CopyScope::HiddenClues,
                CopyScope::PrivateMessages,
                CopyScope::AiInternalMemory
            ],
            "state": state
        });
        let canonical_snapshot_json = serde_json::to_string(&snapshot)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let snapshot_hash = format!(
            "sha256:{:x}",
            Sha256::digest(canonical_snapshot_json.as_bytes())
        );
        Ok(CampaignForkSnapshotPreview {
            canonical_snapshot_json,
            snapshot_hash,
            copy_scopes: DEFAULT_PUBLIC_COPY_SCOPES.to_vec(),
        })
    }

    async fn build_campaign_fork_materialization(
        &self,
        request: &RecordCampaignForkRequest,
        snapshot: &CampaignForkSnapshotPreview,
    ) -> Result<CampaignForkMaterialization, CoreDomainRepositoryError> {
        let envelope: ForkSnapshotEnvelope =
            serde_json::from_str(&snapshot.canonical_snapshot_json)
                .map_err(|_| CoreDomainRepositoryError::Integrity("fork_snapshot_shape"))?;
        let source = envelope.state;
        if source.source_campaign_id != request.parent_campaign_id
            || source.source_session_id != request.source_session_id
            || source.session_state.state != "ENDED"
            || source.session_state.started_at_unix_ms == 0
            || source.session_state.ended_at_unix_ms < source.session_state.started_at_unix_ms
            || source.world_state.ruleset_id.trim().is_empty()
            || source.scene_state.is_empty()
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_snapshot_materialization_shape",
            ));
        }

        let child_rooms: Vec<String> = sqlx::query_scalar(
            "SELECT room_id FROM public.rooms WHERE campaign_id = $1 ORDER BY room_id LIMIT 2",
        )
        .bind(&request.child_campaign_id)
        .fetch_all(&self.primary)
        .await
        .map_err(database_error("load_fork_child_room"))?;
        if child_rooms.len() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_child_room_shape",
            ));
        }
        let child_room_id = child_rooms[0].clone();
        let child_scenario_id =
            fork_child_id(&request.fork_id, "scenario", &request.source_session_id)?;
        let child_session_id =
            fork_child_id(&request.fork_id, "session", &request.source_session_id)?;

        let mut scene_ids = BTreeMap::new();
        for scene in &source.scene_state {
            if !matches!(scene.state.as_str(), "READY" | "ACTIVE" | "CLOSED")
                || scene.scene_key.trim().is_empty()
                || scene.name.trim().is_empty()
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_scene_snapshot_shape",
                ));
            }
            scene_ids.insert(
                scene.scene_id.clone(),
                fork_child_id(&request.fork_id, "scene", &scene.scene_id)?,
            );
        }
        let child_active_scene_id =
            source
                .session_state
                .active_scene_id
                .as_ref()
                .map(|source_scene_id| {
                    scene_ids.get(source_scene_id).cloned().ok_or(
                        CoreDomainRepositoryError::Integrity("fork_active_scene_missing"),
                    )
                })
                .transpose()?;

        let scenario_document = serde_json::json!({
            "schema_version": 1,
            "kind": "FORK_SNAPSHOT",
            "fork_id": request.fork_id,
            "source_campaign_id": request.parent_campaign_id,
            "source_session_id": request.source_session_id,
            "source_snapshot_hash": snapshot.snapshot_hash,
            "endings": source
                .conclusion_state
                .iter()
                .map(|ending| serde_json::json!({
                    "id": ending.ending_id,
                    "summary": ending.summary
                }))
                .collect::<Vec<_>>()
        });
        let scenario_document_json = serde_json::to_string(&scenario_document)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let scenario_content_hash = format!(
            "sha256:{:x}",
            Sha256::digest(scenario_document_json.as_bytes())
        );
        let mut rows = vec![CampaignForkMaterializedRow::Scenario {
            scenario_id: child_scenario_id.clone(),
            ruleset_id: source.world_state.ruleset_id.clone(),
            format_version: "fork-snapshot-1".to_owned(),
            content_hash: scenario_content_hash,
            document_json: scenario_document_json,
            visibility_label: source.world_state.visibility_label.clone(),
            visibility_subject: source.world_state.visibility_subject.clone(),
        }];

        for character in &source.character_state {
            if !matches!(character.state.as_str(), "DRAFT" | "SUBMITTED" | "APPROVED")
                || character.display_name.trim().is_empty()
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_character_snapshot_shape",
                ));
            }
            let sheet =
                character
                    .current_sheet
                    .as_ref()
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "fork_character_sheet_missing",
                    ))?;
            if !sheet.sheet_json.is_object() {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_character_sheet_shape",
                ));
            }
            let (visibility_label, visibility_subject) =
                derive_fork_character_visibility(character, sheet)?;
            let character_id =
                fork_child_id(&request.fork_id, "character", &character.character_id)?;
            let sheet_version_id =
                fork_child_id(&request.fork_id, "sheet", &character.character_id)?;
            rows.push(CampaignForkMaterializedRow::Character {
                character_id,
                owner_user_id: character.owner_user_id.clone(),
                display_name: character.display_name.clone(),
                state: character.state.clone(),
                initial_version_locked: character.initial_version_locked,
                sheet_version_id,
                sheet_json: serde_json::to_string(&sheet.sheet_json)
                    .map_err(|_| CoreDomainRepositoryError::Serialization)?,
                sheet_locked: sheet.locked,
                visibility_label,
                visibility_subject,
            });
        }

        rows.push(CampaignForkMaterializedRow::Session {
            session_id: child_session_id.clone(),
            room_id: child_room_id.clone(),
            scenario_id: child_scenario_id.clone(),
            state: "ENDED".to_owned(),
            active_scene_id: child_active_scene_id,
            started_at_unix_ms: source.session_state.started_at_unix_ms,
            ended_at_unix_ms: source.session_state.ended_at_unix_ms,
            visibility_label: source.session_state.visibility_label.clone(),
            visibility_subject: source.session_state.visibility_subject.clone(),
        });
        for scene in &source.scene_state {
            rows.push(CampaignForkMaterializedRow::Scene {
                scene_id: scene_ids
                    .get(&scene.scene_id)
                    .expect("scene mapping was constructed above")
                    .clone(),
                session_id: child_session_id.clone(),
                scenario_id: child_scenario_id.clone(),
                room_id: child_room_id.clone(),
                scene_key: scene.scene_key.clone(),
                name: scene.name.clone(),
                state: scene.state.clone(),
                visibility_label: scene.visibility_label.clone(),
                visibility_subject: scene.visibility_subject.clone(),
            });
        }

        for public_event in &source.public_events {
            if public_event.sequence == 0
                || public_event.event_type.trim().is_empty()
                || public_event.resource_type.trim().is_empty()
                || public_event.resource_id.trim().is_empty()
                || !public_event.payload.is_object()
                || !public_event
                    .event_integrity_hash
                    .starts_with("hmac-sha256:")
                || !matches!(
                    public_event.visibility_label.as_str(),
                    "public" | "party_visible"
                )
                || public_event.visibility_subject != "not_applicable"
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_public_event_shape",
                ));
            }
            let source_event_sequence = public_event.sequence.to_string();
            rows.push(CampaignForkMaterializedRow::PublicEvent {
                fork_event_id: fork_child_id(
                    &request.fork_id,
                    "public_event",
                    &source_event_sequence,
                )?,
                source_event_sequence: public_event.sequence,
                source_event_type: public_event.event_type.clone(),
                source_resource_type: public_event.resource_type.clone(),
                source_resource_id: public_event.resource_id.clone(),
                source_payload_json: serde_json::to_string(&public_event.payload)
                    .map_err(|_| CoreDomainRepositoryError::Serialization)?,
                source_event_integrity_hash: public_event.event_integrity_hash.clone(),
                visibility_label: public_event.visibility_label.clone(),
                visibility_subject: public_event.visibility_subject.clone(),
            });
        }
        for clue in &source.discovered_clues {
            if clue.clue_id.trim().is_empty()
                || !matches!(clue.importance.as_str(), "CORE" | "OPTIONAL")
                || !matches!(clue.outcome.as_str(), "REVEALED" | "REVEALED_WITH_COST")
                || !matches!(clue.visibility_label.as_str(), "public" | "party_visible")
                || clue.visibility_subject != "not_applicable"
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_clue_snapshot_shape",
                ));
            }
            rows.push(CampaignForkMaterializedRow::DiscoveredClue {
                fork_clue_id: fork_child_id(&request.fork_id, "clue", &clue.clue_id)?,
                source_clue_id: clue.clue_id.clone(),
                importance: clue.importance.clone(),
                outcome: clue.outcome.clone(),
                cost: clue.cost.clone(),
                visibility_label: clue.visibility_label.clone(),
                visibility_subject: clue.visibility_subject.clone(),
            });
        }
        for npc in &source.npc_state {
            if npc.npc_id.trim().is_empty()
                || !npc.state.is_object()
                || !matches!(npc.visibility_label.as_str(), "public" | "party_visible")
                || npc.visibility_subject != "not_applicable"
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_npc_snapshot_shape",
                ));
            }
            rows.push(CampaignForkMaterializedRow::NpcState {
                npc_state_id: fork_child_id(&request.fork_id, "npc", &npc.npc_id)?,
                source_npc_id: npc.npc_id.clone(),
                state_json: serde_json::to_string(&npc.state)
                    .map_err(|_| CoreDomainRepositoryError::Serialization)?,
                visibility_label: npc.visibility_label.clone(),
                visibility_subject: npc.visibility_subject.clone(),
            });
        }
        for combat in &source.combat_state {
            if !matches!(combat.status.as_str(), "ONGOING" | "ENDED")
                || combat.round == 0
                || !combat.state.is_object()
                || !matches!(combat.visibility_label.as_str(), "public" | "party_visible")
                || combat.visibility_subject != "not_applicable"
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_combat_snapshot_shape",
                ));
            }
            let child_combat_id = fork_child_id(&request.fork_id, "combat", &combat.combat_id)?;
            let mut state = combat.state.clone();
            state["combat_id"] = Value::String(child_combat_id.clone());
            state["version"] = Value::from(1_u64);
            rows.push(CampaignForkMaterializedRow::Combat {
                combat_id: child_combat_id,
                session_id: child_session_id.clone(),
                status: combat.status.clone(),
                round: combat.round,
                current_turn_index: combat.current_turn_index,
                state_json: serde_json::to_string(&state)
                    .map_err(|_| CoreDomainRepositoryError::Serialization)?,
                visibility_label: combat.visibility_label.clone(),
                visibility_subject: combat.visibility_subject.clone(),
            });
        }
        for chase in &source.chase_state {
            if !matches!(chase.status.as_str(), "ONGOING" | "ESCAPED" | "CAUGHT")
                || chase.range_band > 5
                || chase.segment == 0
                || !chase.state.is_object()
                || !matches!(chase.visibility_label.as_str(), "public" | "party_visible")
                || chase.visibility_subject != "not_applicable"
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_chase_snapshot_shape",
                ));
            }
            let child_chase_id = fork_child_id(&request.fork_id, "chase", &chase.chase_id)?;
            let mut state = chase.state.clone();
            state["chase_id"] = Value::String(child_chase_id.clone());
            state["version"] = Value::from(1_u64);
            rows.push(CampaignForkMaterializedRow::Chase {
                chase_id: child_chase_id,
                session_id: child_session_id.clone(),
                status: chase.status.clone(),
                range_band: chase.range_band,
                segment: chase.segment,
                state_json: serde_json::to_string(&state)
                    .map_err(|_| CoreDomainRepositoryError::Serialization)?,
                visibility_label: chase.visibility_label.clone(),
                visibility_subject: chase.visibility_subject.clone(),
            });
        }
        if source.conclusion_state.len() > 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_conclusion_snapshot_shape",
            ));
        }
        for conclusion in &source.conclusion_state {
            if conclusion.ending_id.trim().is_empty()
                || conclusion.summary.trim().is_empty()
                || conclusion.summary.len() > 1_024
                || conclusion.ended_at_unix_ms == 0
                || !matches!(
                    conclusion.visibility_label.as_str(),
                    "public" | "party_visible"
                )
                || conclusion.visibility_subject != "not_applicable"
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_conclusion_snapshot_shape",
                ));
            }
            rows.push(CampaignForkMaterializedRow::Conclusion {
                ending_event_id: fork_child_id(
                    &request.fork_id,
                    "ending",
                    &conclusion.ending_event_id,
                )?,
                session_id: child_session_id.clone(),
                ending_id: conclusion.ending_id.clone(),
                summary: conclusion.summary.clone(),
                ended_at_unix_ms: conclusion.ended_at_unix_ms,
                visibility_label: conclusion.visibility_label.clone(),
                visibility_subject: conclusion.visibility_subject.clone(),
            });
        }

        let mut row_digest = Sha256::new();
        for row in &rows {
            let encoded =
                serde_json::to_vec(row).map_err(|_| CoreDomainRepositoryError::Serialization)?;
            row_digest.update((encoded.len() as u64).to_be_bytes());
            row_digest.update(encoded);
        }
        let materialized_root_hash = format!("sha256:{:x}", row_digest.finalize());
        let child_state = serde_json::json!({
            "schema_version": 2,
            "kind": "CONTENT_ADDRESSED_FORK_MATERIALIZATION",
            "fork_id": request.fork_id,
            "child_campaign_id": request.child_campaign_id,
            "source_snapshot_hash": snapshot.snapshot_hash,
            "materialized_row_count": rows.len(),
            "materialized_root_hash": materialized_root_hash
        });
        let child_state_json = serde_json::to_string(&child_state)
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let child_snapshot_hash =
            format!("sha256:{:x}", Sha256::digest(child_state_json.as_bytes()));
        let batches = fork_materialization_batches(&rows)?;
        Ok(CampaignForkMaterialization {
            child_session_id,
            child_scenario_id,
            child_state_json,
            child_snapshot_hash,
            rows,
            batches,
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
            || request.copy_scopes != DEFAULT_PUBLIC_COPY_SCOPES
            || metadata.visibility_label != "keeper_only"
            || metadata.visibility_subject != "not_applicable"
        {
            return Err(CoreDomainRepositoryError::InvalidInput("campaign_fork"));
        }
        self.ensure_campaign_admin(&request.parent_campaign_id, &metadata.requesting_actor_id)
            .await?;
        self.ensure_campaign_admin(&request.child_campaign_id, &metadata.requesting_actor_id)
            .await?;
        let snapshot = self
            .load_public_campaign_fork_snapshot(
                &request.parent_campaign_id,
                &request.source_session_id,
            )
            .await?;
        if snapshot.snapshot_hash != request.snapshot_hash {
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_fork_snapshot_hash_mismatch",
            ));
        }
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

        let existing_fork: Option<(String, String, String, String)> = sqlx::query_as(
            r#"
            SELECT parent_campaign_id, child_campaign_id, source_session_id,
                   source_snapshot_hash
              FROM public.campaign_forks
             WHERE fork_id = $1
            "#,
        )
        .bind(&request.fork_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_existing_campaign_fork_identity"))?;
        let retrying_projection = if let Some(existing) = existing_fork {
            if existing
                != (
                    request.parent_campaign_id.clone(),
                    request.child_campaign_id.clone(),
                    request.source_session_id.clone(),
                    request.snapshot_hash.clone(),
                )
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "campaign_fork_identity_conflict",
                ));
            }
            true
        } else {
            false
        };
        if !retrying_projection {
            let child_has_gameplay_state: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM public.scenarios WHERE campaign_id = $1
                    UNION ALL
                    SELECT 1 FROM public.characters WHERE campaign_id = $1
                    UNION ALL
                    SELECT 1 FROM core_domain.sessions WHERE campaign_id = $1
                    UNION ALL
                    SELECT 1 FROM public.campaign_forks WHERE child_campaign_id = $1
                    UNION ALL
                    SELECT 1 FROM public.campaign_fork_public_events WHERE campaign_id = $1
                    UNION ALL
                    SELECT 1 FROM public.campaign_fork_clues WHERE campaign_id = $1
                    UNION ALL
                    SELECT 1 FROM public.campaign_fork_npc_states WHERE campaign_id = $1
                    UNION ALL
                    SELECT 1 FROM public.combat_states WHERE campaign_id = $1
                    UNION ALL
                    SELECT 1 FROM public.chase_states WHERE campaign_id = $1
                    UNION ALL
                    SELECT 1 FROM public.ending_events WHERE campaign_id = $1
                )
                "#,
            )
            .bind(&request.child_campaign_id)
            .fetch_one(&self.primary)
            .await
            .map_err(database_error("check_fork_child_empty"))?;
            if child_has_gameplay_state {
                return Err(CoreDomainRepositoryError::Integrity("fork_child_not_empty"));
            }
        }

        let materialization = self
            .build_campaign_fork_materialization(request, &snapshot)
            .await?;
        let batch_count = u64::try_from(materialization.batches.len())
            .map_err(|_| CoreDomainRepositoryError::Integrity("fork_batch_count"))?;
        let materialized_row_count = u64::try_from(materialization.rows.len())
            .map_err(|_| CoreDomainRepositoryError::Integrity("fork_row_count"))?;
        if materialization.batches.len() + 2 > 256 {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_event_batch_limit",
            ));
        }
        let snapshot_reference_json = fork_snapshot_reference_json(&snapshot.snapshot_hash)?;
        let recorded = CoreDomainEvent::CampaignForkRecorded {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            fork_id: request.fork_id.clone(),
            parent_campaign_id: request.parent_campaign_id.clone(),
            child_campaign_id: request.child_campaign_id.clone(),
            source_session_id: request.source_session_id.clone(),
            snapshot_hash: request.snapshot_hash.clone(),
            child_snapshot_hash: materialization.child_snapshot_hash.clone(),
            copy_scopes: snapshot.copy_scopes.clone(),
            canonical_snapshot_json: snapshot_reference_json,
            reason: request.reason.clone(),
        };
        let manifest = CoreDomainEvent::CampaignForkMaterializationRecorded {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            fork_id: request.fork_id.clone(),
            child_campaign_id: request.child_campaign_id.clone(),
            child_session_id: materialization.child_session_id.clone(),
            child_scenario_id: materialization.child_scenario_id.clone(),
            child_snapshot_hash: materialization.child_snapshot_hash.clone(),
            child_state_json: materialization.child_state_json.clone(),
            materialized_row_count,
            batch_count,
        };
        let mut events = vec![
            (
                recorded,
                vec![projection_target("public.campaign_forks", &request.fork_id)],
            ),
            (
                manifest,
                vec![projection_target(
                    "public.campaign_fork_materializations",
                    &request.fork_id,
                )],
            ),
        ];
        for (index, batch) in materialization.batches.iter().enumerate() {
            let projection_targets = batch
                .rows
                .iter()
                .flat_map(fork_row_projection_targets)
                .collect::<Vec<_>>();
            if projection_targets.len() > 32 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_projection_target_limit",
                ));
            }
            events.push((
                CoreDomainEvent::CampaignForkMaterialized {
                    schema_version: CORE_EVENT_SCHEMA_VERSION,
                    fork_id: request.fork_id.clone(),
                    child_campaign_id: request.child_campaign_id.clone(),
                    batch_index: u64::try_from(index + 1)
                        .map_err(|_| CoreDomainRepositoryError::Integrity("fork_batch_index"))?,
                    batch_count,
                    rows: batch.rows.clone(),
                },
                projection_targets,
            ));
        }
        let mut draft = metadata.to_multi_event_draft(
            &request.child_campaign_id,
            &request.fork_id,
            "campaign_fork",
            "campaign.fork.record",
            events,
        )?;
        for (event, batch) in draft
            .events
            .iter_mut()
            .skip(2)
            .zip(&materialization.batches)
        {
            event.visibility = Some(CanonicalEventVisibility {
                label: batch.visibility_label.clone(),
                subject: batch.visibility_subject.clone(),
                data_subject_id: batch.data_subject_id.clone(),
            });
        }
        let persisted = self.canonical.commit(&draft).await?;
        let replay_events = self
            .load_campaign_events(&request.child_campaign_id)
            .await?
            .into_iter()
            .filter(|event| {
                event.sequence >= persisted.first_event_sequence
                    && event.sequence <= persisted.last_event_sequence
                    && event.stream_id == request.fork_id
            })
            .collect::<Vec<_>>();
        if replay_events.len() != materialization.batches.len() + 2 {
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_fork_event_batch_mismatch",
            ));
        }
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_campaign_fork")
            .await?;
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *transaction)
            .await
            .map_err(database_error("defer_campaign_fork_constraints"))?;
        for replay_event in &replay_events {
            apply_campaign_fork_replay_event(&mut transaction, replay_event).await?;
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
                review_workflow_version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, 'REQUESTED', NULL, $6, 1,
                2,
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
            || request.review_summary.trim().is_empty()
            || request.review_summary.len() > 512
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
                    review_summary,
                    ..
                } if reconsideration_id == &request.reconsideration_id
                    && review_event_id == &request.review_event_id
                    && review_summary == &request.review_summary
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
        if row.get::<String, _>("state") != "REQUESTED" {
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::EventChainInvalid,
            ));
        }
        let event = CoreDomainEvent::ReconsiderationReviewed {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            reconsideration_id: request.reconsideration_id.clone(),
            review_event_id: request.review_event_id.clone(),
            review_summary: request.review_summary.clone(),
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
        let result = sqlx::query(
            r#"
            UPDATE public.reconsiderations
               SET state = 'REVIEWED',
                   review_summary = $1,
                   event_chain = event_chain || jsonb_build_array($2::TEXT),
                   version = version + 1,
                   visibility_label = $3,
                   visibility_subject = $4,
                   provenance_kind = $5,
                   provenance_reference = $6,
                   provenance_recorded_by = $7,
                   last_event_sequence = $8
             WHERE reconsideration_id = $9
               AND state = 'REQUESTED'
               AND version = $10
            "#,
        )
        .bind(request.review_summary.trim())
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

    pub async fn resolve_reconsideration(
        &self,
        metadata: &CoreCommandMetadata,
        request: &ResolveReconsiderationRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if request.resolution_event_id.trim().is_empty()
            || request.resolution.trim().is_empty()
            || request.resolution.len() > 512
        {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "reconsideration_resolution",
            ));
        }
        let corrected_payload = match request.outcome {
            ReconsiderationOutcome::Upheld => {
                if request.corrected_event_type.is_some()
                    || request.corrected_payload_json.is_some()
                {
                    return Err(CoreDomainRepositoryError::InvalidInput(
                        "upheld_reconsideration_correction",
                    ));
                }
                None
            }
            ReconsiderationOutcome::Corrected => {
                let event_type = request
                    .corrected_event_type
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty() && value.len() <= 128)
                    .ok_or(CoreDomainRepositoryError::InvalidInput(
                        "corrected_event_type",
                    ))?;
                let payload_json = request.corrected_payload_json.as_deref().ok_or(
                    CoreDomainRepositoryError::InvalidInput("corrected_payload_json"),
                )?;
                let payload: Value = serde_json::from_str(payload_json).map_err(|_| {
                    CoreDomainRepositoryError::InvalidInput("corrected_payload_json")
                })?;
                if !payload.is_object() {
                    return Err(CoreDomainRepositoryError::InvalidInput(
                        "corrected_payload_json",
                    ));
                }
                Some((event_type.to_owned(), payload_json.to_owned()))
            }
        };
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let row = sqlx::query(
            r#"
            SELECT campaign_id, original_event_sequence, state, version, last_event_sequence
              FROM public.reconsiderations
             WHERE reconsideration_id = $1
            "#,
        )
        .bind(&request.reconsideration_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_reconsideration_for_resolution"))?
        .ok_or(CoreDomainRepositoryError::NotFound("reconsideration"))?;
        if row.get::<String, _>("campaign_id") != request.campaign_id {
            return Err(CoreDomainRepositoryError::Forbidden);
        }
        let current_version: i64 = row.get("version");
        let current_event_sequence: i64 = row.get("last_event_sequence");
        let original_event_sequence: i64 = row.get("original_event_sequence");
        let event_type = match request.outcome {
            ReconsiderationOutcome::Upheld => "ReconsiderationUpheld",
            ReconsiderationOutcome::Corrected => "ReconsiderationCorrected",
        };
        if self
            .projection_matches_command(current_event_sequence, metadata)
            .await?
        {
            let existing_event = self
                .load_idempotent_core_event(
                    &request.campaign_id,
                    &request.reconsideration_id,
                    metadata,
                    event_type,
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_reconsideration_resolution_event_missing",
                ))?;
            let matches_request = match (&existing_event, request.outcome, &corrected_payload) {
                (
                    CoreDomainEvent::ReconsiderationUpheld {
                        reconsideration_id,
                        resolution_event_id,
                        original_event_sequence: persisted_original,
                        resolution,
                        ..
                    },
                    ReconsiderationOutcome::Upheld,
                    None,
                ) => {
                    reconsideration_id == &request.reconsideration_id
                        && resolution_event_id == &request.resolution_event_id
                        && *persisted_original
                            == u64::try_from(original_event_sequence).unwrap_or_default()
                        && resolution == &request.resolution
                }
                (
                    CoreDomainEvent::ReconsiderationCorrected {
                        reconsideration_id,
                        resolution_event_id,
                        original_event_sequence: persisted_original,
                        resolution,
                        corrected_event_type,
                        corrected_payload_json,
                        ..
                    },
                    ReconsiderationOutcome::Corrected,
                    Some((requested_event_type, requested_payload)),
                ) => {
                    reconsideration_id == &request.reconsideration_id
                        && resolution_event_id == &request.resolution_event_id
                        && *persisted_original
                            == u64::try_from(original_event_sequence).unwrap_or_default()
                        && resolution == &request.resolution
                        && corrected_event_type == requested_event_type
                        && corrected_payload_json == requested_payload
                }
                _ => false,
            };
            if !matches_request {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_reconsideration_resolution_conflict",
                ));
            }
            return self
                .commit_event(
                    metadata,
                    &request.campaign_id,
                    &request.reconsideration_id,
                    ("reconsideration", "reconsideration.resolve"),
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
        if row.get::<String, _>("state") != "REVIEWED" {
            return Err(CoreDomainRepositoryError::Domain(
                CoreEntityError::EventChainInvalid,
            ));
        }
        let original_event_sequence = u64::try_from(original_event_sequence)
            .map_err(|_| CoreDomainRepositoryError::Integrity("original_event_sequence"))?;
        let event = match &corrected_payload {
            None => CoreDomainEvent::ReconsiderationUpheld {
                schema_version: CORE_EVENT_SCHEMA_VERSION,
                reconsideration_id: request.reconsideration_id.clone(),
                resolution_event_id: request.resolution_event_id.clone(),
                original_event_sequence,
                resolution: request.resolution.clone(),
            },
            Some((corrected_event_type, corrected_payload_json)) => {
                CoreDomainEvent::ReconsiderationCorrected {
                    schema_version: CORE_EVENT_SCHEMA_VERSION,
                    reconsideration_id: request.reconsideration_id.clone(),
                    resolution_event_id: request.resolution_event_id.clone(),
                    original_event_sequence,
                    resolution: request.resolution.clone(),
                    corrected_event_type: corrected_event_type.clone(),
                    corrected_payload_json: corrected_payload_json.clone(),
                }
            }
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.reconsideration_id,
                ("reconsideration", "reconsideration.resolve"),
                &event,
                vec![projection_target(
                    "public.reconsiderations",
                    &request.reconsideration_id,
                )],
            )
            .await?;
        let (outcome, corrected_event_type, corrected_payload_json) = match &corrected_payload {
            None => ("UPHELD", None, None),
            Some((event_type, payload_json)) => (
                "CORRECTED",
                Some(event_type.as_str()),
                Some(payload_json.as_str()),
            ),
        };
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_reconsideration_resolution")
            .await?;
        let result = sqlx::query(
            r#"
            UPDATE public.reconsiderations
               SET state = 'RESOLVED',
                   outcome = $1,
                   resolution = $2,
                   corrected_event_type = $3,
                   corrected_payload = $4::JSONB,
                   event_chain = event_chain || jsonb_build_array($5::TEXT),
                   version = version + 1,
                   visibility_label = $6,
                   visibility_subject = $7,
                   provenance_kind = $8,
                   provenance_reference = $9,
                   provenance_recorded_by = $10,
                   last_event_sequence = $11
             WHERE reconsideration_id = $12
               AND state = 'REVIEWED'
               AND version = $13
            "#,
        )
        .bind(outcome)
        .bind(request.resolution.trim())
        .bind(corrected_event_type)
        .bind(corrected_payload_json)
        .bind(&request.resolution_event_id)
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
        .map_err(database_error("project_reconsideration_resolution"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "reconsideration_resolution_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_reconsideration_resolution"))?;
        Ok(persisted)
    }

    pub async fn record_combat_state(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordCombatStateRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let next_version = metadata
            .expected_version
            .checked_add(1)
            .filter(|version| *version > 0)
            .ok_or(CoreDomainRepositoryError::InvalidInput("combat_version"))?;
        if request.state_json.is_empty() || request.state_json.len() > 1_048_576 {
            return Err(CoreDomainRepositoryError::InvalidInput("combat_state"));
        }
        let inspected = inspect_combat_state(&request.state_json)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_state"))?;
        validate_combat_server_roll_evidence(
            &request.state_json,
            request.attacker_roll.as_ref(),
            request.defender_roll.as_ref(),
            request.damage_roll.as_ref(),
            request.medical_roll.as_ref(),
        )
        .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_roll_evidence"))?;
        let combat_id = inspected.combat_id().to_owned();
        let status = inspected.status();
        let round = i64::from(inspected.round());
        let turn_index = i64::try_from(inspected.current_turn_index())
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_turn"))?;
        if i64::try_from(inspected.version()).ok() != Some(next_version) {
            return Err(CoreDomainRepositoryError::InvalidInput("combat_state"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        self.ensure_gameplay_session(&request.campaign_id, &request.session_id)
            .await?;
        let event = CoreDomainEvent::CombatStateRecorded {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            combat_id: combat_id.clone(),
            campaign_id: request.campaign_id.clone(),
            session_id: request.session_id.clone(),
            status: status.to_owned(),
            round: u64::try_from(round)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_round"))?,
            turn_index: u64::try_from(turn_index)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_turn"))?,
            version: u64::try_from(next_version)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_version"))?,
            state_json: request.state_json.clone(),
        };
        let existing = sqlx::query(
            r#"
            SELECT campaign_id, session_id, state_json, version,
                   last_event_sequence
              FROM public.combat_states
             WHERE combat_id = $1
            "#,
        )
        .bind(&combat_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_combat_state_transition"))?;
        let previous_state = if let Some(row) = existing {
            if row.get::<String, _>("campaign_id") != request.campaign_id
                || row.get::<String, _>("session_id") != request.session_id
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_state_identity_conflict",
                ));
            }
            let event_sequence: i64 = row.get("last_event_sequence");
            if self
                .projection_matches_command(event_sequence, metadata)
                .await?
            {
                let existing_event = self
                    .load_idempotent_core_event(
                        &request.campaign_id,
                        &combat_id,
                        metadata,
                        "CombatStateRecorded",
                    )
                    .await?
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "idempotent_combat_event_missing",
                    ))?;
                if existing_event != event {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "idempotent_combat_request_conflict",
                    ));
                }
                return self
                    .commit_event(
                        metadata,
                        &request.campaign_id,
                        &combat_id,
                        ("combat_state", "combat.state.record"),
                        &existing_event,
                        vec![projection_target("public.combat_states", &combat_id)],
                    )
                    .await;
            }
            if row.get::<i64, _>("version") != metadata.expected_version {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_state_projection_conflict",
                ));
            }
            Some(row.get::<Value, _>("state_json"))
        } else {
            None
        };
        if (metadata.expected_version == 0) != previous_state.is_none() {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_state_projection_conflict",
            ));
        }
        let previous_state_json = previous_state
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let validated =
            validate_combat_state_transition(previous_state_json.as_deref(), &request.state_json)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_transition"))?;
        if validated != inspected {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_state_validation_mismatch",
            ));
        }
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &combat_id,
                ("combat_state", "combat.state.record"),
                &event,
                vec![projection_target("public.combat_states", &combat_id)],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_combat_state")
            .await?;
        let result = sqlx::query(
            r#"
            INSERT INTO public.combat_states (
                combat_id, campaign_id, session_id, status, round,
                current_turn_index, state_json, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7::JSONB, $8,
                $9, $10, $11, $12, $13, $14
            )
            ON CONFLICT (combat_id) DO UPDATE
               SET status = EXCLUDED.status,
                   round = EXCLUDED.round,
                   current_turn_index = EXCLUDED.current_turn_index,
                   state_json = EXCLUDED.state_json,
                   version = EXCLUDED.version,
                   visibility_label = EXCLUDED.visibility_label,
                   visibility_subject = EXCLUDED.visibility_subject,
                   provenance_kind = EXCLUDED.provenance_kind,
                   provenance_reference = EXCLUDED.provenance_reference,
                   provenance_recorded_by = EXCLUDED.provenance_recorded_by,
                   last_event_sequence = EXCLUDED.last_event_sequence
             WHERE combat_states.campaign_id = EXCLUDED.campaign_id
               AND combat_states.session_id = EXCLUDED.session_id
               AND combat_states.status = 'ONGOING'
               AND combat_states.version = $15
            "#,
        )
        .bind(&combat_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(status)
        .bind(round)
        .bind(turn_index)
        .bind(&request.state_json)
        .bind(next_version)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(metadata.expected_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_combat_state"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_state_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_combat_state"))?;
        Ok(persisted)
    }

    pub async fn record_chase_state(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordChaseStateRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let next_version = metadata
            .expected_version
            .checked_add(1)
            .filter(|version| *version > 0)
            .ok_or(CoreDomainRepositoryError::InvalidInput("chase_version"))?;
        if request.state_json.is_empty() || request.state_json.len() > 1_048_576 {
            return Err(CoreDomainRepositoryError::InvalidInput("chase_state"));
        }
        let inspected = inspect_chase_state(&request.state_json)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_state"))?;
        validate_chase_server_roll_evidence(&request.state_json, &request.participant_rolls)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_roll_evidence"))?;
        let chase_id = inspected.chase_id().to_owned();
        let status = inspected.status();
        let range_band = i16::from(inspected.range());
        let segment = i64::from(inspected.segment());
        if i64::try_from(inspected.version()).ok() != Some(next_version) {
            return Err(CoreDomainRepositoryError::InvalidInput("chase_state"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        self.ensure_gameplay_session(&request.campaign_id, &request.session_id)
            .await?;
        let event = CoreDomainEvent::ChaseStateRecorded {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            chase_id: chase_id.clone(),
            campaign_id: request.campaign_id.clone(),
            session_id: request.session_id.clone(),
            status: status.to_owned(),
            range_band: u8::try_from(range_band)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_range"))?,
            segment: u64::try_from(segment)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_segment"))?,
            version: u64::try_from(next_version)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_version"))?,
            state_json: request.state_json.clone(),
        };
        let existing = sqlx::query(
            r#"
            SELECT campaign_id, session_id, state_json, version,
                   last_event_sequence
              FROM public.chase_states
             WHERE chase_id = $1
            "#,
        )
        .bind(&chase_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_chase_state_transition"))?;
        let previous_state = if let Some(row) = existing {
            if row.get::<String, _>("campaign_id") != request.campaign_id
                || row.get::<String, _>("session_id") != request.session_id
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "chase_state_identity_conflict",
                ));
            }
            let event_sequence: i64 = row.get("last_event_sequence");
            if self
                .projection_matches_command(event_sequence, metadata)
                .await?
            {
                let existing_event = self
                    .load_idempotent_core_event(
                        &request.campaign_id,
                        &chase_id,
                        metadata,
                        "ChaseStateRecorded",
                    )
                    .await?
                    .ok_or(CoreDomainRepositoryError::Integrity(
                        "idempotent_chase_event_missing",
                    ))?;
                if existing_event != event {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "idempotent_chase_request_conflict",
                    ));
                }
                return self
                    .commit_event(
                        metadata,
                        &request.campaign_id,
                        &chase_id,
                        ("chase_state", "chase.state.record"),
                        &existing_event,
                        vec![projection_target("public.chase_states", &chase_id)],
                    )
                    .await;
            }
            if row.get::<i64, _>("version") != metadata.expected_version {
                return Err(CoreDomainRepositoryError::Integrity(
                    "chase_state_projection_conflict",
                ));
            }
            Some(row.get::<Value, _>("state_json"))
        } else {
            None
        };
        if (metadata.expected_version == 0) != previous_state.is_none() {
            return Err(CoreDomainRepositoryError::Integrity(
                "chase_state_projection_conflict",
            ));
        }
        let previous_state_json = previous_state
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let validated =
            validate_chase_state_transition(previous_state_json.as_deref(), &request.state_json)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("chase_transition"))?;
        if validated != inspected {
            return Err(CoreDomainRepositoryError::Integrity(
                "chase_state_validation_mismatch",
            ));
        }
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &chase_id,
                ("chase_state", "chase.state.record"),
                &event,
                vec![projection_target("public.chase_states", &chase_id)],
            )
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_chase_state")
            .await?;
        let result = sqlx::query(
            r#"
            INSERT INTO public.chase_states (
                chase_id, campaign_id, session_id, status, range_band,
                segment, state_json, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7::JSONB, $8,
                $9, $10, $11, $12, $13, $14
            )
            ON CONFLICT (chase_id) DO UPDATE
               SET status = EXCLUDED.status,
                   range_band = EXCLUDED.range_band,
                   segment = EXCLUDED.segment,
                   state_json = EXCLUDED.state_json,
                   version = EXCLUDED.version,
                   visibility_label = EXCLUDED.visibility_label,
                   visibility_subject = EXCLUDED.visibility_subject,
                   provenance_kind = EXCLUDED.provenance_kind,
                   provenance_reference = EXCLUDED.provenance_reference,
                   provenance_recorded_by = EXCLUDED.provenance_recorded_by,
                   last_event_sequence = EXCLUDED.last_event_sequence
             WHERE chase_states.campaign_id = EXCLUDED.campaign_id
               AND chase_states.session_id = EXCLUDED.session_id
               AND chase_states.status = 'ONGOING'
               AND chase_states.version = $15
            "#,
        )
        .bind(&chase_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(status)
        .bind(range_band)
        .bind(segment)
        .bind(&request.state_json)
        .bind(next_version)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(metadata.expected_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_chase_state"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "chase_state_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_chase_state"))?;
        Ok(persisted)
    }

    pub async fn record_ending(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordEndingRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || request.ending_id.trim().is_empty()
            || request.summary.trim().is_empty()
            || request.summary.len() > 1_024
            || request.ended_at_unix_ms == 0
        {
            return Err(CoreDomainRepositoryError::InvalidInput("ending"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_ending")
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "p08-ending:{}:{}",
                request.campaign_id, request.session_id
            ))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_ending_session"))?;
        let session = sqlx::query(
            r#"
            SELECT session.state, scenario.document_json
              FROM core_domain.sessions AS session
              JOIN public.scenarios AS scenario
                ON scenario.scenario_id = session.scenario_id
               AND scenario.campaign_id = session.campaign_id
             WHERE session.session_id = $1
               AND session.campaign_id = $2
            "#,
        )
        .bind(&request.session_id)
        .bind(&request.campaign_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_ending_session"))?
        .ok_or(CoreDomainRepositoryError::NotFound("ending_session"))?;
        if session.get::<String, _>("state") != "ENDED" {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "ending_session_state",
            ));
        }
        let scenario_document: Value = session.get("document_json");
        let ending_is_defined = scenario_document
            .get("endings")
            .and_then(Value::as_array)
            .is_some_and(|endings| {
                endings.iter().any(|ending| {
                    ending.get("id").and_then(Value::as_str) == Some(request.ending_id.trim())
                })
            });
        if !ending_is_defined {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "ending_id_not_defined",
            ));
        }
        if let Some(existing_ending_event_id) = sqlx::query_scalar::<_, String>(
            "SELECT ending_event_id FROM public.ending_events WHERE session_id = $1",
        )
        .bind(&request.session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_existing_session_ending"))?
        {
            if existing_ending_event_id != request.ending_event_id {
                return Err(CoreDomainRepositoryError::Integrity(
                    "ending_session_already_recorded",
                ));
            }
        }
        let event = CoreDomainEvent::EndingRecorded {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            ending_event_id: request.ending_event_id.clone(),
            campaign_id: request.campaign_id.clone(),
            session_id: request.session_id.clone(),
            ending_id: request.ending_id.clone(),
            summary: request.summary.clone(),
            ended_at_unix_ms: request.ended_at_unix_ms,
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.ending_event_id,
                ("ending", "ending.record"),
                &event,
                vec![projection_target(
                    "public.ending_events",
                    &request.ending_event_id,
                )],
            )
            .await?;
        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.ending_events WHERE ending_event_id = $1",
        )
        .bind(&request.ending_event_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_existing_ending"))?
        {
            if existing_sequence == persisted.last_event_sequence
                && self
                    .projection_matches_command(existing_sequence, metadata)
                    .await?
            {
                return Ok(persisted);
            }
            return Err(CoreDomainRepositoryError::Integrity(
                "ending_identity_conflict",
            ));
        }
        let result = sqlx::query(
            r#"
            INSERT INTO public.ending_events (
                ending_event_id, campaign_id, session_id, ending_id, summary,
                ended_at, version, visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, 1, $7, $8, $9, $10, $11, $12
            )
            ON CONFLICT (ending_event_id) DO NOTHING
            "#,
        )
        .bind(&request.ending_event_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(&request.ending_id)
        .bind(request.summary.trim())
        .bind(timestamp_from_unix_ms(
            request.ended_at_unix_ms,
            "ending_timestamp",
        )?)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_ending"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "ending_identity_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_ending"))?;
        Ok(persisted)
    }

    pub async fn record_growth(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordGrowthRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || request.skill_name.trim().is_empty()
            || request.skill_name.len() > 128
        {
            return Err(CoreDomainRepositoryError::InvalidInput("growth"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let improvement_check = request.growth_rolls.improvement_check();
        let improvement_check_roll = improvement_check.value();
        let increase = request.growth_rolls.increase();
        let increase_roll = increase.map(|roll| roll.value());
        let server_roll_id = improvement_check.roll_id().to_owned();
        let increase_roll_id = increase.map(|roll| roll.roll_id().to_owned());
        if increase_roll_id.as_deref() == Some(server_roll_id.as_str()) {
            return Err(CoreDomainRepositoryError::InvalidInput("growth_rolls"));
        }
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_growth")
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!(
                "p08-growth:{}:{}",
                request.campaign_id, request.character_id
            ))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_growth_character"))?;
        if let Some(existing_sequence) = sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.growth_events WHERE growth_event_id = $1",
        )
        .bind(&request.growth_event_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_existing_growth"))?
        {
            if !self
                .projection_matches_command(existing_sequence, metadata)
                .await?
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "growth_event_identity_conflict",
                ));
            }
            let existing_event = self
                .load_idempotent_core_event(
                    &request.campaign_id,
                    &request.growth_event_id,
                    metadata,
                    "CharacterGrowthApplied",
                )
                .await?
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "idempotent_growth_event_missing",
                ))?;
            if !matches!(
                &existing_event,
                CoreDomainEvent::CharacterGrowthApplied {
                    growth_event_id,
                    campaign_id,
                    session_id,
                    ending_event_id,
                    character_id,
                    source_sheet_version_id,
                    new_sheet_version_id,
                    skill_name,
                    improvement_check_roll: event_check_roll,
                    increase_roll: event_increase_roll,
                    server_roll_id: event_server_roll_id,
                    increase_roll_id: event_increase_roll_id,
                    ..
                } if growth_event_id == &request.growth_event_id
                    && campaign_id == &request.campaign_id
                    && session_id == &request.session_id
                    && ending_event_id == &request.ending_event_id
                    && character_id == &request.character_id
                    && source_sheet_version_id == &request.source_sheet_version_id
                    && new_sheet_version_id == &request.new_sheet_version_id
                    && skill_name == request.skill_name.trim()
                    && event_check_roll == &improvement_check_roll
                    && event_increase_roll == &increase_roll
                    && event_server_roll_id == &server_roll_id
                    && event_increase_roll_id == &increase_roll_id
            ) {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_growth_request_conflict",
                ));
            }
            return self
                .commit_event(
                    metadata,
                    &request.campaign_id,
                    &request.growth_event_id,
                    ("growth", "growth.record"),
                    &existing_event,
                    vec![
                        projection_target("public.growth_events", &request.growth_event_id),
                        projection_target(
                            "public.character_sheet_versions",
                            &request.new_sheet_version_id,
                        ),
                        projection_target("public.characters", &request.character_id),
                    ],
                )
                .await;
        }
        if let Some(existing_growth_event_id) = sqlx::query_scalar::<_, String>(
            r#"
            SELECT growth_event_id
              FROM public.growth_events
             WHERE ending_event_id = $1
               AND character_id = $2
               AND skill_name = $3
            "#,
        )
        .bind(&request.ending_event_id)
        .bind(&request.character_id)
        .bind(request.skill_name.trim())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_existing_semantic_growth"))?
        {
            if existing_growth_event_id != request.growth_event_id {
                return Err(CoreDomainRepositoryError::Integrity(
                    "growth_skill_already_recorded",
                ));
            }
        }
        let row = sqlx::query(
            r#"
            SELECT character.current_sheet_version,
                   character.version AS character_version,
                   sheet.version AS sheet_version,
                   sheet.sheet_json,
                   ending.ending_id,
                   scenario.document_json
              FROM public.characters AS character
              JOIN public.character_sheet_versions AS sheet
                ON sheet.character_id = character.character_id
               AND sheet.version = character.current_sheet_version
              JOIN public.ending_events AS ending
                ON ending.ending_event_id = $1
               AND ending.campaign_id = character.campaign_id
               AND ending.session_id = $2
              JOIN core_domain.sessions AS session
                ON session.session_id = ending.session_id
               AND session.campaign_id = ending.campaign_id
              JOIN public.scenarios AS scenario
                ON scenario.scenario_id = session.scenario_id
               AND scenario.campaign_id = session.campaign_id
             WHERE character.character_id = $3
               AND character.campaign_id = $4
               AND sheet.sheet_version_id = $5
               AND sheet.locked
            "#,
        )
        .bind(&request.ending_event_id)
        .bind(&request.session_id)
        .bind(&request.character_id)
        .bind(&request.campaign_id)
        .bind(&request.source_sheet_version_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_growth_source"))?
        .ok_or(CoreDomainRepositoryError::NotFound("growth_source"))?;
        let source_version: i64 = row.get("sheet_version");
        if row.get::<i64, _>("current_sheet_version") != source_version {
            return Err(CoreDomainRepositoryError::Integrity(
                "growth_source_not_current",
            ));
        }
        let ending_id: String = row.get("ending_id");
        let scenario_document: Value = row.get("document_json");
        let skill_is_awarded = scenario_document
            .get("endings")
            .and_then(Value::as_array)
            .and_then(|endings| {
                endings.iter().find(|ending| {
                    ending.get("id").and_then(Value::as_str) == Some(ending_id.trim())
                })
            })
            .and_then(|ending| ending.get("growth_awards"))
            .and_then(Value::as_array)
            .is_some_and(|awards| {
                awards.iter().any(|award| {
                    award.get("skill_name").and_then(Value::as_str)
                        == Some(request.skill_name.trim())
                })
            });
        if !skill_is_awarded {
            return Err(CoreDomainRepositoryError::InvalidInput(
                "growth_skill_not_awarded",
            ));
        }
        let mut sheet_json: Value = row.get("sheet_json");
        let persisted_skill = sheet_json
            .get("skills")
            .and_then(Value::as_object)
            .and_then(|skills| skills.get(request.skill_name.trim()))
            .and_then(Value::as_i64);
        let skill_before = persisted_skill
            .and_then(|value| u8::try_from(value).ok())
            .filter(|value| *value <= 99)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "growth_skill_source_invalid",
            ))?;
        let qualifies = skill_before < 99
            && (improvement_check_roll > skill_before || improvement_check_roll >= 96);
        if qualifies != increase.is_some() {
            return Err(CoreDomainRepositoryError::InvalidInput("growth_rolls"));
        }
        let skill_after = increase_roll
            .map(|roll| skill_before.saturating_add(roll).min(99))
            .unwrap_or(skill_before);
        sheet_json
            .get_mut("skills")
            .and_then(Value::as_object_mut)
            .ok_or(CoreDomainRepositoryError::Integrity(
                "growth_sheet_skills_missing",
            ))?
            .insert(
                request.skill_name.trim().to_owned(),
                Value::from(skill_after),
            );
        let new_sheet_version =
            source_version
                .checked_add(1)
                .ok_or(CoreDomainRepositoryError::Integrity(
                    "growth_sheet_version_overflow",
                ))?;
        let event = CoreDomainEvent::CharacterGrowthApplied {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            growth_event_id: request.growth_event_id.clone(),
            campaign_id: request.campaign_id.clone(),
            session_id: request.session_id.clone(),
            ending_event_id: request.ending_event_id.clone(),
            character_id: request.character_id.clone(),
            source_sheet_version_id: request.source_sheet_version_id.clone(),
            new_sheet_version_id: request.new_sheet_version_id.clone(),
            skill_name: request.skill_name.trim().to_owned(),
            skill_before,
            improvement_check_roll,
            increase_roll,
            skill_after,
            server_roll_id: server_roll_id.clone(),
            increase_roll_id: increase_roll_id.clone(),
        };
        let persisted = self
            .commit_event(
                metadata,
                &request.campaign_id,
                &request.growth_event_id,
                ("growth", "growth.record"),
                &event,
                vec![
                    projection_target("public.growth_events", &request.growth_event_id),
                    projection_target(
                        "public.character_sheet_versions",
                        &request.new_sheet_version_id,
                    ),
                    projection_target("public.characters", &request.character_id),
                ],
            )
            .await?;
        let inserted_sheet = sqlx::query(
            r#"
            INSERT INTO public.character_sheet_versions (
                sheet_version_id, character_id, version, sheet_json, locked,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                campaign_id, last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, TRUE, $5, $6, $7, $8, $9, $10, $11
            )
            ON CONFLICT (sheet_version_id) DO NOTHING
            "#,
        )
        .bind(&request.new_sheet_version_id)
        .bind(&request.character_id)
        .bind(new_sheet_version)
        .bind(sqlx::types::Json(&sheet_json))
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(&request.campaign_id)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_growth_sheet"))?;
        if inserted_sheet.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "growth_sheet_identity_conflict",
            ));
        }
        let advanced_character = sqlx::query(
            r#"
            UPDATE public.characters
               SET current_sheet_version = $1,
                   version = version + 1,
                   visibility_label = $2,
                   visibility_subject = $3,
                   provenance_kind = $4,
                   provenance_reference = $5,
                   provenance_recorded_by = $6,
                   last_event_sequence = $7
             WHERE character_id = $8
               AND campaign_id = $9
               AND current_sheet_version = $10
               AND version = $11
            "#,
        )
        .bind(new_sheet_version)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(&request.character_id)
        .bind(&request.campaign_id)
        .bind(source_version)
        .bind(row.get::<i64, _>("character_version"))
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_growth_character"))?;
        if advanced_character.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "growth_character_projection_conflict",
            ));
        }
        let inserted_growth = sqlx::query(
            r#"
            INSERT INTO public.growth_events (
                growth_event_id, campaign_id, session_id, ending_event_id,
                character_id, source_sheet_version_id, new_sheet_version_id,
                skill_name, skill_before, improvement_check_roll,
                increase_roll, skill_after, server_roll_id, increase_roll_id, random_source,
                version, visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, 'SERVER_OS_CSPRNG', 1, $15, $16,
                $17, $18, $19, $20
            )
            ON CONFLICT (growth_event_id) DO NOTHING
            "#,
        )
        .bind(&request.growth_event_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(&request.ending_event_id)
        .bind(&request.character_id)
        .bind(&request.source_sheet_version_id)
        .bind(&request.new_sheet_version_id)
        .bind(request.skill_name.trim())
        .bind(i16::from(skill_before))
        .bind(i16::from(improvement_check_roll))
        .bind(increase_roll.map(i16::from))
        .bind(i16::from(skill_after))
        .bind(&server_roll_id)
        .bind(increase_roll_id.as_deref())
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_growth_event"))?;
        if inserted_growth.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "growth_event_identity_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_growth"))?;
        Ok(persisted)
    }

    async fn ensure_gameplay_session(
        &self,
        campaign_id: &str,
        session_id: &str,
    ) -> Result<(), CoreDomainRepositoryError> {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM core_domain.sessions
                 WHERE session_id = $1
                   AND campaign_id = $2
            )
            "#,
        )
        .bind(session_id)
        .bind(campaign_id)
        .fetch_one(&self.primary)
        .await
        .map_err(database_error("load_gameplay_session"))?;
        if exists {
            Ok(())
        } else {
            Err(CoreDomainRepositoryError::NotFound("gameplay_session"))
        }
    }
}
