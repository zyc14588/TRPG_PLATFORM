use trpg_domain_core::ddd::{
    Actor, ActorRole, AuthorityMode, DomainError, FactProvenance, ProvenanceKind, Visibility,
    VisibilityLabel,
};
use trpg_domain_core::decision_record_model::{DecisionKind, DecisionRecord, DecisionRecordDraft};

#[test]
fn decision_record_requires_rules_reference_and_context_hash() {
    let actor = Actor::new("workflow_001", ActorRole::Workflow).unwrap();
    let provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        "decision_001",
        "rules_001",
    )
    .unwrap();

    let error = DecisionRecord::from_draft(DecisionRecordDraft {
        decision_id: "decision_001".to_owned(),
        campaign_id: "camp_ai_harbor".to_owned(),
        decided_by: actor.clone(),
        authority_mode: AuthorityMode::AiKp,
        kind: DecisionKind::Ruling,
        rules_reference: "COC7-spot-hidden".to_owned(),
        source_context_hash: String::new(),
        visibility: Visibility::new(VisibilityLabel::Public),
        fact_provenance: provenance.clone(),
    })
    .unwrap_err();

    assert_eq!(error, DomainError::MissingCommandMetadata);

    let record = DecisionRecord::from_draft(DecisionRecordDraft {
        decision_id: "decision_001".to_owned(),
        campaign_id: "camp_ai_harbor".to_owned(),
        decided_by: actor,
        authority_mode: AuthorityMode::AiKp,
        kind: DecisionKind::Ruling,
        rules_reference: "COC7-spot-hidden".to_owned(),
        source_context_hash: "ctx_hash_001".to_owned(),
        visibility: Visibility::new(VisibilityLabel::Public),
        fact_provenance: provenance,
    })
    .unwrap();
    assert_eq!(record.source_context_hash, "ctx_hash_001");
}
