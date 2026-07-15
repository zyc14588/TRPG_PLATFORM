use trpg_domain_core::authority_contract_impl::{
    append_authority_contract_decision, fork_locked_authority_contract,
};
use trpg_domain_core::ddd::{
    Actor, ActorRole, AgentClass, AuthorityMode, DomainError, EntityId, EventStore, FactProvenance,
    PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};
use trpg_domain_core::decision_record_model::{
    DecisionEvidenceCatalog, DecisionKind, DecisionRecord, DecisionRecordDraft,
};

fn decision_and_evidence(
    contract: &trpg_shared_kernel::AuthorityContract,
    visibility: Visibility,
) -> (DecisionRecord, DecisionEvidenceCatalog) {
    let decided_by = match contract.mode() {
        AuthorityMode::HumanKp => Actor::authenticated_user(
            contract.authority_owner().as_str(),
            ActorRole::HumanKeeper,
            "session_keeper",
        )
        .unwrap(),
        AuthorityMode::AiKp => Actor::verified_agent_run(
            contract.authority_owner().as_str(),
            "agent_run_001",
            AgentClass::AiKeeperOrchestrator,
            contract.campaign_id().as_str(),
        )
        .unwrap(),
    };
    let provenance = FactProvenance::new(
        if contract.mode() == &AuthorityMode::HumanKp {
            ProvenanceKind::HumanKeeperStatement
        } else {
            ProvenanceKind::ToolResult
        },
        "event_001",
        contract.authority_owner().as_str(),
    )
    .unwrap();
    let context_hash = format!("sha256:{}", "a".repeat(64));
    let record = DecisionRecord::from_draft(DecisionRecordDraft {
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
    (record, evidence)
}

#[test]
fn authority_contract_impl_rejects_authority_violation_without_event() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_001",
        AuthorityMode::AiKp,
        "ai_kp",
        1,
    )
    .unwrap();
    let (record, evidence) =
        decision_and_evidence(&contract, Visibility::new(VisibilityLabel::Public));
    let mut command =
        trpg_test_support::governed_command_for_contract(&contract, record, ActorRole::HumanKeeper);
    command.visibility = Visibility::new(VisibilityLabel::Public);
    command.fact_provenance = command.payload.fact_provenance().clone();
    let mut store = EventStore::default();

    let error =
        append_authority_contract_decision(&contract, &evidence, &mut store, &command).unwrap_err();

    assert_eq!(error, DomainError::AuthorityViolation);
    assert!(store.events().is_empty());
}

#[test]
fn authority_contract_impl_preserves_visibility_and_provenance_on_replay() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "campaign_001",
        AuthorityMode::HumanKp,
        "keeper",
        1,
    )
    .unwrap();
    let visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let (record, evidence) = decision_and_evidence(&contract, visibility.clone());
    let provenance = record.fact_provenance().clone();
    let mut command =
        trpg_test_support::governed_command_for_contract(&contract, record, ActorRole::HumanKeeper);
    command.visibility = visibility;
    command.fact_provenance = provenance.clone();
    let mut store = EventStore::default();

    let event =
        append_authority_contract_decision(&contract, &evidence, &mut store, &command).unwrap();
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
