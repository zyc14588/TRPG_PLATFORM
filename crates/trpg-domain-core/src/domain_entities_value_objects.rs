use crate::authority_contract::DomainAuthorityContract;
use crate::ddd::{
    require_confirmable_fact_source, DomainResult, EntityId, FactProvenance, FactSource, Visibility,
};

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
    pub fact_id: EntityId,
    pub source: FactSource,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
    pub confirmed: bool,
}

impl MemoryFact {
    pub fn confirmed(
        fact_id: impl Into<String>,
        source: FactSource,
        visibility: Visibility,
        fact_provenance: FactProvenance,
    ) -> DomainResult<Self> {
        require_confirmable_fact_source(source)?;

        Ok(Self {
            fact_id: EntityId::new(fact_id)?,
            source,
            visibility,
            fact_provenance,
            confirmed: true,
        })
    }
}
