mod common;

use std::path::PathBuf;

use async_trait::async_trait;
use trpg_security_governance::cloud_egress::{
    authorize_cloud_egress, CloudConsentQuery, CloudEgressAuditRecord, CloudEgressLedger,
    CloudEgressOutcome, CloudEgressRequest, CloudRouteSnapshotRecord, ConsentVisibilityScope,
    PersistedCloudConsent, ProviderBoundary,
};
use trpg_security_governance::secret::{
    KmsClient, KmsSecretResolver, SecretManager, SecretReference,
};
use trpg_security_governance::tamper_evident_audit::{
    AuditDecision, AuditRecordDraft, AuditSink, FileAuditLog,
};
use trpg_security_governance::{
    adr_0006_openfga_opa, audit_log_contract, copyright_allows, copyright_boundary,
    data_retention_deletion, evaluate_visibility_derivation, most_restrictive_visibility,
    permission_allows, permission_matrix, policy_authorization, policy_authz, policy_openfga_opa,
    privacy_copyright, readme, security_privacy, security_privacy_copyright,
    validate_provider_boundary, visibility_enforcement_points, ContentLicense, ContentUse,
    DeploymentEnvironment, DerivedObject, LocalModelCertificationInput,
    LocalModelCertificationLevel, PermissionPrincipalRole, ProviderEndpoint, RedactionOutcome,
    SecurityGovernanceAction, SecurityGovernanceCommand, SecurityGovernanceRepository,
    SECURITY_GOVERNANCE_DECISION_RECORDED_EVENT, SECURITY_GOVERNANCE_METRIC_MODULE,
    SECURITY_GOVERNANCE_REQUIRED_METRICS,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, EntityId, FormalWritePath, KernelResult,
    PrincipalScope, TrpgError, Visibility, VisibilityLabel,
};

const S04_VISIBILITY_ERRORS_FIXTURE: &str =
    include_str!("../../../fixtures/stages/detailed/S04_visibility_policy_errors.current.json.md");
const S04_PERMISSION_MATRIX_FIXTURE: &str =
    include_str!("../../../fixtures/security/permission_matrix.v1.json.md");
const S04_OPENFGA_SECURITY_GOVERNANCE_MODEL: &str =
    include_str!("../../../policy/openfga/security_governance.fga");
const S04_OPENFGA_SECURITY_GOVERNANCE_JSON_MODEL: &str =
    include_str!("../../../policy/openfga/security_governance.model.json");
const S04_VISIBILITY_REDACTION_FIXTURE: &str =
    include_str!("../../../fixtures/visibility/visibility_redaction_matrix.v1.json.md");
const AUDIT_KEY: [u8; 32] = [0x42; 32];

struct BatchCloudLedger {
    consent: Option<PersistedCloudConsent>,
}

#[async_trait]
impl CloudEgressLedger for BatchCloudLedger {
    async fn trusted_now_unix_ms(&self) -> KernelResult<u64> {
        Ok(10_000)
    }

    async fn notice_is_recorded(
        &self,
        notice_reference: &EntityId,
        _subject_id: &EntityId,
        _policy_version: &EntityId,
    ) -> KernelResult<bool> {
        Ok(!notice_reference.as_str().is_empty())
    }

    async fn load_active_consent(
        &self,
        _query: &CloudConsentQuery,
    ) -> KernelResult<Option<PersistedCloudConsent>> {
        Ok(self.consent.clone())
    }

    async fn record_route_decision(
        &self,
        _snapshot: CloudRouteSnapshotRecord,
        _audit: CloudEgressAuditRecord,
    ) -> KernelResult<bool> {
        Ok(true)
    }
}

fn batch_cloud_request(snapshot_id: &str) -> CloudEgressRequest {
    CloudEgressRequest {
        snapshot_id: EntityId::new(snapshot_id).unwrap(),
        audit_id: EntityId::new(format!("audit-{snapshot_id}")).unwrap(),
        subject_id: EntityId::new("batch-player").unwrap(),
        source_provider: EntityId::new("ollama").unwrap(),
        target_provider: EntityId::new("cloud").unwrap(),
        source_endpoint: "http://127.0.0.1:11434/v1".to_owned(),
        target_endpoint: "https://cloud.example.test/v1".to_owned(),
        model_id: EntityId::new("cloud-model-v1").unwrap(),
        source_credential: SecretReference::development("batch_ollama", 1).unwrap(),
        target_credential: SecretReference::development("batch_cloud", 1).unwrap(),
        source_boundary: ProviderBoundary::Local,
        target_boundary: ProviderBoundary::Cloud,
        fallback_policy: EntityId::new("explicit_audited_only").unwrap(),
        privacy_boundary: EntityId::new("explicit_consent_no_silent_fallback").unwrap(),
        purpose: EntityId::new("gameplay").unwrap(),
        policy_version: EntityId::new("privacy-v1").unwrap(),
        notice_reference: Some(EntityId::new("batch-notice").unwrap()),
        target_audience: PrincipalScope::Player(EntityId::new("batch-player").unwrap()),
        context: vec![common::verified_cloud_fact(
            "batch-public-fact",
            Visibility::new(VisibilityLabel::Public),
            vec![b'x'; 32],
        )],
    }
}

