
#[tokio::test]
async fn canonical_deletion_rejects_an_unwitnessed_hmac_shaped_digest() {
    let mut command = deletion_command();
    command.visibility =
        Visibility::private_to_player(EntityId::new("player_001").expect("valid player id"));
    command.fact_provenance = trpg_shared_kernel::FactProvenance::new(
        trpg_shared_kernel::ProvenanceKind::UserStatement,
        command.command_id.as_str(),
        "test_authority_registrar",
    )
    .unwrap();
    let deletion_port = RecordingDeletionPort::default();
    let canonical = ForgedHashCanonicalPort {
        inner: trpg_test_support::test_canonical_commit_port(),
    };
    let (authorizer, workflow, requester) = permitted_deletion_custody();

    let error = request_data_deletion_canonical(
        &deletion_port,
        &authorizer,
        &canonical,
        &workflow,
        Some(&requester),
        &command,
        202,
    )
    .await
    .expect_err("an unverified HMAC-shaped digest must not confirm deletion evidence");

    assert_eq!(error, TrpgError::AuditIntegrityViolation);
    assert!(!deletion_port.called.load(Ordering::SeqCst));
    assert!(!deletion_port.confirmed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn authenticated_non_member_cannot_delete_data_in_another_campaign() {
    let mut command = deletion_command();
    command.visibility =
        Visibility::private_to_player(EntityId::new("player_001").expect("valid player id"));
    command.fact_provenance = trpg_shared_kernel::FactProvenance::new(
        trpg_shared_kernel::ProvenanceKind::UserStatement,
        command.command_id.as_str(),
        "test_authority_registrar",
    )
    .unwrap();
    let deletion_port = RecordingDeletionPort::default();
    let canonical = trpg_test_support::test_canonical_commit_port();
    let (authorizer, workflow, non_member) = deletion_custody_without_campaign_membership();

    let error = request_data_deletion_canonical(
        &deletion_port,
        &authorizer,
        canonical.as_ref(),
        &workflow,
        Some(&non_member),
        &command,
        202,
    )
    .await
    .expect_err("authentication without live membership in this campaign must fail closed");

    assert_eq!(error, TrpgError::AuthorizationDenied);
    assert!(!deletion_port.called.load(Ordering::SeqCst));
    assert!(!deletion_port.confirmed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn stale_deletion_command_cannot_create_a_job_before_event_validation() {
    let mut command = deletion_command();
    command.expected_version = 1;
    let mut repository = SecurityPrivacyCopyrightRepository::default();
    let deletion_port = RecordingDeletionPort::default();
    let (authorizer, authentication, requester) = permitted_deletion_custody();

    let error = request_data_deletion(
        &mut repository,
        &deletion_port,
        &authorizer,
        &authentication,
        Some(&requester),
        &command,
        202,
    )
    .await
    .expect_err("a stale command must fail before creating a deletion job");

    assert_eq!(
        error,
        TrpgError::ExpectedVersionConflict {
            expected: 1,
            actual: 0,
        }
    );
    assert!(!deletion_port.called.load(Ordering::SeqCst));
    assert!(repository.events().is_empty());
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
