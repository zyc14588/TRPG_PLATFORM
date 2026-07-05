use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, ProvenanceKind,
    TrpgError,
};

pub const COC7_RULESET_ID: &str = "coc7";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coc7EventPayload {
    pub ruleset_id: &'static str,
    pub decision_type: &'static str,
    pub summary: String,
    pub visibility_label: &'static str,
    pub provenance_kind: ProvenanceKind,
}

impl Coc7EventPayload {
    pub fn from_command<T>(
        command: &CommandEnvelope<T>,
        decision_type: &'static str,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            ruleset_id: COC7_RULESET_ID,
            decision_type,
            summary: summary.into(),
            visibility_label: command.visibility.label().as_str(),
            provenance_kind: command.fact_provenance.kind.clone(),
        }
    }
}

pub fn validate_coc7_ruleset_id(ruleset_id: &str) -> KernelResult<()> {
    if ruleset_id == COC7_RULESET_ID {
        Ok(())
    } else {
        Err(TrpgError::InvalidConfiguration("coc7_ruleset_id"))
    }
}

pub fn append_coc7_event<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    event_type: &'static str,
    decision_type: &'static str,
    summary: impl Into<String>,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    contract.validate_command(command)?;
    store.append(
        command,
        event_type,
        Coc7EventPayload::from_command(command, decision_type, summary),
    )
}
