use trpg_domain_core::ddd::{
    Actor, ActorRole, AuthorityMode, DomainError, EventStore, FactProvenance, ProvenanceKind,
    Visibility, VisibilityLabel,
};
use trpg_domain_core::decision_record_model::{
    DecisionEvidenceCatalog, DecisionKind, DecisionRecord, DecisionRecordDraft,
};

fn draft() -> DecisionRecordDraft {
    let actor =
        Actor::authenticated_user("keeper", ActorRole::HumanKeeper, "session_keeper").unwrap();
    DecisionRecordDraft {
        decision_id: "decision_001".to_owned(),
        campaign_id: "camp_human_archive".to_owned(),
        authority_contract_id: "authority_contract_camp_human_archive_1".to_owned(),
        authority_contract_version: 1,
        authority_owner: "keeper".to_owned(),
        decided_by: actor,
        authority_mode: AuthorityMode::HumanKp,
        kind: DecisionKind::Ruling,
        rules_reference: "COC7-spot-hidden".to_owned(),
        source_context_hash: format!("sha256:{}", "a".repeat(64)),
        linked_event_ids: vec!["event_001".to_owned()],
        linked_tool_result_ids: Vec::new(),
        visibility: Visibility::new(VisibilityLabel::Public),
        fact_provenance: FactProvenance::new(
            ProvenanceKind::HumanKeeperStatement,
            "event_001",
            "keeper",
        )
        .unwrap(),
    }
}

fn event_evidence(
    contract: &trpg_shared_kernel::AuthorityContract,
    context_hash: String,
    visibility: Visibility,
    provenance: FactProvenance,
) -> DecisionEvidenceCatalog {
    let mut command =
        trpg_test_support::governed_command_for_contract(contract, (), ActorRole::HumanKeeper);
    command.visibility = visibility;
    command.fact_provenance = provenance;
    let mut store = EventStore::default();
    let event = store.append(&command, "GameEvent", ()).unwrap();
    let mut evidence = DecisionEvidenceCatalog::default();
    evidence.register_event(&event, context_hash).unwrap();
    evidence
}

#[test]
fn decision_record_requires_hash_link_and_valid_provenance() {
    let mut missing_hash = draft();
    missing_hash.source_context_hash.clear();
    let error = DecisionRecord::from_draft(missing_hash).unwrap_err();

    assert_eq!(error, DomainError::MissingCommandMetadata);

    let mut no_link = draft();
    no_link.linked_event_ids.clear();
    assert_eq!(
        DecisionRecord::from_draft(no_link).unwrap_err(),
        DomainError::MissingCommandMetadata
    );

    let mut imported = draft();
    imported.fact_provenance =
        FactProvenance::new(ProvenanceKind::ImportedSource, "event_001", "keeper").unwrap();
    assert_eq!(
        DecisionRecord::from_draft(imported).unwrap_err(),
        DomainError::InvalidConfirmedFactSource
    );

    let record = DecisionRecord::from_draft(draft()).unwrap();
    assert!(record.source_context_hash().starts_with("sha256:"));
    let contract = trpg_test_support::authority_contract_with_owner(
        "camp_human_archive",
        AuthorityMode::HumanKp,
        "keeper",
        1,
    )
    .unwrap();
    record.validate_against(&contract).unwrap();
}

#[test]
fn syntactically_valid_but_unresolvable_evidence_is_rejected() {
    let record = DecisionRecord::from_draft(draft()).unwrap();
    let contract = trpg_test_support::authority_contract_with_owner(
        "camp_human_archive",
        AuthorityMode::HumanKp,
        "keeper",
        1,
    )
    .unwrap();

    assert_eq!(
        record
            .validate_against_evidence(&contract, &DecisionEvidenceCatalog::default())
            .unwrap_err(),
        DomainError::InvalidConfirmedFactSource
    );
}

#[test]
fn decision_record_resolves_every_link_against_matching_canonical_evidence() {
    let record = DecisionRecord::from_draft(draft()).unwrap();
    let contract = trpg_test_support::authority_contract_with_owner(
        "camp_human_archive",
        AuthorityMode::HumanKp,
        "keeper",
        1,
    )
    .unwrap();
    let evidence = event_evidence(
        &contract,
        format!("sha256:{}", "a".repeat(64)),
        Visibility::new(VisibilityLabel::Public),
        FactProvenance::new(ProvenanceKind::HumanKeeperStatement, "event_001", "keeper").unwrap(),
    );

    record
        .validate_against_evidence(&contract, &evidence)
        .unwrap();
}

#[test]
fn context_or_visibility_mismatch_cannot_be_laundered_into_a_decision_record() {
    let record = DecisionRecord::from_draft(draft()).unwrap();
    let contract = trpg_test_support::authority_contract_with_owner(
        "camp_human_archive",
        AuthorityMode::HumanKp,
        "keeper",
        1,
    )
    .unwrap();
    let evidence = event_evidence(
        &contract,
        format!("sha256:{}", "b".repeat(64)),
        Visibility::new(VisibilityLabel::KeeperOnly),
        FactProvenance::new(ProvenanceKind::HumanKeeperStatement, "event_001", "keeper").unwrap(),
    );

    assert_eq!(
        record
            .validate_against_evidence(&contract, &evidence)
            .unwrap_err(),
        DomainError::InvalidConfirmedFactSource
    );
}

#[test]
fn command_acceptance_is_not_decision_evidence() {
    let contract = trpg_test_support::authority_contract_with_owner(
        "camp_human_archive",
        AuthorityMode::HumanKp,
        "keeper",
        1,
    )
    .unwrap();
    let mut command =
        trpg_test_support::governed_command_for_contract(&contract, (), ActorRole::HumanKeeper);
    command.fact_provenance =
        FactProvenance::new(ProvenanceKind::HumanKeeperStatement, "event_001", "keeper").unwrap();
    let mut store = EventStore::default();
    let event = store.append(&command, "CommandAccepted", ()).unwrap();
    let mut evidence = DecisionEvidenceCatalog::default();

    assert_eq!(
        evidence
            .register_event(&event, format!("sha256:{}", "a".repeat(64)))
            .unwrap_err(),
        DomainError::InvalidConfirmedFactSource
    );
}
