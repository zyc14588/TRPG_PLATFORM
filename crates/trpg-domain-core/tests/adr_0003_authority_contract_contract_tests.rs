use trpg_domain_core::adr_0003_authority_contract::{
    fork_locked_authority_contract, record_authority_contract_decision,
    reject_authority_contract_update, validate_adr_0003_contract, ADR_0003_INVARIANTS,
};
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, CommandEnvelope, DomainError, EntityId, EventStore, FactProvenance,
    FormalWritePath, PrincipalScope, ProvenanceKind, TrpgError, Visibility, VisibilityLabel,
};
use trpg_domain_core::decision_record_model::{
    DecisionEvidenceCatalog, DecisionKind, DecisionRecord, DecisionRecordDraft,
};

fn decision_command(
    contract: &trpg_shared_kernel::AuthorityContract,
    command_actor_role: ActorRole,
    visibility: Visibility,
) -> (CommandEnvelope<DecisionRecord>, DecisionEvidenceCatalog) {
    let provenance_kind = if contract.mode() == &AuthorityMode::HumanKp {
        ProvenanceKind::HumanKeeperStatement
    } else {
        ProvenanceKind::ToolResult
    };
    let provenance = FactProvenance::new(
        provenance_kind,
        "event_001",
        contract.authority_owner().as_str(),
    )
    .unwrap();
    let context_hash = format!("sha256:{}", "a".repeat(64));
    let decided_by = trpg_test_support::actor_for_role(
        if contract.mode() == &AuthorityMode::HumanKp {
            ActorRole::HumanKeeper
        } else {
            ActorRole::AiKeeper
        },
        contract.campaign_id().as_str(),
        contract.authority_owner().as_str(),
    );
    let decision = DecisionRecord::from_draft(DecisionRecordDraft {
        decision_id: "decision_001".to_owned(),
        campaign_id: contract.campaign_id().to_string(),
        authority_contract_id: contract.contract_id().to_string(),
        authority_contract_version: contract.version(),
        authority_owner: contract.authority_owner().to_string(),
        decided_by,
        authority_mode: contract.mode().clone(),
        kind: DecisionKind::Ruling,
        rules_reference: "COC7-ruling".to_owned(),
        source_context_hash: context_hash.clone(),
        linked_event_ids: vec!["event_001".to_owned()],
        linked_tool_result_ids: Vec::new(),
        visibility: visibility.clone(),
        fact_provenance: provenance.clone(),
    })
    .unwrap();
    let mut evidence_command = trpg_test_support::governed_command_for_contract(
        contract,
        (),
        if contract.mode() == &AuthorityMode::HumanKp {
            ActorRole::HumanKeeper
        } else {
            ActorRole::Workflow
        },
    );
    evidence_command.visibility = visibility.clone();
    evidence_command.fact_provenance = provenance.clone();
    let mut evidence_store = EventStore::default();
    let event = evidence_store
        .append(&evidence_command, "GameEvent", ())
        .unwrap();
    let mut evidence = DecisionEvidenceCatalog::default();
    evidence.register_event(&event, context_hash).unwrap();
    let mut command =
        trpg_test_support::governed_command_for_contract(contract, decision, command_actor_role);
    command.visibility = visibility;
    command.fact_provenance = provenance;
    (command, evidence)
}

#[test]
fn adr_0003_authority_contract_rejects_authority_violation_without_event() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_001",
        AuthorityMode::AiKp,
        "ai_kp",
        1,
    )
    .unwrap();
    let (command, evidence) = decision_command(
        &contract,
        ActorRole::HumanKeeper,
        Visibility::new(VisibilityLabel::Public),
    );
    let mut store = EventStore::default();

    let error =
        record_authority_contract_decision(&contract, &evidence, &mut store, &command).unwrap_err();

    assert_eq!(error, DomainError::AuthorityViolation);
    assert!(store.events().is_empty());
}

#[test]
fn adr_0003_authority_contract_blocks_direct_agent_write_without_event() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_001",
        AuthorityMode::AiKp,
        "ai_kp",
        1,
    )
    .unwrap();
    let (mut command, evidence) = decision_command(
        &contract,
        ActorRole::Workflow,
        Visibility::new(VisibilityLabel::Public),
    );
    command.write_path = FormalWritePath::DirectAgent;
    let mut store = EventStore::default();

    let error =
        record_authority_contract_decision(&contract, &evidence, &mut store, &command).unwrap_err();

    assert_eq!(error, DomainError::PolicyDenied);
    assert!(store.events().is_empty());
}

#[test]
fn adr_0003_authority_contract_keeps_visibility_and_fact_provenance_on_replay() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_001",
        AuthorityMode::HumanKp,
        "keeper",
        1,
    )
    .unwrap();
    let (command, evidence) = decision_command(
        &contract,
        ActorRole::HumanKeeper,
        Visibility::new(VisibilityLabel::KeeperOnly),
    );
    let provenance = command.fact_provenance.clone();
    let mut store = EventStore::default();

    let event =
        record_authority_contract_decision(&contract, &evidence, &mut store, &command).unwrap();
    let child =
        fork_locked_authority_contract(&contract, "campaign_child", AuthorityMode::AiKp, "ai_kp")
            .unwrap();

    assert_eq!(event.fact_provenance, provenance);
    assert_eq!(child.version(), 1);
    assert_eq!(store.replay_visible(&PrincipalScope::Keeper).len(), 1);
    assert!(store
        .replay_visible(&PrincipalScope::Player(
            EntityId::new("player_001").unwrap()
        ))
        .is_empty());
}

#[test]
fn adr_0003_authority_contract_requires_locked_fork_only_policy() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_001",
        AuthorityMode::AiKp,
        "ai_kp",
        1,
    )
    .unwrap();

    assert!(ADR_0003_INVARIANTS.contains(&"change_policy_fork_only"));
    validate_adr_0003_contract(&contract).unwrap();
    assert_eq!(
        reject_authority_contract_update(&contract, AuthorityMode::HumanKp, "keeper").unwrap_err(),
        DomainError::AuthorityContractImmutable
    );

    assert_eq!(
        trpg_test_support::authority_contract_with_owner(
            "campaign_bad",
            AuthorityMode::AiKp,
            "ai_kp",
            0
        )
        .unwrap_err(),
        TrpgError::AuthorityContractMutation
    );
}
