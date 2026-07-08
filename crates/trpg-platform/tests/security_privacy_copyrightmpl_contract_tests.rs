use trpg_platform::security_privacy_copyrightmpl::{
    review_security_privacy_copyright_policy, ReviewSecurityPrivacyCopyrightPolicy,
    SecurityPrivacyCopyrightEvent, SecurityPrivacyCopyrightRepository,
    SECURITY_PRIVACY_COPYRIGHTMPL_METRIC_MODULE, SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, PrincipalScope, TrpgError, Visibility,
    VisibilityLabel,
};

fn command() -> CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy> {
    CommandEnvelope::governed(
        ReviewSecurityPrivacyCopyrightPolicy {
            asset_id: "handout_001".to_owned(),
            license_tag: "original_campaign_asset".to_owned(),
            detail: "keeper_only_handout_notes".to_owned(),
            contains_restricted_visibility: false,
            export_allowed: true,
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    )
}

#[test]
fn security_privacy_copyrightmpl_rejects_authority_contract_violation() {
    let command = CommandEnvelope::governed(
        command().payload,
        ActorRole::AiKeeper,
        AuthorityMode::HumanKp,
    );
    let mut repository = SecurityPrivacyCopyrightRepository::default();

    let err = review_security_privacy_copyright_policy(&mut repository, &command)
        .expect_err("authority mismatch denied");

    assert_eq!(err, TrpgError::AuthorityViolation);
    assert!(repository.events().is_empty());
}

#[test]
fn security_privacy_copyrightmpl_keeps_visibility_and_fact_provenance_on_replay() {
    let mut command = command();
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let mut repository = SecurityPrivacyCopyrightRepository::default();

    let event = review_security_privacy_copyright_policy(&mut repository, &command)
        .expect("review recorded");

    assert_eq!(event.event_type, SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert!(repository
        .replay_visible(&PrincipalScope::Public)
        .is_empty());
    assert_eq!(repository.replay_visible(&PrincipalScope::System).len(), 1);
    assert!(matches!(
        event.payload,
        SecurityPrivacyCopyrightEvent::SecurityPrivacyCopyrightReviewed { detail, .. }
            if detail == "[redacted]"
    ));
}

#[test]
fn security_privacy_copyrightmpl_rejects_restricted_visibility_export() {
    let mut command = command();
    command.payload.contains_restricted_visibility = true;
    command.payload.export_allowed = true;
    let mut repository = SecurityPrivacyCopyrightRepository::default();

    let err = review_security_privacy_copyright_policy(&mut repository, &command)
        .expect_err("restricted visibility export denied");

    assert_eq!(err, TrpgError::VisibilityDenied);
    assert!(repository.events().is_empty());
    assert_eq!(
        SECURITY_PRIVACY_COPYRIGHTMPL_METRIC_MODULE,
        "security_privacy_copyrightmpl"
    );
}
