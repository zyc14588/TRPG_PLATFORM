use crate::{append_coc7_event, validate_coc7_ruleset_id, Coc7EventPayload, COC7_RULESET_ID};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RulesetPackSdkContract {
    pub ruleset_id: &'static str,
    pub sdk_version: &'static str,
    pub exported_modules: Vec<&'static str>,
    pub extension_direct_state_write_allowed: bool,
    pub tool_gate_required: bool,
    pub provider_access_allowed: bool,
}

pub fn coc7_ruleset_pack_sdk_contract() -> RulesetPackSdkContract {
    RulesetPackSdkContract {
        ruleset_id: COC7_RULESET_ID,
        sdk_version: "v1",
        exported_modules: vec![
            "coc7",
            "rules_coc7",
            "dice_roll_contract",
            "sanity_madness_state_machine",
            "combat_state_machine",
            "chase_state_machine",
        ],
        extension_direct_state_write_allowed: false,
        tool_gate_required: true,
        provider_access_allowed: false,
    }
}

pub fn validate_ruleset_pack_sdk_contract(contract: &RulesetPackSdkContract) -> KernelResult<()> {
    validate_coc7_ruleset_id(contract.ruleset_id)?;
    if contract.extension_direct_state_write_allowed
        || !contract.tool_gate_required
        || contract.provider_access_allowed
    {
        return Err(TrpgError::InvalidConfiguration("ruleset_pack_sdk_contract"));
    }
    if !contract.exported_modules.contains(&"coc7")
        || !contract.exported_modules.contains(&"dice_roll_contract")
    {
        return Err(TrpgError::InvalidConfiguration("ruleset_pack_sdk_modules"));
    }
    Ok(())
}

pub fn record_ruleset_pack_sdk_registered<T>(
    authority: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    sdk_contract: &RulesetPackSdkContract,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    validate_ruleset_pack_sdk_contract(sdk_contract)?;
    append_coc7_event(
        authority,
        store,
        command,
        trpg_contracts::EventType::Coc7RulesetPackSdkRegistered.name(),
        "ruleset_pack_sdk",
        format!(
            "sdk={} modules={}",
            sdk_contract.sdk_version,
            sdk_contract.exported_modules.len()
        ),
    )
}
