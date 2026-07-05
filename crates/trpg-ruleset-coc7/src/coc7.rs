use crate::{append_coc7_event, validate_coc7_ruleset_id, Coc7EventPayload, COC7_RULESET_ID};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coc7GovernanceProfile {
    pub ruleset_id: &'static str,
    pub modules: Vec<&'static str>,
    pub server_dice_required: bool,
    pub event_log_required: bool,
    pub direct_llm_allowed: bool,
}

pub fn coc7_governance_profile() -> Coc7GovernanceProfile {
    Coc7GovernanceProfile {
        ruleset_id: COC7_RULESET_ID,
        modules: vec![
            "dice_roll_contract",
            "san",
            "combat_state_machine",
            "chase_state_machine",
            "investigation_clue_npc_time",
        ],
        server_dice_required: true,
        event_log_required: true,
        direct_llm_allowed: false,
    }
}

pub fn validate_coc7_governance_profile(profile: &Coc7GovernanceProfile) -> KernelResult<()> {
    validate_coc7_ruleset_id(profile.ruleset_id)?;
    if !profile.server_dice_required || !profile.event_log_required || profile.direct_llm_allowed {
        return Err(TrpgError::InvalidConfiguration("coc7_governance_profile"));
    }
    if !profile.modules.contains(&"dice_roll_contract")
        || !profile.modules.contains(&"investigation_clue_npc_time")
    {
        return Err(TrpgError::InvalidConfiguration("coc7_required_modules"));
    }
    Ok(())
}

pub fn record_coc7_governance_profile<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    profile: &Coc7GovernanceProfile,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    validate_coc7_governance_profile(profile)?;
    append_coc7_event(
        contract,
        store,
        command,
        "coc7.governance_profile_recorded",
        "coc7",
        format!("modules={}", profile.modules.len()),
    )
}
