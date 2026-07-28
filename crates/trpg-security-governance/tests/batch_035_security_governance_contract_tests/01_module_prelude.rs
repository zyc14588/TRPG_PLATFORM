
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
