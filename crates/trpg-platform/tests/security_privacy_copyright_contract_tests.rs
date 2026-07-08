use trpg_platform::security_privacy_copyright::{
    request_data_deletion, review_security_privacy_copyright_policy, RequestDataDeletion,
    ReviewSecurityPrivacyCopyrightPolicy, SecurityPolicyDecision, SecurityPolicyGate,
    SecurityPrivacyCopyrightEvent, SecurityPrivacyCopyrightRepository,
    DATA_DELETION_REQUESTED_EVENT, SECURITY_PRIVACY_COPYRIGHT_METRIC_MODULE,
    SECURITY_PRIVACY_COPYRIGHT_REQUIRED_METRICS, SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, EntityId, FormalWritePath, PrincipalScope,
    TrpgError, Visibility, VisibilityLabel,
};

fn review_command() -> CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy> {
    CommandEnvelope::governed(
        ReviewSecurityPrivacyCopyrightPolicy {
            asset_id: "handout_001".to_owned(),
            license_tag: "original_campaign_asset".to_owned(),
            detail: "keeper_only_handout_notes".to_owned(),
            contains_restricted_visibility: false,
            export_allowed: true,
            policy_gate: SecurityPolicyGate::permit_all(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    )
}

fn deletion_command() -> CommandEnvelope<RequestDataDeletion> {
    CommandEnvelope::governed(
        RequestDataDeletion {
            subject_id: "player_001".to_owned(),
            retention_policy: "audit_log_retained_private_payload_removed".to_owned(),
            reason: "player privacy request".to_owned(),
            policy_gate: SecurityPolicyGate::permit_all(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    )
}

#[test]
fn security_privacy_copyright_rejects_authority_contract_violation() {
    let command = CommandEnvelope::governed(
        review_command().payload,
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
fn security_privacy_copyright_keeps_visibility_and_fact_provenance_on_replay() {
    let mut command = review_command();
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
fn security_privacy_copyright_fails_closed_on_policy_gate_deny() {
    for policy_gate in [
        SecurityPolicyGate {
            openfga: SecurityPolicyDecision::Deny,
            opa: SecurityPolicyDecision::Permit,
            tool_grant: SecurityPolicyDecision::Permit,
        },
        SecurityPolicyGate {
            openfga: SecurityPolicyDecision::Permit,
            opa: SecurityPolicyDecision::Deny,
            tool_grant: SecurityPolicyDecision::Permit,
        },
        SecurityPolicyGate {
            openfga: SecurityPolicyDecision::Permit,
            opa: SecurityPolicyDecision::Permit,
            tool_grant: SecurityPolicyDecision::Deny,
        },
    ] {
        let mut command = review_command();
        command.payload.policy_gate = policy_gate;
        let mut repository = SecurityPrivacyCopyrightRepository::default();

        let err = review_security_privacy_copyright_policy(&mut repository, &command)
            .expect_err("policy gate denied");

        assert_eq!(err, TrpgError::PolicyDenied);
        assert!(repository.events().is_empty());
    }
}

#[test]
fn security_privacy_copyright_rejects_restricted_visibility_export() {
    let mut command = review_command();
    command.payload.contains_restricted_visibility = true;
    command.payload.export_allowed = true;
    let mut repository = SecurityPrivacyCopyrightRepository::default();

    let err = review_security_privacy_copyright_policy(&mut repository, &command)
        .expect_err("restricted visibility export denied");

    assert_eq!(err, TrpgError::VisibilityDenied);
    assert!(repository.events().is_empty());
}

#[test]
fn security_privacy_copyright_enforces_expected_version_and_idempotency() {
    let mut repository = SecurityPrivacyCopyrightRepository::default();
    let command = review_command();

    review_security_privacy_copyright_policy(&mut repository, &command)
        .expect("first command recorded");

    let mut stale = review_command();
    stale.idempotency_key = "idem_stale".to_owned();
    let err = review_security_privacy_copyright_policy(&mut repository, &stale)
        .expect_err("stale expected version denied");
    assert_eq!(
        err,
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1,
        }
    );

    let mut duplicate = command;
    duplicate.expected_version = 1;
    let err = review_security_privacy_copyright_policy(&mut repository, &duplicate)
        .expect_err("duplicate idempotency denied");
    assert_eq!(err, TrpgError::DuplicateCommand);
}

#[test]
fn security_privacy_copyright_rejects_direct_agent_write_path() {
    let mut command = review_command();
    command.write_path = FormalWritePath::DirectAgent;
    let mut repository = SecurityPrivacyCopyrightRepository::default();

    let err = review_security_privacy_copyright_policy(&mut repository, &command)
        .expect_err("direct agent state write denied");

    assert_eq!(err, TrpgError::DirectAgentStateWrite);
    assert!(repository.events().is_empty());
}

#[test]
fn security_privacy_copyright_records_data_deletion_request_as_event() {
    let mut command = deletion_command();
    command.visibility =
        Visibility::private_to_player(EntityId::new("player_001").expect("valid player id"));
    let mut repository = SecurityPrivacyCopyrightRepository::default();

    let event =
        request_data_deletion(&mut repository, &command).expect("deletion request event recorded");

    assert_eq!(event.event_type, DATA_DELETION_REQUESTED_EVENT);
    assert_eq!(event.correlation_id, command.correlation_id);
    assert_eq!(event.causation_id, command.causation_id);
    assert!(matches!(
        event.payload,
        SecurityPrivacyCopyrightEvent::DataDeletionRequested { reason, .. }
            if reason == "[redacted]"
    ));
}

#[test]
fn security_privacy_copyright_uses_current_safe_event_and_metric_names() {
    assert_eq!(
        SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT,
        "platform.security_privacy_copyright.reviewed"
    );
    assert_eq!(
        DATA_DELETION_REQUESTED_EVENT,
        "platform.security_privacy_copyright.data_deletion_requested"
    );
    assert_eq!(
        SECURITY_PRIVACY_COPYRIGHT_METRIC_MODULE,
        "security_privacy_copyright"
    );
    assert!(
        SECURITY_PRIVACY_COPYRIGHT_REQUIRED_METRICS.contains(&"trpg_visibility_redaction_total")
    );
    assert!(
        SECURITY_PRIVACY_COPYRIGHT_REQUIRED_METRICS.contains(&"trpg_data_deletion_request_total")
    );
}
