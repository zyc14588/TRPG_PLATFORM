use trpg_security_governance::{
    evaluate_derived_visibility, DerivationRequest, DerivedObject, RedactionOutcome,
};
use trpg_shared_kernel::{
    EntityId, PrincipalCapability, PrincipalClaims, PrincipalScope, Visibility, VisibilityLabel,
};

fn all_visibilities() -> Vec<Visibility> {
    vec![
        Visibility::new(VisibilityLabel::Public),
        Visibility::new(VisibilityLabel::PartyVisible),
        Visibility::private_to_player(EntityId::new("player_a").unwrap()),
        Visibility::private_to_group(EntityId::new("group_a").unwrap()),
        Visibility::new(VisibilityLabel::KeeperOnly),
        Visibility::investigator_private(EntityId::new("player_a").unwrap()),
        Visibility::new(VisibilityLabel::AiInternal),
        Visibility::new(VisibilityLabel::SystemOnly),
        Visibility::new(VisibilityLabel::SpectatorVisible),
        Visibility::new(VisibilityLabel::SpectatorHidden),
        Visibility::new(VisibilityLabel::SystemPrivate),
    ]
}

fn all_principals() -> Vec<PrincipalScope> {
    let composite = PrincipalClaims::new("user_a")
        .unwrap()
        .with_player("player_a")
        .unwrap()
        .with_group("group_a")
        .unwrap()
        .with_capability(PrincipalCapability::PartyMember)
        .with_capability(PrincipalCapability::Spectator);
    let agent = PrincipalClaims::new("agent_worker")
        .unwrap()
        .with_capability(PrincipalCapability::AiRuntime);
    vec![
        PrincipalScope::Public,
        PrincipalScope::PartyMember,
        PrincipalScope::Keeper,
        PrincipalScope::Player(EntityId::new("player_a").unwrap()),
        PrincipalScope::Player(EntityId::new("player_b").unwrap()),
        PrincipalScope::GroupMember(EntityId::new("group_a").unwrap()),
        PrincipalScope::GroupMember(EntityId::new("group_b").unwrap()),
        PrincipalScope::Spectator,
        PrincipalScope::Claims(composite),
        PrincipalScope::Claims(agent),
        PrincipalScope::System,
    ]
}

fn all_targets() -> [DerivedObject; 6] {
    [
        DerivedObject::PlayerExport,
        DerivedObject::PartySummary,
        DerivedObject::RagChunk,
        DerivedObject::DebugLog,
        DerivedObject::AgentContext,
        DerivedObject::AuditLog,
    ]
}

#[test]
fn every_source_processor_audience_target_combination_fails_closed() {
    let visibilities = all_visibilities();
    let principals = all_principals();

    for source in &visibilities {
        for processor in &principals {
            for audience in &principals {
                for target in all_targets() {
                    let sources = [source.clone()];
                    let decision = evaluate_derived_visibility(DerivationRequest {
                        sources: &sources,
                        processor,
                        target_audience: audience,
                        target,
                    });

                    assert_eq!(decision.result_visibility, *source);
                    if decision.outcome == RedactionOutcome::Visible {
                        assert!(source.can_view(processor));
                        assert!(source.can_view(audience));
                        assert!(
                            !(target == DerivedObject::DebugLog && source.label().is_restricted())
                        );
                    }
                    if !source.can_view(processor) {
                        assert_eq!(decision.outcome, RedactionOutcome::Omitted);
                        assert_eq!(
                            decision.error_code,
                            Some("DERIVATION_PROCESSOR_NOT_AUTHORIZED")
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn privileged_processor_never_widens_player_facing_output() {
    let player = PrincipalScope::Player(EntityId::new("player_a").unwrap());
    for source in all_visibilities() {
        for target in [
            DerivedObject::PlayerExport,
            DerivedObject::PartySummary,
            DerivedObject::RagChunk,
            DerivedObject::AgentContext,
        ] {
            let sources = [source.clone()];
            let decision = evaluate_derived_visibility(DerivationRequest {
                sources: &sources,
                processor: &PrincipalScope::System,
                target_audience: &player,
                target,
            });
            if !source.can_view(&player) {
                assert_ne!(decision.outcome, RedactionOutcome::Visible);
                assert!(decision.error_code.is_some());
            }
            assert_eq!(decision.result_visibility, source);
        }
    }
}

#[test]
fn derived_label_is_an_audience_intersection_not_a_rank_guess() {
    let visibilities = all_visibilities();
    let principals = all_principals();

    for left in &visibilities {
        for right in &visibilities {
            let result = left.intersection(right);
            for principal in &principals {
                if result.can_view(principal) {
                    assert!(
                        left.can_view(principal) && right.can_view(principal),
                        "{} intersect {} widened access for {principal:?}",
                        left.label().as_str(),
                        right.label().as_str()
                    );
                }
            }

            let sources = [left.clone(), right.clone()];
            let decision = evaluate_derived_visibility(DerivationRequest {
                sources: &sources,
                processor: &PrincipalScope::System,
                target_audience: &PrincipalScope::System,
                target: DerivedObject::AuditLog,
            });
            assert_eq!(decision.outcome, RedactionOutcome::Visible);
            assert_eq!(decision.result_visibility, result);
        }
    }
}

#[test]
fn missing_sources_do_not_create_a_public_derived_value() {
    let decision = evaluate_derived_visibility(DerivationRequest {
        sources: &[],
        processor: &PrincipalScope::System,
        target_audience: &PrincipalScope::Public,
        target: DerivedObject::PlayerExport,
    });
    assert_eq!(decision.outcome, RedactionOutcome::Redacted);
    assert_eq!(decision.error_code, Some("DERIVATION_SOURCE_REQUIRED"));
}
