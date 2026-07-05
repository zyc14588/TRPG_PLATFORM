use crate::{append_coc7_event, validate_coc7_ruleset_id, Coc7EventPayload, COC7_RULESET_ID};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coc7RulesetPack {
    pub ruleset_id: &'static str,
    pub pack_version: &'static str,
    pub modules: Vec<&'static str>,
    pub server_dice_required: bool,
    pub event_log_required: bool,
    pub direct_llm_allowed: bool,
}

pub fn coc7_ruleset_pack() -> Coc7RulesetPack {
    Coc7RulesetPack {
        ruleset_id: COC7_RULESET_ID,
        pack_version: "v1",
        modules: vec![
            "dice_roll_contract",
            "sanity_madness_state_machine",
            "combat_state_machine",
            "chase_state_machine",
            "investigation_clue_npc_time",
        ],
        server_dice_required: true,
        event_log_required: true,
        direct_llm_allowed: false,
    }
}

pub fn validate_ruleset_pack(pack: &Coc7RulesetPack) -> KernelResult<()> {
    validate_coc7_ruleset_id(pack.ruleset_id)?;
    if !pack.server_dice_required || !pack.event_log_required || pack.direct_llm_allowed {
        return Err(TrpgError::InvalidConfiguration("ruleset_pack_governance"));
    }
    Ok(())
}

pub fn record_ruleset_pack_loaded<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    pack: &Coc7RulesetPack,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    validate_ruleset_pack(pack)?;
    append_coc7_event(
        contract,
        store,
        command,
        "coc7.ruleset_pack_loaded",
        "rule_runtime_coc7_ruleset_pack",
        format!("pack={} modules={}", pack.pack_version, pack.modules.len()),
    )
}