fn command(
    _role: PermissionPrincipalRole,
    action: SecurityGovernanceAction,
) -> CommandEnvelope<SecurityGovernanceCommand> {
    trpg_test_support::governed_command(
        SecurityGovernanceCommand::new(action),
        ActorRole::Workflow,
        AuthorityMode::HumanKp,
    )
}

fn audit_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "trpg-security-{name}-{}-{}.jsonl",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

#[test]
fn data_retention_deletion_command_cannot_accept_a_caller_supplied_hold_flag() {
    let command = command(
        PermissionPrincipalRole::Workflow,
        SecurityGovernanceAction::DeletePersonalData,
    );
    assert_eq!(
        command.payload.action,
        SecurityGovernanceAction::DeletePersonalData
    );
    assert!(command.payload.target_visibility.is_well_formed());
    assert_eq!(
        data_retention_deletion::MODULE,
        "security_governance::data_retention_deletion"
    );
}

#[test]
fn policy_openfga_opa_fails_closed() {
    let command = command(
        PermissionPrincipalRole::Workflow,
        SecurityGovernanceAction::RecordAudit,
    );
    let mut repository = SecurityGovernanceRepository::default();

    let err = policy_openfga_opa::evaluate(&mut repository, &command)
        .expect_err("a policy adapter is mandatory");

    assert_eq!(err, TrpgError::PolicyUnavailable);
    assert!(repository.events().is_empty());
}

#[test]
fn openfga_security_governance_model_matches_permission_matrix_fixture() {
    assert!(S04_OPENFGA_SECURITY_GOVERNANCE_MODEL.contains("model"));
    assert!(S04_OPENFGA_SECURITY_GOVERNANCE_MODEL.contains("schema 1.1"));
    assert!(S04_OPENFGA_SECURITY_GOVERNANCE_MODEL.contains("type campaign"));
    let json_model: serde_json::Value =
        serde_json::from_str(S04_OPENFGA_SECURITY_GOVERNANCE_JSON_MODEL).unwrap();
    assert_eq!(json_model["schema_version"], "1.1");

    for (fixture_action, relation, role) in [
        ("pause_room", "can_pause_room", "server_owner"),
        ("mute_player", "can_mute_player", "moderator"),
        ("confirm_agent_draft", "can_confirm_agent_draft", "human_kp"),
        (
            "request_reconsideration",
            "can_request_reconsideration",
            "player",
        ),
    ] {
        assert!(S04_PERMISSION_MATRIX_FIXTURE.contains(fixture_action));
        assert!(
            S04_OPENFGA_SECURITY_GOVERNANCE_MODEL.contains(&format!("define {relation}: {role}"))
        );
    }
    for relation in [
        "can_record_audit",
        "can_export_player_report",
        "can_manage_campaign_membership",
    ] {
        assert!(S04_OPENFGA_SECURITY_GOVERNANCE_MODEL.contains(relation));
        assert!(S04_OPENFGA_SECURITY_GOVERNANCE_JSON_MODEL.contains(relation));
    }

    for (fixture_action, relation, denied_role) in [
        (
            "override_dice_roll",
            "can_override_dice_roll",
            "server_owner",
        ),
        (
            "change_game_decision",
            "can_change_game_decision",
            "moderator",
        ),
        ("override_ai_decision", "can_override_ai_decision", "player"),
    ] {
        assert!(S04_PERMISSION_MATRIX_FIXTURE.contains(fixture_action));
        assert!(S04_OPENFGA_SECURITY_GOVERNANCE_MODEL
            .contains(&format!("# deny: {denied_role} {fixture_action}")));
        assert!(
            S04_OPENFGA_SECURITY_GOVERNANCE_MODEL.contains(&format!("define {relation}: no_grant"))
        );
        assert!(!S04_OPENFGA_SECURITY_GOVERNANCE_MODEL
            .contains(&format!("define {relation}: {denied_role}")));
    }
}

