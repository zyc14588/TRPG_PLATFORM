use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::ddd::{
    Actor, ActorRole, AuthenticatedCommandContext, AuthorityBinding, AuthorityMode,
    CommandEnvelope, CommandMetadata, EntityId, FactProvenance, FormalWritePath, ProvenanceKind,
    ResourceRef, TrpgError, Visibility, VisibilityLabel,
};
use trpg_shared_kernel::AuthorityRegistry;

fn command_with_binding(
    contract: &DomainAuthorityContract,
    campaign_id: &str,
    authority_owner: &str,
    contract_version: u64,
    actor: Actor,
) -> CommandEnvelope<&'static str> {
    let context = AuthenticatedCommandContext::new(
        actor,
        ResourceRef::new(campaign_id, "campaign", campaign_id).unwrap(),
        AuthorityBinding::new(
            contract.contract_id().as_str(),
            authority_owner,
            contract_version,
        )
        .unwrap(),
        "trace_authority_test",
        1,
        10_000,
    )
    .unwrap();
    CommandEnvelope::new(
        "write",
        CommandMetadata {
            command_id: EntityId::new("command_authority_test").unwrap(),
            idempotency_key: "authority-test".to_owned(),
            expected_version: 0,
            authority_mode: contract.mode().clone(),
            visibility: Visibility::new(VisibilityLabel::SystemOnly),
            fact_provenance: FactProvenance::new(
                ProvenanceKind::HumanKeeperStatement,
                "event_authority_test",
                contract.authority_owner().as_str(),
            )
            .unwrap(),
            correlation_id: EntityId::new("correlation_authority_test").unwrap(),
            causation_id: EntityId::new("causation_authority_test").unwrap(),
            write_path: FormalWritePath::WorkflowDecision,
            authenticated_context: context,
        },
    )
}

#[test]
fn authority_rejects_cross_campaign_wrong_owner_and_stale_version() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_a",
        AuthorityMode::HumanKp,
        "keeper_a",
        7,
    )
    .unwrap();

    let owner =
        Actor::authenticated_user("keeper_a", ActorRole::HumanKeeper, "session_owner").unwrap();
    let cross_campaign = command_with_binding(&contract, "campaign_b", "keeper_a", 7, owner);
    let error = contract.validate_command(&cross_campaign).unwrap_err();
    assert_eq!(error, TrpgError::CampaignScopeMismatch);
    assert_eq!(error.http_status(), 403);

    let workflow = Actor::verified_workload(
        "workflow_authority_test",
        trpg_domain_core::ddd::WorkloadRole::WorkflowEngine,
    )
    .unwrap();
    let wrong_binding = command_with_binding(&contract, "campaign_a", "keeper_b", 7, workflow);
    let error = contract.validate_command(&wrong_binding).unwrap_err();
    assert_eq!(error, TrpgError::AuthorityOwnerMismatch);
    assert_eq!(error.http_status(), 403);

    let workflow = Actor::verified_workload(
        "workflow_authority_test",
        trpg_domain_core::ddd::WorkloadRole::WorkflowEngine,
    )
    .unwrap();
    let stale = command_with_binding(&contract, "campaign_a", "keeper_a", 6, workflow);
    let error = contract.validate_command(&stale).unwrap_err();
    assert_eq!(error, TrpgError::AuthorityContractVersionConflict);
    assert_eq!(error.http_status(), 409);
}

#[test]
fn authority_rejects_non_owner_and_in_place_mode_change() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_a",
        AuthorityMode::HumanKp,
        "keeper_a",
        1,
    )
    .unwrap();
    let attacker =
        Actor::authenticated_user("keeper_b", ActorRole::HumanKeeper, "session_attacker").unwrap();
    let command = command_with_binding(&contract, "campaign_a", "keeper_a", 1, attacker);
    assert_eq!(
        contract.validate_command(&command).unwrap_err(),
        TrpgError::AuthorityOwnerMismatch
    );
    assert_eq!(
        contract.fork(AuthorityMode::AiKp, 2).unwrap_err(),
        TrpgError::AuthorityContractMutation
    );
    assert_eq!(contract.mode(), &AuthorityMode::HumanKp);
    assert_eq!(contract.version(), 1);
}

#[test]
fn canonical_registry_rejects_caller_supplied_conflicting_authority() {
    let canonical = trpg_test_support::authority_contract_with_owner(
        "campaign_a",
        AuthorityMode::HumanKp,
        "keeper_a",
        1,
    )
    .unwrap();
    let forged = trpg_test_support::authority_contract_with_owner(
        "campaign_a",
        AuthorityMode::HumanKp,
        "keeper_attacker",
        1,
    )
    .unwrap();
    let workflow = Actor::verified_workload(
        "workflow_authority_test",
        trpg_domain_core::ddd::WorkloadRole::WorkflowEngine,
    )
    .unwrap();
    let forged_command =
        command_with_binding(&forged, "campaign_a", "keeper_attacker", 1, workflow);
    let mut registry = AuthorityRegistry::from_contracts([canonical]).unwrap();

    assert_eq!(
        registry.validate_command(&forged_command).unwrap_err(),
        TrpgError::AuthorityOwnerMismatch
    );
    assert_eq!(
        registry.register(forged).unwrap_err(),
        TrpgError::AuthorityContractMutation
    );
}
