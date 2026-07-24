use crate::authority_contract::DomainAuthorityContract;
use crate::ddd::{DomainResult, EntityId, FactProvenance, FactSource, Visibility};
use crate::visibility_fact_provenance::CommittedFactEvidence;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Campaign {
    pub campaign_id: EntityId,
    pub authority_contract: DomainAuthorityContract,
    pub current_version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterSheetVersion {
    pub character_id: EntityId,
    pub version: u64,
    pub source_event_id: EntityId,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryFact {
    fact_id: EntityId,
    source: FactSource,
    visibility: Visibility,
    fact_provenance: FactProvenance,
    source_event_sequence: u64,
    confirmed: bool,
}

impl MemoryFact {
    pub fn confirmed(
        fact_id: impl Into<String>,
        evidence: &CommittedFactEvidence,
    ) -> DomainResult<Self> {
        let fact_id = EntityId::new(fact_id)?;
        if &fact_id != evidence.target_fact_id() {
            return Err(crate::ddd::DomainError::CommittedFactEvidenceInvalid);
        }
        Ok(Self {
            fact_id,
            source: evidence.source(),
            visibility: evidence.visibility().clone(),
            fact_provenance: evidence.fact_provenance().clone(),
            source_event_sequence: evidence.event_sequence(),
            confirmed: true,
        })
    }

    pub const fn is_confirmed(&self) -> bool {
        self.confirmed
    }

    pub fn fact_id(&self) -> &EntityId {
        &self.fact_id
    }

    pub const fn source(&self) -> FactSource {
        self.source
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub fn fact_provenance(&self) -> &FactProvenance {
        &self.fact_provenance
    }

    pub const fn source_event_sequence(&self) -> u64 {
        self.source_event_sequence
    }
}