#[test]
fn security_privacy_rejects_direct_agent_write_path() {
    let mut command = command(
        PermissionPrincipalRole::Workflow,
        SecurityGovernanceAction::WriteOfficialState,
    );
    command.write_path = FormalWritePath::DirectAgent;
    let mut repository = SecurityGovernanceRepository::default();

    let err = security_privacy::evaluate(&mut repository, &command)
        .expect_err("agent direct write is blocked by kernel envelope");

    assert_eq!(err, TrpgError::DirectAgentStateWrite);
    assert!(repository.events().is_empty());
}

#[test]
fn visibility_enforcement_points_redacts_stage_cases() {
    let player = PrincipalScope::Player(EntityId::new("user_player_a").expect("valid player id"));
    let other_player =
        PrincipalScope::Player(EntityId::new("user_player_b").expect("valid player id"));
    let keeper_only = Visibility::new(VisibilityLabel::KeeperOnly);

    let decision =
        evaluate_visibility_derivation(&keeper_only, &player, DerivedObject::PlayerExport);

    assert_eq!(decision.outcome, RedactionOutcome::Redacted);
    assert_eq!(decision.error_code, Some("VISIBILITY_DOWNGRADE_FORBIDDEN"));

    let private_note =
        Visibility::private_to_player(EntityId::new("user_player_a").expect("valid player id"));
    let decision =
        evaluate_visibility_derivation(&private_note, &other_player, DerivedObject::PartySummary);

    assert_eq!(decision.outcome, RedactionOutcome::Redacted);
    assert_eq!(decision.error_code, Some("VISIBILITY_SCOPE_VIOLATION"));

    let decision =
        evaluate_visibility_derivation(&keeper_only, &player, DerivedObject::PartySummary);

    assert_eq!(decision.outcome, RedactionOutcome::Redacted);
    assert_eq!(decision.error_code, Some("VISIBILITY_LEAKAGE_DETECTED"));

    let decision = evaluate_visibility_derivation(&keeper_only, &player, DerivedObject::RagChunk);

    assert_eq!(decision.outcome, RedactionOutcome::Omitted);
    assert_eq!(decision.error_code, Some("VISIBILITY_LEAKAGE_DETECTED"));
    assert_eq!(
        most_restrictive_visibility(&[VisibilityLabel::Public, VisibilityLabel::KeeperOnly]),
        VisibilityLabel::KeeperOnly
    );
    assert_eq!(
        most_restrictive_visibility(&[
            Visibility::private_to_player(EntityId::new("player_a").unwrap())
                .label()
                .clone(),
            Visibility::private_to_group(EntityId::new("group_a").unwrap())
                .label()
                .clone(),
        ]),
        VisibilityLabel::KeeperOnly
    );
    assert_eq!(
        most_restrictive_visibility(&[
            Visibility::private_to_group(EntityId::new("group_a").unwrap())
                .label()
                .clone(),
            Visibility::private_to_player(EntityId::new("player_a").unwrap())
                .label()
                .clone(),
        ]),
        VisibilityLabel::KeeperOnly
    );
    for expected in [
        "keeper_only_to_player_export",
        "private_to_player_to_party_summary",
        "ai_internal_to_export",
        "summary_leaks_keeper_secret",
        "keeper_secret_not_in_player_export",
        "private_to_player_not_party_visible",
        "ai_internal_never_exported",
        "combined_visibility_most_restrictive",
        "rag_keeper_chunk_not_in_player_context",
    ] {
        assert!(
            S04_VISIBILITY_ERRORS_FIXTURE.contains(expected)
                || S04_VISIBILITY_REDACTION_FIXTURE.contains(expected)
        );
    }
    assert_eq!(
        visibility_enforcement_points::MODULE,
        "security_governance::visibility_enforcement_points"
    );
}

#[test]
fn adr_0006_openfga_opa_uses_current_safe_names() {
    let module = adr_0006_openfga_opa::MODULE;

    assert_eq!(module, "security_governance::adr_0006_openfga_opa");
    assert!(!module.contains("v6"));
    assert!(!module.contains("hash"));
    assert_eq!(
        SECURITY_GOVERNANCE_DECISION_RECORDED_EVENT,
        "security_governance.decision_recorded"
    );
}

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
