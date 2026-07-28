
#[test]
fn audit_log_contract_persists_audit_metadata() {
    let mut command = command(
        PermissionPrincipalRole::Workflow,
        SecurityGovernanceAction::RecordAudit,
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let path = audit_path("audit-contract");
    let mut anchor_name = path.as_os_str().to_os_string();
    anchor_name.push(".head");
    let anchor_path = PathBuf::from(anchor_name);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&anchor_path);
    let mut audit = FileAuditLog::open(&path, "test-audit-key-v1", &AUDIT_KEY).unwrap();

    audit
        .append(AuditRecordDraft {
            actor_id: command.actor.id().to_string(),
            actor_origin: "workload".to_owned(),
            authentication_reference: "workflow_001".to_owned(),
            campaign_id: "camp_human_archive".to_owned(),
            resource_type: "campaign".to_owned(),
            resource_id: "camp_human_archive".to_owned(),
            action: "record_audit".to_owned(),
            requested_role: "not_applicable".to_owned(),
            visibility_label: "keeper_only".to_owned(),
            visibility_subject: "not_applicable".to_owned(),
            provenance_kind: "rules_engine_decision".to_owned(),
            provenance_reference: command.fact_provenance.reference.to_string(),
            provenance_recorded_by: command.fact_provenance.recorded_by.to_string(),
            decision: AuditDecision::Permit,
            openfga_decision_id: "openfga_batch_035".to_owned(),
            openfga_policy_revision: "openfga_model_035".to_owned(),
            opa_decision_id: "opa_batch_035".to_owned(),
            opa_policy_revision: "opa_bundle_035".to_owned(),
            trace_id: "trace_001".to_owned(),
        })
        .unwrap();

    assert_eq!(
        audit_log_contract::MODULE,
        "security_governance::audit_log_contract"
    );
    let records = audit.verify().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].actor_id, command.actor.id().as_str());
    assert_eq!(records[0].authentication_reference, "workflow_001");
    assert_eq!(records[0].visibility_label, "keeper_only");
    assert_eq!(records[0].provenance_reference, "fact_001");
    assert_eq!(records[0].openfga_policy_revision, "openfga_model_035");
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(anchor_path).unwrap();
}

#[test]
fn copyright_boundary_rejects_commercial_full_text() {
    assert!(!copyright_allows(
        ContentLicense::CopyrightedCommercial,
        ContentUse::FullTextImport
    ));
    assert!(copyright_allows(
        ContentLicense::CopyrightedCommercial,
        ContentUse::ShortQuote
    ));
    assert_eq!(
        copyright_boundary::MODULE,
        "security_governance::copyright_boundary"
    );
}

#[test]
fn security_privacy_copyright_denies_prod_placeholder_provider() {
    struct TestKms;
    impl KmsClient for TestKms {
        fn decrypt_secret(&self, _secret_id: &str, _version: u64) -> KernelResult<Vec<u8>> {
            Ok(b"not-used-for-rejected-public-endpoint".to_vec())
        }
    }

    let endpoint = ProviderEndpoint::new(
        "ollama",
        "http://0.0.0.0:11434/v1",
        trpg_security_governance::secret::SecretReference::mounted("ollama_credential", 1).unwrap(),
        DeploymentEnvironment::Production,
        "local-model-v1",
        "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
    )
    .unwrap();
    let manager = SecretManager::new(KmsSecretResolver::new(TestKms));

    let err = validate_provider_boundary(&endpoint, &manager)
        .expect_err("prod local exposure is blocked");

    assert_eq!(
        err,
        TrpgError::InvalidConfiguration("unauthenticated_local_provider_exposed")
    );
    assert!(endpoint.credential().production_eligible());
    assert_eq!(
        security_privacy_copyright::MODULE,
        "security_governance::security_privacy_copyright"
    );
}

