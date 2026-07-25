mod common;

use trpg_domain_core::ddd::{
    DomainError, EntityId, FactSource, PrincipalScope, ProvenanceKind, Visibility, VisibilityLabel,
};
use trpg_domain_core::visibility_fact_provenance::{
    most_restrictive_label, most_restrictive_visibility, promote_fact_to_confirmed, redaction_for,
    DerivedObject, RedactionOutcome,
};

#[test]
fn visibility_fact_provenance_redacts_restricted_content() {
    assert_eq!(
        most_restrictive_label(&[VisibilityLabel::Public, VisibilityLabel::KeeperOnly]),
        Some(VisibilityLabel::KeeperOnly)
    );

    assert_eq!(
        redaction_for(
            &Visibility::new(VisibilityLabel::KeeperOnly),
            DerivedObject::AgentContextForPlayer,
            &PrincipalScope::System,
            &PrincipalScope::Player(EntityId::new("user_player_a").unwrap())
        ),
        RedactionOutcome::Omitted
    );

    assert_eq!(
        redaction_for(
            &Visibility::new(VisibilityLabel::AiInternal),
            DerivedObject::AnyPlayerOrKeeperExport,
            &PrincipalScope::System,
            &PrincipalScope::Keeper
        ),
        RedactionOutcome::RedactedOrAuditOnly
    );

    assert_eq!(
        redaction_for(
            &Visibility::new(VisibilityLabel::KeeperOnly),
            DerivedObject::PlayerExport,
            &PrincipalScope::Keeper,
            &PrincipalScope::Public,
        ),
        RedactionOutcome::Redacted,
        "processor authority must never be reused as target-audience authority"
    );
}

#[test]
fn visibility_fact_provenance_orders_every_current_audience_without_downgrade() {
    let ordered = [
        Visibility::new(VisibilityLabel::Public),
        Visibility::new(VisibilityLabel::SpectatorVisible),
        Visibility::new(VisibilityLabel::PartyVisible),
        Visibility::new(VisibilityLabel::SpectatorHidden),
        Visibility::private_to_player(EntityId::new("player_a").unwrap()),
        Visibility::private_to_group(EntityId::new("group_a").unwrap()),
        Visibility::investigator_private(EntityId::new("player_a").unwrap()),
        Visibility::new(VisibilityLabel::KeeperOnly),
        Visibility::new(VisibilityLabel::AiInternal),
        Visibility::new(VisibilityLabel::SystemOnly),
        Visibility::new(VisibilityLabel::SystemPrivate),
    ];

    for expected in ordered {
        assert_eq!(
            most_restrictive_label(&[VisibilityLabel::Public, expected.label().clone()]),
            Some(expected.label().clone())
        );
    }
    let private_group = Visibility::private_to_group(EntityId::new("group_a").unwrap());
    assert_eq!(
        most_restrictive_label(&[
            VisibilityLabel::SpectatorVisible,
            VisibilityLabel::PartyVisible,
            private_group.label().clone(),
        ]),
        Some(private_group.label().clone())
    );

    for (left, right) in [
        (
            Visibility::private_to_player(EntityId::new("player_a").unwrap()),
            Visibility::private_to_group(EntityId::new("group_a").unwrap()),
        ),
        (
            Visibility::private_to_player(EntityId::new("player_a").unwrap()),
            Visibility::investigator_private(EntityId::new("player_b").unwrap()),
        ),
        (
            Visibility::private_to_group(EntityId::new("group_a").unwrap()),
            Visibility::investigator_private(EntityId::new("player_a").unwrap()),
        ),
    ] {
        assert_eq!(
            most_restrictive_label(&[left.label().clone(), right.label().clone()]),
            Some(VisibilityLabel::KeeperOnly)
        );
        assert_eq!(
            most_restrictive_label(&[right.label().clone(), left.label().clone()]),
            Some(VisibilityLabel::KeeperOnly)
        );
    }
}

#[test]
fn visibility_derivation_preserves_targets_and_closes_incomparable_scopes() {
    let player_a = EntityId::new("player_a").unwrap();
    let player_b = EntityId::new("player_b").unwrap();
    let group_a = EntityId::new("group_a").unwrap();
    let private_player = Visibility::private_to_player(player_a.clone());
    let investigator_alias = Visibility::investigator_private(player_a.clone());

    let merged = most_restrictive_visibility(&[
        Visibility::new(VisibilityLabel::Public),
        private_player.clone(),
        investigator_alias,
    ])
    .unwrap()
    .unwrap();
    assert_eq!(merged.label().as_str(), "investigator_private");
    assert_eq!(merged.subject_id(), Some(&player_a));

    for incompatible in [
        Visibility::private_to_player(player_b),
        Visibility::private_to_group(group_a),
    ] {
        let forward = most_restrictive_visibility(&[private_player.clone(), incompatible.clone()])
            .unwrap()
            .unwrap();
        let reverse = most_restrictive_visibility(&[incompatible, private_player.clone()])
            .unwrap()
            .unwrap();
        assert_eq!(forward, Visibility::new(VisibilityLabel::KeeperOnly));
        assert_eq!(reverse, Visibility::new(VisibilityLabel::KeeperOnly));
    }
}

#[test]
fn visibility_fact_provenance_blocks_untrusted_confirmed_fact_sources() {
    assert_eq!(
        common::committed_fact_evidence(
            FactSource::NpcClaim,
            ProvenanceKind::RulesEngineDecision,
            "rejected_npc_claim",
        )
        .unwrap_err(),
        DomainError::InvalidConfirmedFactSource
    );

    let evidence = common::committed_fact_evidence(
        FactSource::DecisionRecord,
        ProvenanceKind::RulesEngineDecision,
        "fact_decision",
    )
    .unwrap();
    let confirmed = promote_fact_to_confirmed("fact_decision", &evidence).unwrap();
    assert_eq!(confirmed.source(), FactSource::DecisionRecord);
}
