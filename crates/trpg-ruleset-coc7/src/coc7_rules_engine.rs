use crate::{append_coc7_event, Coc7EventPayload};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coc7EngineDecision {
    SkillCheck,
    OpposedRoll,
    SanityCheck,
    CombatRound,
    ChaseRound,
    InvestigationStep,
}

pub fn engine_decision_route(decision: Coc7EngineDecision) -> &'static str {
    match decision {
        Coc7EngineDecision::SkillCheck => "skill_check",
        Coc7EngineDecision::OpposedRoll => "opposed_roll",
        Coc7EngineDecision::SanityCheck => "sanity_check",
        Coc7EngineDecision::CombatRound => "combat_round",
        Coc7EngineDecision::ChaseRound => "chase_round",
        Coc7EngineDecision::InvestigationStep => "investigation_step",
    }
}

pub fn record_coc7_rules_engine_decision<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    decision: Coc7EngineDecision,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        "coc7.rules_engine_decision_recorded",
        "coc7_rules_engine",
        format!("decision={}", engine_decision_route(decision)),
    )
}
