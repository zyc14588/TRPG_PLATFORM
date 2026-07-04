use crate::authority_contract::DomainAuthorityContract;
use crate::command_cqrs::{submit_domain_command, CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{CommandEnvelope, DomainResult, EventEnvelope, EventStore, FactSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvestigationClueNpcTimeTrack {
    Investigation,
    Clue,
    Npc,
    Time,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvestigationClueNpcTimeDecision {
    pub track: InvestigationClueNpcTimeTrack,
    pub fact_source: FactSource,
}

impl InvestigationClueNpcTimeDecision {
    pub fn for_track(track: InvestigationClueNpcTimeTrack) -> Self {
        let fact_source = match track {
            InvestigationClueNpcTimeTrack::Clue => FactSource::ClueRevealEvent,
            InvestigationClueNpcTimeTrack::Investigation
            | InvestigationClueNpcTimeTrack::Npc
            | InvestigationClueNpcTimeTrack::Time => FactSource::DecisionRecord,
        };

        Self { track, fact_source }
    }
}

pub fn record_investigation_clue_npc_time_decision<T>(
    contract: &DomainAuthorityContract,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &CommandEnvelope<T>,
    decision: InvestigationClueNpcTimeDecision,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    submit_domain_command(
        contract,
        store,
        command,
        DomainCommandKind::RecordDecision,
        decision.fact_source,
    )
}
