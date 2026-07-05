use crate::{append_coc7_event, validate_coc7_ruleset_id, Coc7EventPayload, COC7_RULESET_ID};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coc7RuntimeGovernance {
    pub ruleset_id: &'static str,
    pub gateway_required: bool,
    pub direct_llm_allowed: bool,
    pub direct_state_write_allowed: bool,
}

pub fn coc7_runtime_governance() -> Coc7RuntimeGovernance {
    Coc7RuntimeGovernance {
        ruleset_id: COC7_RULESET_ID,
        gateway_required: true,
        direct_llm_allowed: false,
        direct_state_write_allowed: false,
    }
}

pub fn validate_coc7_runtime_governance(governance: &Coc7RuntimeGovernance) -> KernelResult<()> {
    validate_coc7_ruleset_id(governance.ruleset_id)?;
    if !governance.gateway_required
        || governance.direct_llm_allowed
        || governance.direct_state_write_allowed
    {
        return Err(TrpgError::InvalidConfiguration("coc7_runtime_governance"));
    }
    Ok(())
}

pub fn record_coc7_runtime_governance<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    governance: &Coc7RuntimeGovernance,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    validate_coc7_runtime_governance(governance)?;
    append_coc7_event(
        contract,
        store,
        command,
        "coc7.runtime_governance_recorded",
        "coc7_rule_runtime",
        "gateway_required=true direct_llm_allowed=false",
    )
}
