use crate::{append_coc7_event, validate_coc7_ruleset_id, Coc7EventPayload, COC7_RULESET_ID};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coc7ReadmeContract {
    pub ruleset_id: &'static str,
    pub command_endpoint: &'static str,
    pub event_store: &'static str,
    pub realtime_subjects: Vec<&'static str>,
    pub projection_is_canon: bool,
    pub direct_model_access_allowed: bool,
}

pub fn coc7_readme_contract() -> Coc7ReadmeContract {
    Coc7ReadmeContract {
        ruleset_id: COC7_RULESET_ID,
        command_endpoint: "/api/v1/rulesets/coc7/commands",
        event_store: "coc7_rule_events",
        realtime_subjects: vec![
            "trpg.rules.coc7.dice.rolled",
            "trpg.rules.coc7.state.changed",
        ],
        projection_is_canon: false,
        direct_model_access_allowed: false,
    }
}

pub fn validate_coc7_readme_contract(contract: &Coc7ReadmeContract) -> KernelResult<()> {
    validate_coc7_ruleset_id(contract.ruleset_id)?;
    if contract.projection_is_canon || contract.direct_model_access_allowed {
        return Err(TrpgError::InvalidConfiguration("coc7_readme_contract"));
    }
    if contract.command_endpoint != "/api/v1/rulesets/coc7/commands" {
        return Err(TrpgError::InvalidConfiguration("coc7_command_endpoint"));
    }
    Ok(())
}

pub fn record_coc7_readme_contract<T>(
    authority: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    readme_contract: &Coc7ReadmeContract,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    validate_coc7_readme_contract(readme_contract)?;
    append_coc7_event(
        authority,
        store,
        command,
        "coc7.readme_contract_recorded",
        "readme",
        format!("subjects={}", readme_contract.realtime_subjects.len()),
    )
}
