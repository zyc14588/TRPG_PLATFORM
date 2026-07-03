use crate::ddd::{
    Actor, AuthorityMode, DomainError, DomainResult, EntityId, FactProvenance, Visibility,
};

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
    pub decision_id: EntityId,
    pub campaign_id: EntityId,
    pub decided_by: Actor,
    pub authority_mode: AuthorityMode,
    pub kind: DecisionKind,
    pub rules_reference: String,
    pub source_context_hash: String,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionRecordDraft {
    pub decision_id: String,
    pub campaign_id: String,
    pub decided_by: Actor,
    pub authority_mode: AuthorityMode,
    pub kind: DecisionKind,
    pub rules_reference: String,
    pub source_context_hash: String,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
}

impl DecisionRecord {
    pub fn from_draft(draft: DecisionRecordDraft) -> DomainResult<Self> {
        let record = Self {
            decision_id: EntityId::new(draft.decision_id)?,
            campaign_id: EntityId::new(draft.campaign_id)?,
            decided_by: draft.decided_by,
            authority_mode: draft.authority_mode,
            kind: draft.kind,
            rules_reference: draft.rules_reference,
            source_context_hash: draft.source_context_hash,
            visibility: draft.visibility,
            fact_provenance: draft.fact_provenance,
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> DomainResult<()> {
        if self.rules_reference.trim().is_empty() || self.source_context_hash.trim().is_empty() {
            return Err(DomainError::MissingCommandMetadata);
        }

        Ok(())
    }
}
