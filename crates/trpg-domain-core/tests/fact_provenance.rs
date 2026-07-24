use trpg_domain_core::command_cqrs::{CommandAcceptedPayload, DomainCommandKind};
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, DomainError, EntityId, EventStore, FactProvenance, FactSource,
    ProvenanceKind, Visibility, VisibilityLabel,
};
use trpg_domain_core::domain_entities_value_objects::MemoryFact;
#[cfg(feature = "canonical-store-internal")]
use trpg_domain_core::visibility_fact_provenance::PersistedFactEvidenceRecord;
use trpg_domain_core::visibility_fact_provenance::{
    promote_fact_to_confirmed, CommittedFactEvidence,
};

fn committed_event(
    source: FactSource,
    kind: DomainCommandKind,
    provenance_kind: ProvenanceKind,
    target_fact_id: &str,
) -> (EventStore<CommandAcceptedPayload>, u64) {
    let mut command = trpg_test_support::governed_command(
        "formal fact",
        ActorRole::RulesEngine,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::PartyVisible);
    command.fact_provenance =
        FactProvenance::new(provenance_kind, "source_record_001", "rules_engine_001").unwrap();
    let mut store = EventStore::default();
    let event = store
        .append(
            &command,
            trpg_domain_core::visibility_fact_provenance::expected_source_event_type(source),
            CommandAcceptedPayload {
                kind,
                fact_source: source,
                target_fact_id: target_fact_id.to_owned(),
            },
        )
        .unwrap();
    (store, event.sequence)
}

#[test]
fn confirmed_fact_requires_a_present_integrity_checked_formal_event() {
    let (store, sequence) = committed_event(
        FactSource::DecisionRecord,
        DomainCommandKind::RecordDecision,
        ProvenanceKind::RulesEngineDecision,
        "fact_decision_001",
    );
    let evidence = CommittedFactEvidence::load(&store, sequence, "fact_decision_001").unwrap();

    let fact = promote_fact_to_confirmed("fact_decision_001", &evidence).unwrap();
    assert_eq!(fact.source(), FactSource::DecisionRecord);
    assert_eq!(fact.source_event_sequence(), sequence);
    assert_eq!(
        fact.fact_provenance().kind,
        ProvenanceKind::RulesEngineDecision
    );

    assert_eq!(
        MemoryFact::confirmed("different_decision_fact", &evidence).unwrap_err(),
        DomainError::CommittedFactEvidenceInvalid
    );
    let memory = MemoryFact::confirmed("fact_decision_001", &evidence).unwrap();
    assert!(memory.is_confirmed());
    assert_eq!(memory.source_event_sequence(), sequence);
    assert_eq!(
        memory.visibility(),
        &Visibility::new(VisibilityLabel::PartyVisible)
    );

    assert_eq!(
        CommittedFactEvidence::load(&store, sequence + 1, "missing_fact").unwrap_err(),
        DomainError::CommittedFactEvidenceMissing
    );
}

#[test]
fn agent_proposal_and_nonformal_sources_cannot_be_wrapped_as_confirmed() {
    for (source, provenance) in [
        (FactSource::DecisionRecord, ProvenanceKind::AgentProposal),
        (FactSource::AgentDraft, ProvenanceKind::RulesEngineDecision),
        (FactSource::NpcClaim, ProvenanceKind::HumanKeeperStatement),
        (FactSource::PlayerInference, ProvenanceKind::ToolResult),
        (FactSource::GameEvent, ProvenanceKind::ImportedSource),
        (FactSource::GameEvent, ProvenanceKind::SystemFixture),
    ] {
        let (store, sequence) = committed_event(
            source,
            DomainCommandKind::PromoteFact,
            provenance,
            "rejected_fact",
        );
        assert_eq!(
            CommittedFactEvidence::load(&store, sequence, "rejected_fact").unwrap_err(),
            DomainError::CommittedFactEvidenceInvalid
        );
    }
}

#[test]
fn source_kind_and_workflow_event_must_be_consistent() {
    let (wrong_kind_store, wrong_kind_sequence) = committed_event(
        FactSource::GameEvent,
        DomainCommandKind::SubmitPlayerAction,
        ProvenanceKind::RulesEngineDecision,
        "wrong_kind",
    );
    assert_eq!(
        CommittedFactEvidence::load(&wrong_kind_store, wrong_kind_sequence, "wrong_kind")
            .unwrap_err(),
        DomainError::CommittedFactEvidenceInvalid
    );

    let (wrong_provenance_store, wrong_provenance_sequence) = committed_event(
        FactSource::DiceRoll,
        DomainCommandKind::RecordDecision,
        ProvenanceKind::HumanKeeperStatement,
        "wrong_provenance",
    );
    assert_eq!(
        CommittedFactEvidence::load(
            &wrong_provenance_store,
            wrong_provenance_sequence,
            "wrong_provenance"
        )
        .unwrap_err(),
        DomainError::CommittedFactEvidenceInvalid
    );
}

