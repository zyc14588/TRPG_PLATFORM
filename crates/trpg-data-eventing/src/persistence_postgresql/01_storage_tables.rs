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

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, TimeZone, Utc};
use hmac::{Hmac, Mac};
use ring::rand::{SecureRandom, SystemRandom};
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
    CampaignAggregate, CampaignForkMaterializedRow, CampaignInvite, Character,
    CharacterCombatHealthUpdate, CharacterState, CoreDomainEvent, CoreEntityError,
    ReconsiderationOutcome, Room, Session, SessionState, UserId,
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
const FORK_CHILD_LINEAGE_MARKER_RELATION: &str = "public.campaign_fork_materializations";
const SESSION_ENDING_RESERVATION_RELATION: &str = "core_domain.session_ending_reservation";

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

    #[test]
    fn fork_lineage_target_shape_preserves_pre_marker_retries() {
        let legacy = campaign_fork_recorded_projection_targets("fork_legacy_retry", false);
        assert_eq!(
            legacy,
            vec![projection_target(
                "public.campaign_forks",
                "fork_legacy_retry"
            )]
        );

        let child_owned_v2 = campaign_fork_recorded_projection_targets("fork_child_owned_v2", true);
        assert_eq!(child_owned_v2.len(), 2);
        assert!(child_owned_v2.iter().any(|target| {
            target.relation == FORK_CHILD_LINEAGE_MARKER_RELATION
                && target.row_id == "fork_child_owned_v2"
        }));
    }
}
