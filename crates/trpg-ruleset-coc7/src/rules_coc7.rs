use crate::{append_coc7_event, validate_coc7_ruleset_id, Coc7EventPayload, COC7_RULESET_ID};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coc7RulesetMetadata {
    pub ruleset_id: &'static str,
    pub edition: &'static str,
    pub server_dice_required: bool,
    pub event_log_required: bool,
    pub direct_llm_allowed: bool,
}

pub fn rules_coc7_metadata() -> Coc7RulesetMetadata {
    Coc7RulesetMetadata {
        ruleset_id: COC7_RULESET_ID,
        edition: "Call of Cthulhu 7th Edition",
        server_dice_required: true,
        event_log_required: true,
        direct_llm_allowed: false,
    }
}

pub fn assert_rules_coc7_request(ruleset_id: &str) -> KernelResult<()> {
    validate_coc7_ruleset_id(ruleset_id)
}

pub fn record_rules_coc7_dispatch<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    route: &'static str,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        "coc7.rules_dispatch_recorded",
        "rules_coc7",
        format!("route={route}"),
    )
}
