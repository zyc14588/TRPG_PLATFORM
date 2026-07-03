use trpg_shared_kernel::shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, EventStore, TrpgError,
};
use trpg_shared_kernel::system_context::{
    append_system_context_reviewed, current_system_context_policy, system_context_contract,
    system_context_review, validate_system_context_policy, ContextPropagationChannel,
};
use trpg_shared_kernel::workspace_and_governance::{
    validate_governance_contract, GovernanceSurface,
};

#[test]
fn system_context_contract_carries_visibility_and_provenance_channels() {
    let contract = system_context_contract();
    let policy = current_system_context_policy();

    validate_governance_contract(&contract).unwrap();
    validate_system_context_policy(&policy).unwrap();
    assert_eq!(contract.surface, GovernanceSurface::SystemContext);
    assert!(policy.channels.contains(&ContextPropagationChannel::Api));
    assert!(policy.channels.contains(&ContextPropagationChannel::Event));
    assert!(policy
        .channels
        .contains(&ContextPropagationChannel::AgentContext));
    assert!(policy
        .channels
        .contains(&ContextPropagationChannel::ToolResult));
    assert!(policy.channels.contains(&ContextPropagationChannel::Metric));
}

#[test]
fn system_context_contract_rejects_bypass_and_missing_channel() {
    let mut direct_provider = current_system_context_policy();
    direct_provider.direct_model_provider_access = true;
    assert_eq!(
        validate_system_context_policy(&direct_provider).unwrap_err(),
        TrpgError::PolicyDenied
    );

    let mut direct_agent = current_system_context_policy();
    direct_agent.direct_agent_state_write = true;
    assert_eq!(
        validate_system_context_policy(&direct_agent).unwrap_err(),
        TrpgError::DirectAgentStateWrite
    );

    let mut missing_metric = current_system_context_policy();
    missing_metric
        .channels
        .retain(|channel| *channel != ContextPropagationChannel::Metric);
    assert_eq!(
        validate_system_context_policy(&missing_metric).unwrap_err(),
        TrpgError::MissingFactProvenance
    );
}

#[test]
fn system_context_review_is_recorded_as_a_governed_event() {
    let command = CommandEnvelope::governed(
        system_context_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_system_context_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.payload.module_name, "system_context");
    assert_eq!(event.payload.reviewed_requirements, 3);
    assert_eq!(event.idempotency_key, command.idempotency_key);
}
