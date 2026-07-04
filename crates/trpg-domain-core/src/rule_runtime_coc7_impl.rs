use crate::authority_contract::DomainAuthorityContract;
use crate::command_cqrs::{submit_domain_command, CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{CommandEnvelope, DomainResult, EventEnvelope, EventStore, FactSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coc7RuleRuntimeDecision {
    SkillCheck,
    OpposedRoll,
    SanityCheck,
    ChaseStep,
    CombatRound,
}

impl Coc7RuleRuntimeDecision {
    pub fn fact_source(self) -> FactSource {
        match self {
            Self::SkillCheck | Self::OpposedRoll | Self::SanityCheck => FactSource::DiceRoll,
            Self::ChaseStep | Self::CombatRound => FactSource::DecisionRecord,
        }
    }
}

pub fn record_rule_runtime_coc7_decision<T>(
    contract: &DomainAuthorityContract,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &CommandEnvelope<T>,
    decision: Coc7RuleRuntimeDecision,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    submit_domain_command(
        contract,
        store,
        command,
        DomainCommandKind::RecordDecision,
        decision.fact_source(),
    )
}
