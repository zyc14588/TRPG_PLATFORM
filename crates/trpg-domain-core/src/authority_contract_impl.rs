use crate::authority_contract::{patch_locked_authority_contract, DomainAuthorityContract};
use crate::command_cqrs::{submit_domain_command, CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{
    AuthorityMode, CommandEnvelope, DomainResult, EventEnvelope, EventStore, FactSource,
};
use crate::decision_record_model::{DecisionEvidenceCatalog, DecisionRecord};

pub fn validate_locked_authority_command<T>(
    contract: &DomainAuthorityContract,
    command: &CommandEnvelope<T>,
) -> DomainResult<()> {
    contract
        .validate_command(command)
        .map_err(crate::ddd::DomainError::from)
}

pub fn reject_authority_contract_update(
    contract: &DomainAuthorityContract,
    attempted_mode: AuthorityMode,
    attempted_owner: impl Into<String>,
) -> DomainResult<()> {
    patch_locked_authority_contract(contract, attempted_mode, attempted_owner)
}

pub fn fork_locked_authority_contract(
    contract: &DomainAuthorityContract,
    child_campaign_id: impl Into<String>,
    child_mode: AuthorityMode,
    child_owner: impl Into<String>,
) -> DomainResult<DomainAuthorityContract> {
    contract
        .fork_for_child(child_campaign_id, child_mode, child_owner)
        .map_err(crate::ddd::DomainError::from)
}

pub fn append_authority_contract_decision(
    contract: &DomainAuthorityContract,
    evidence: &DecisionEvidenceCatalog,
    store: &mut EventStore<CommandAcceptedPayload>,
    command: &CommandEnvelope<DecisionRecord>,
) -> DomainResult<EventEnvelope<CommandAcceptedPayload>> {
    command
        .payload
        .validate_against_evidence(contract, evidence)?;
    if command.visibility != *command.payload.visibility()
        || command.fact_provenance != *command.payload.fact_provenance()
    {
        return Err(crate::ddd::DomainError::InvalidConfirmedFactSource);
    }
    submit_domain_command(
        contract,
        store,
        command,
        DomainCommandKind::RecordDecision,
        FactSource::DecisionRecord,
    )
}