#[test]
fn policy_authz_matches_permission_matrix_fixture() {
    for (fixture_action, role, authority_mode, action, expected) in [
        (
            "pause_room",
            PermissionPrincipalRole::ServerOwner,
            None,
            SecurityGovernanceAction::PauseRoom,
            true,
        ),
        (
            "override_dice_roll",
            PermissionPrincipalRole::ServerOwner,
            None,
            SecurityGovernanceAction::OverrideDiceRoll,
            false,
        ),
        (
            "mute_player",
            PermissionPrincipalRole::Moderator,
            None,
            SecurityGovernanceAction::MutePlayer,
            true,
        ),
        (
            "change_game_decision",
            PermissionPrincipalRole::Moderator,
            None,
            SecurityGovernanceAction::ChangeGameDecision,
            false,
        ),
        (
            "confirm_agent_draft",
            PermissionPrincipalRole::HumanKp,
            Some(AuthorityMode::HumanKp),
            SecurityGovernanceAction::ConfirmAgentDraft,
            true,
        ),
        (
            "request_reconsideration",
            PermissionPrincipalRole::Player,
            Some(AuthorityMode::AiKp),
            SecurityGovernanceAction::RequestReconsideration,
            true,
        ),
        (
            "override_ai_decision",
            PermissionPrincipalRole::Player,
            Some(AuthorityMode::AiKp),
            SecurityGovernanceAction::OverrideAiDecision,
            false,
        ),
        (
            "change_game_decision",
            PermissionPrincipalRole::CampaignOwner,
            Some(AuthorityMode::HumanKp),
            SecurityGovernanceAction::ChangeGameDecision,
            false,
        ),
        (
            "override_ai_decision",
            PermissionPrincipalRole::Spectator,
            Some(AuthorityMode::AiKp),
            SecurityGovernanceAction::OverrideAiDecision,
            false,
        ),
    ] {
        assert!(S04_PERMISSION_MATRIX_FIXTURE.contains(fixture_action));
        assert_eq!(
            permission_allows(role, authority_mode.as_ref(), action),
            expected
        );
    }
    assert_eq!(policy_authz::MODULE, "security_governance::policy_authz");
}

#[test]
fn policy_authorization_enforces_authority_specific_actions() {
    let human_mode = AuthorityMode::HumanKp;
    let ai_mode = AuthorityMode::AiKp;

    assert!(permission_allows(
        PermissionPrincipalRole::HumanKp,
        Some(&human_mode),
        SecurityGovernanceAction::ConfirmAgentDraft
    ));
    assert!(!permission_allows(
        PermissionPrincipalRole::Player,
        Some(&ai_mode),
        SecurityGovernanceAction::OverrideAiDecision
    ));
    assert_eq!(
        policy_authorization::MODULE,
        "security_governance::policy_authorization"
    );
}

#[test]
fn privacy_copyright_blocks_ai_internal_export() {
    let source = Visibility::new(VisibilityLabel::AiInternal);
    let decision = evaluate_visibility_derivation(
        &source,
        &PrincipalScope::Keeper,
        DerivedObject::PlayerExport,
    );

    assert_eq!(decision.outcome, RedactionOutcome::Redacted);
    assert_eq!(decision.error_code, Some("AI_INTERNAL_EXPORT_FORBIDDEN"));
    assert_eq!(
        privacy_copyright::MODULE,
        "security_governance::privacy_copyright"
    );
}

#[test]
fn readme_contract_lists_required_governance_metrics() {
    assert_eq!(SECURITY_GOVERNANCE_METRIC_MODULE, "security_governance");
    assert!(SECURITY_GOVERNANCE_REQUIRED_METRICS.contains(&"trpg_policy_deny_total"));
    assert!(SECURITY_GOVERNANCE_REQUIRED_METRICS.contains(&"trpg_visibility_redaction_total"));
    assert_eq!(readme::MODULE, "security_governance::readme");
}

#[tokio::test]
async fn permission_matrix_covers_provider_certification_and_fallback() {
    let stable_model = LocalModelCertificationInput {
        json_schema_support: true,
        tool_call_support: true,
        visibility_tests_pass: true,
        rules_eval_pass: true,
        latency_ms: 1_800,
    };

    assert_eq!(
        trpg_security_governance::certify_local_model(stable_model),
        LocalModelCertificationLevel::LocalModelLevel4
    );
    let denied = authorize_cloud_egress(
        &BatchCloudLedger { consent: None },
        batch_cloud_request("batch-route-denied"),
    )
    .await
    .unwrap();
    assert!(matches!(denied, CloudEgressOutcome::Denied { .. }));
    let allowed = authorize_cloud_egress(
        &BatchCloudLedger {
            consent: Some(PersistedCloudConsent::loaded_from_repository(
                EntityId::new("batch-consent").unwrap(),
                EntityId::new("batch-player").unwrap(),
                EntityId::new("cloud").unwrap(),
                EntityId::new("gameplay").unwrap(),
                EntityId::new("privacy-v1").unwrap(),
                ConsentVisibilityScope::PublicOnly,
                20_000,
            )),
        },
        batch_cloud_request("batch-route-allowed"),
    )
    .await
    .unwrap();
    assert!(matches!(allowed, CloudEgressOutcome::Authorized(_)));
    assert_eq!(
        permission_matrix::MODULE,
        "security_governance::permission_matrix"
    );
}
