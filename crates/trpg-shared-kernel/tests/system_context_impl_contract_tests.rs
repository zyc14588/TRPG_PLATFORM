use trpg_shared_kernel::shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, EventStore, TrpgError,
};
use trpg_shared_kernel::system_context::{
    current_system_context_policy, ContextPropagationChannel,
};
use trpg_shared_kernel::system_context_impl::{
    append_system_context_impl_reviewed, system_context_impl_review, system_context_landing,
    validate_system_context_landing,
};

#[test]
fn system_context_impl_accepts_full_visibility_and_provenance_policy() {
    let landing = system_context_landing(current_system_context_policy());

    validate_system_context_landing(&landing).unwrap();
    assert_eq!(
        landing.governance_contract.module_name,
        "system_context_impl"
    );
}

#[test]
fn system_context_impl_rejects_direct_model_provider_access() {
    let mut policy = current_system_context_policy();
    policy.direct_model_provider_access = true;
    let landing = system_context_landing(policy);

    assert_eq!(
        validate_system_context_landing(&landing).unwrap_err(),
        TrpgError::PolicyDenied
    );
}

#[test]
fn system_context_impl_rejects_missing_metric_channel() {
    let mut policy = current_system_context_policy();
    policy
        .channels
        .retain(|channel| *channel != ContextPropagationChannel::Metric);
    let landing = system_context_landing(policy);

    assert_eq!(
        validate_system_context_landing(&landing).unwrap_err(),
        TrpgError::MissingFactProvenance
    );
}

#[test]
fn system_context_impl_review_is_recorded_as_a_governed_event() {
    let command = CommandEnvelope::governed(
        system_context_impl_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_system_context_impl_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.payload.module_name, "system_context_impl");
    assert_eq!(event.payload.reviewed_requirements, 3);
    assert_eq!(event.idempotency_key, command.idempotency_key);
}
