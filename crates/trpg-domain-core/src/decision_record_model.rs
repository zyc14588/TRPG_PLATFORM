use crate::authority_contract::DomainAuthorityContract;
use crate::ddd::{
    Actor, ActorRole, AuthorityMode, DomainError, DomainResult, EntityId, EventEnvelope,
    FactProvenance, ProvenanceKind, Visibility,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionKind {
    Ruling,
    SkillCheck,
    Sanity,
    Fork,
    AdHoc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionRecord {
    decision_id: EntityId,
    campaign_id: EntityId,
    authority_contract_id: EntityId,
    authority_contract_version: u64,
    authority_owner: EntityId,
    decided_by: Actor,
    authority_mode: AuthorityMode,
    kind: DecisionKind,
    rules_reference: String,
    source_context_hash: String,
    linked_event_ids: Vec<EntityId>,
    linked_tool_result_ids: Vec<EntityId>,
    visibility: Visibility,
    fact_provenance: FactProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionRecordDraft {
    pub decision_id: String,
    pub campaign_id: String,
    pub authority_contract_id: String,
    pub authority_contract_version: u64,
    pub authority_owner: String,
    pub decided_by: Actor,
    pub authority_mode: AuthorityMode,
    pub kind: DecisionKind,
    pub rules_reference: String,
    pub source_context_hash: String,
    pub linked_event_ids: Vec<String>,
    pub linked_tool_result_ids: Vec<String>,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecisionEvidenceKind {
    Event,
    ToolResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DecisionEvidence {
    evidence_id: EntityId,
    campaign_id: EntityId,
    source_context_hash: String,
    visibility: Visibility,
    fact_provenance: FactProvenance,
    kind: DecisionEvidenceKind,
}

/// Canonical evidence resolver populated from the Event Store and persisted
/// Tool Results. Merely well-formed IDs are never treated as proof.
#[derive(Clone, Debug, Default)]
pub struct DecisionEvidenceCatalog {
    evidence_by_id: HashMap<EntityId, DecisionEvidence>,
}

impl DecisionEvidenceCatalog {
    pub fn register_event<P>(
        &mut self,
        event: &EventEnvelope<P>,
        source_context_hash: impl Into<String>,
    ) -> DomainResult<()> {
        event
            .verify_recorded_integrity()
            .map_err(|_| DomainError::InvalidConfirmedFactSource)?;
        if !matches!(
            event.event_type,
            "GameEvent" | "DecisionCommitted" | "DiceRoll" | "RulesDecision"
        ) {
            return Err(DomainError::InvalidConfirmedFactSource);
        }
        self.register(
            event.fact_provenance.reference.clone(),
            event.campaign_id.clone(),
            source_context_hash,
            event.visibility.clone(),
            event.fact_provenance.clone(),
            DecisionEvidenceKind::Event,
        )
    }

    pub fn register_tool_result<P>(
        &mut self,
        event: &EventEnvelope<P>,
        source_context_hash: impl Into<String>,
    ) -> DomainResult<()> {
        event
            .verify_recorded_integrity()
            .map_err(|_| DomainError::InvalidConfirmedFactSource)?;
        if !matches!(event.event_type, "ToolResult" | "ToolResultRecorded") {
            return Err(DomainError::InvalidConfirmedFactSource);
        }
        self.register(
            event.fact_provenance.reference.clone(),
            event.campaign_id.clone(),
            source_context_hash,
            event.visibility.clone(),
            event.fact_provenance.clone(),
            DecisionEvidenceKind::ToolResult,
        )
    }

    fn register(
        &mut self,
        evidence_id: EntityId,
        campaign_id: EntityId,
        source_context_hash: impl Into<String>,
        visibility: Visibility,
        fact_provenance: FactProvenance,
        kind: DecisionEvidenceKind,
    ) -> DomainResult<()> {
        let evidence = DecisionEvidence {
            evidence_id,
            campaign_id,
            source_context_hash: source_context_hash.into(),
            visibility,
            fact_provenance,
            kind,
        };
        if !is_sha256(&evidence.source_context_hash) {
            return Err(DomainError::MissingCommandMetadata);
        }
        if self.evidence_by_id.contains_key(&evidence.evidence_id) {
            return Err(DomainError::DuplicateCommand);
        }
        self.evidence_by_id
            .insert(evidence.evidence_id.clone(), evidence);
        Ok(())
    }

    fn resolve(&self, evidence_id: &EntityId) -> DomainResult<&DecisionEvidence> {
        self.evidence_by_id
            .get(evidence_id)
            .ok_or(DomainError::InvalidConfirmedFactSource)
    }
}

impl DecisionRecord {
    pub fn from_draft(draft: DecisionRecordDraft) -> DomainResult<Self> {
        let record = Self {
            decision_id: EntityId::new(draft.decision_id)?,
            campaign_id: EntityId::new(draft.campaign_id)?,
            authority_contract_id: EntityId::new(draft.authority_contract_id)?,
            authority_contract_version: draft.authority_contract_version,
            authority_owner: EntityId::new(draft.authority_owner)?,
            decided_by: draft.decided_by,
            authority_mode: draft.authority_mode,
            kind: draft.kind,
            rules_reference: draft.rules_reference,
            source_context_hash: draft.source_context_hash,
            linked_event_ids: draft
                .linked_event_ids
                .into_iter()
                .map(EntityId::new)
                .collect::<Result<Vec<_>, _>>()?,
            linked_tool_result_ids: draft
                .linked_tool_result_ids
                .into_iter()
                .map(EntityId::new)
                .collect::<Result<Vec<_>, _>>()?,
            visibility: draft.visibility,
            fact_provenance: draft.fact_provenance,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> DomainResult<()> {
        if self.rules_reference.trim().is_empty()
            || self.authority_contract_version == 0
            || !is_sha256(&self.source_context_hash)
            || (self.linked_event_ids.is_empty() && self.linked_tool_result_ids.is_empty())
        {
            return Err(DomainError::MissingCommandMetadata);
        }

        if !matches!(
            self.fact_provenance.kind,
            ProvenanceKind::HumanKeeperStatement
                | ProvenanceKind::RulesEngineDecision
                | ProvenanceKind::ToolResult
        ) {
            return Err(DomainError::InvalidConfirmedFactSource);
        }

        let provenance_is_linked = self
            .linked_event_ids
            .iter()
            .chain(self.linked_tool_result_ids.iter())
            .any(|id| id == &self.fact_provenance.reference);
        if !provenance_is_linked || self.fact_provenance.recorded_by != *self.decided_by.id() {
            return Err(DomainError::InvalidConfirmedFactSource);
        }

        Ok(())
    }

    pub fn validate_against(&self, contract: &DomainAuthorityContract) -> DomainResult<()> {
        self.validate()?;
        if &self.campaign_id != contract.campaign_id() {
            return Err(DomainError::CampaignScopeMismatch);
        }
        if &self.authority_contract_id != contract.contract_id() {
            return Err(DomainError::AuthorityContractImmutable);
        }
        if self.authority_contract_version != contract.version() {
            return Err(DomainError::AuthorityContractVersionConflict);
        }
        if &self.authority_owner != contract.authority_owner()
            || self.decided_by.id() != contract.authority_owner()
        {
            return Err(DomainError::AuthorityOwnerMismatch);
        }
        if &self.authority_mode != contract.mode() {
            return Err(DomainError::AuthorityViolation);
        }
        match (&self.authority_mode, self.decided_by.role()) {
            (AuthorityMode::HumanKp, ActorRole::HumanKeeper)
            | (AuthorityMode::AiKp, ActorRole::AiKeeper) => Ok(()),
            _ => Err(DomainError::AuthorityViolation),
        }
    }

    pub fn validate_against_evidence(
        &self,
        contract: &DomainAuthorityContract,
        evidence: &DecisionEvidenceCatalog,
    ) -> DomainResult<()> {
        self.validate_against(contract)?;
        for event_id in &self.linked_event_ids {
            self.validate_evidence(evidence.resolve(event_id)?, DecisionEvidenceKind::Event)?;
        }
        for tool_result_id in &self.linked_tool_result_ids {
            self.validate_evidence(
                evidence.resolve(tool_result_id)?,
                DecisionEvidenceKind::ToolResult,
            )?;
        }
        Ok(())
    }

    fn validate_evidence(
        &self,
        evidence: &DecisionEvidence,
        expected_kind: DecisionEvidenceKind,
    ) -> DomainResult<()> {
        if evidence.kind != expected_kind
            || evidence.campaign_id != self.campaign_id
            || evidence.source_context_hash != self.source_context_hash
            || evidence.visibility != self.visibility
        {
            return Err(DomainError::InvalidConfirmedFactSource);
        }
        if evidence.evidence_id == self.fact_provenance.reference
            && (evidence.fact_provenance.kind != self.fact_provenance.kind
                || evidence.fact_provenance.recorded_by != self.fact_provenance.recorded_by)
        {
            return Err(DomainError::InvalidConfirmedFactSource);
        }
        Ok(())
    }

    pub fn decision_id(&self) -> &EntityId {
        &self.decision_id
    }

    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn source_context_hash(&self) -> &str {
        &self.source_context_hash
    }

    pub fn linked_event_ids(&self) -> &[EntityId] {
        &self.linked_event_ids
    }

    pub fn linked_tool_result_ids(&self) -> &[EntityId] {
        &self.linked_tool_result_ids
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub fn fact_provenance(&self) -> &FactProvenance {
        &self.fact_provenance
    }
}

fn is_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}
