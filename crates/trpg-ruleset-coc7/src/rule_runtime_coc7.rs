use crate::{append_coc7_event, Coc7EventPayload};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleRuntimeCoc7Call {
    SkillCheck,
    SanityCheck,
    CombatRound,
    ChaseRound,
    InvestigationStep,
}

pub fn rule_runtime_coc7_subject(call: RuleRuntimeCoc7Call) -> &'static str {
    match call {
        RuleRuntimeCoc7Call::SkillCheck => "ruleset.coc7.skill_check.decide",
        RuleRuntimeCoc7Call::SanityCheck => "ruleset.coc7.sanity_check.decide",
        RuleRuntimeCoc7Call::CombatRound => "ruleset.coc7.combat_round.decide",
        RuleRuntimeCoc7Call::ChaseRound => "ruleset.coc7.chase_round.decide",
        RuleRuntimeCoc7Call::InvestigationStep => "ruleset.coc7.investigation_step.decide",
    }
}

pub fn record_rule_runtime_coc7_decision<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    call: RuleRuntimeCoc7Call,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        "coc7.rule_runtime_decision_recorded",
        "rule_runtime_coc7",
        format!("subject={}", rule_runtime_coc7_subject(call)),
    )
}
