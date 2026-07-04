use crate::authority_contract::DomainAuthorityContract;
use crate::command_cqrs::{submit_domain_command, CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{CommandEnvelope, DomainResult, EventEnvelope, EventStore, FactSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterCombatSanChaseTrack {
    Character,
    Combat,
    Sanity,
    Chase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterCombatSanChaseDecision {
    pub track: CharacterCombatSanChaseTrack,
    pub command_kind: DomainCommandKind,
    pub fact_source: FactSource,
}

impl CharacterCombatSanChaseDecision {
    pub fn for_track(track: CharacterCombatSanChaseTrack) -> Self {
        let fact_source = match track {
            CharacterCombatSanChaseTrack::Character => FactSource::CharacterSheetVersion,
            CharacterCombatSanChaseTrack::Combat
            | CharacterCombatSanChaseTrack::Sanity
            | CharacterCombatSanChaseTrack::Chase => FactSource::DiceRoll,
        };

        Self {
            track,
            command_kind: DomainCommandKind::RecordDecision,
            fact_source,
        }
    }
}

pub fn record_character_combat_san_chase_decision<T>(
    contract: &DomainAuthorityContract,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &CommandEnvelope<T>,
    decision: CharacterCombatSanChaseDecision,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    submit_domain_command(
        contract,
        store,
        command,
        decision.command_kind,
        decision.fact_source,
    )
}