#[test]
fn generic_command_payload_cannot_self_attest_a_server_dice_roll() {
    let (store, sequence) = committed_event(
        FactSource::DiceRoll,
        DomainCommandKind::RecordDecision,
        ProvenanceKind::RulesEngineDecision,
        "forged_dice_fact",
    );
    assert_eq!(
        CommittedFactEvidence::load(&store, sequence, "forged_dice_fact").unwrap_err(),
        DomainError::CommittedFactEvidenceInvalid
    );
}

#[test]
fn evidence_target_is_bound_to_the_recorded_payload() {
    let (store, sequence) = committed_event(
        FactSource::DecisionRecord,
        DomainCommandKind::RecordDecision,
        ProvenanceKind::RulesEngineDecision,
        "recorded_fact",
    );
    assert_eq!(
        CommittedFactEvidence::load(&store, sequence, "substituted_fact").unwrap_err(),
        DomainError::CommittedFactEvidenceInvalid
    );
}

#[test]
fn evidence_preserves_the_committed_targeted_visibility() {
    let mut command = trpg_test_support::governed_command(
        "private clue",
        ActorRole::RulesEngine,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::private_to_player(EntityId::new("player_a").unwrap());
    command.fact_provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        "clue_event_001",
        "rules_engine_001",
    )
    .unwrap();
    let mut store = EventStore::default();
    let event = store
        .append(
            &command,
            "ClueRevealed",
            CommandAcceptedPayload {
                kind: DomainCommandKind::RecordDecision,
                fact_source: FactSource::ClueRevealEvent,
                target_fact_id: "private_clue".to_owned(),
            },
        )
        .unwrap();

    let evidence = CommittedFactEvidence::load(&store, event.sequence, "private_clue").unwrap();
    let fact = promote_fact_to_confirmed("private_clue", &evidence).unwrap();
    assert_eq!(fact.visibility().subject_id().unwrap().as_str(), "player_a");
    assert_eq!(fact.source(), FactSource::ClueRevealEvent);
}

#[test]
#[cfg(feature = "canonical-store-internal")]
fn persisted_evidence_requires_the_canonical_integrity_key() {
    let key = [0x41; 32];
    let record = PersistedFactEvidenceRecord::seal_verified(
        "persisted_decision_fact",
        "campaign_persisted_fact",
        91,
        "campaign_fact_stream",
        7,
        "DecisionCommitted",
        DomainCommandKind::RecordDecision,
        FactSource::DecisionRecord,
        Visibility::new(VisibilityLabel::PartyVisible),
        FactProvenance::new(
            ProvenanceKind::RulesEngineDecision,
            "decision_record_91",
            "rules_engine_001",
        )
        .unwrap(),
        format!("hmac-sha256:{}", "a".repeat(64)),
        format!("sha256:{}", "b".repeat(64)),
        &key,
    )
    .unwrap();

    let evidence = CommittedFactEvidence::load_persisted(&record, &key).unwrap();
    assert_eq!(
        evidence.target_fact_id().as_str(),
        "persisted_decision_fact"
    );
    assert_eq!(evidence.campaign_id().as_str(), "campaign_persisted_fact");
    assert_eq!(evidence.event_sequence(), 91);
    assert_eq!(evidence.source(), FactSource::DecisionRecord);
    assert_eq!(
        CommittedFactEvidence::load_persisted(&record, &[0x42; 32]).unwrap_err(),
        DomainError::CommittedFactEvidenceInvalid
    );
}

#[test]
#[cfg(feature = "canonical-store-internal")]
fn a_sealed_agent_proposal_still_cannot_become_confirmed() {
    let key = [0x51; 32];
    let record = PersistedFactEvidenceRecord::seal_verified(
        "rejected_agent_fact",
        "campaign_persisted_fact",
        92,
        "campaign_fact_stream",
        8,
        "DecisionCommitted",
        DomainCommandKind::RecordDecision,
        FactSource::DecisionRecord,
        Visibility::new(VisibilityLabel::KeeperOnly),
        FactProvenance::new(
            ProvenanceKind::AgentProposal,
            "agent_draft_92",
            "agent_runtime_001",
        )
        .unwrap(),
        format!("hmac-sha256:{}", "c".repeat(64)),
        format!("sha256:{}", "d".repeat(64)),
        &key,
    )
    .unwrap();

    assert_eq!(
        CommittedFactEvidence::load_persisted(&record, &key).unwrap_err(),
        DomainError::InvalidConfirmedFactSource
    );
}
