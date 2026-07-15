use trpg_contracts::{CanonicalEventHeader, WireErrorCode};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, ProvenanceKind,
    TrpgError,
};

pub const COC7_RULESET_ID: &str = "coc7";

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Coc7EventPayload {
    pub ruleset_id: &'static str,
    pub decision_type: &'static str,
    pub summary: String,
    pub visibility_label: &'static str,
    pub provenance_kind: ProvenanceKind,
    pub schema_version: u16,
    pub schema_id: &'static str,
}

impl Coc7EventPayload {
    fn from_command<T>(
        command: &CommandEnvelope<T>,
        event_type: &'static str,
        decision_type: &'static str,
        summary: impl Into<String>,
    ) -> KernelResult<Self> {
        let header =
            CanonicalEventHeader::resolve(event_type).map_err(|error| match error.code {
                WireErrorCode::EventContractUnknown => TrpgError::EventContractUnknown,
                WireErrorCode::EventContractVersionMismatch => {
                    TrpgError::EventContractVersionMismatch
                }
                _ => TrpgError::EventContractUnknown,
            })?;
        Ok(Self {
            ruleset_id: COC7_RULESET_ID,
            decision_type,
            summary: summary.into(),
            visibility_label: command.visibility.label().as_str(),
            provenance_kind: command.fact_provenance.kind.clone(),
            schema_version: header.schema_version,
            schema_id: header.schema_id,
        })
    }
}

pub fn validate_coc7_ruleset_id(ruleset_id: &str) -> KernelResult<()> {
    if ruleset_id == COC7_RULESET_ID {
        Ok(())
    } else {
        Err(TrpgError::InvalidConfiguration("coc7_ruleset_id"))
    }
}

pub fn validate_coc7_event_contract(event_type: &'static str) -> KernelResult<()> {
    CanonicalEventHeader::resolve(event_type)
        .map(|_| ())
        .map_err(|error| match error.code {
            WireErrorCode::EventContractUnknown => TrpgError::EventContractUnknown,
            WireErrorCode::EventContractVersionMismatch => TrpgError::EventContractVersionMismatch,
            _ => TrpgError::EventContractUnknown,
        })
}

pub(crate) fn append_coc7_event<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    event_type: &'static str,
    decision_type: &'static str,
    summary: impl Into<String>,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    contract.validate_command(command)?;
    let payload = Coc7EventPayload::from_command(command, event_type, decision_type, summary)?;
    store.append(command, event_type, payload)
}
