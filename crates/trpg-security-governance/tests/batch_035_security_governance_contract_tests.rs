use std::path::PathBuf;

use trpg_security_governance::tamper_evident_audit::{
    AuditDecision, AuditRecordDraft, AuditSink, FileAuditLog,
};
use trpg_security_governance::{
    adr_0006_openfga_opa, audit_log_contract, copyright_allows, copyright_boundary,
    data_retention_deletion, evaluate_cloud_fallback, evaluate_visibility_derivation,
    is_placeholder_api_key, most_restrictive_visibility, permission_allows, permission_matrix,
    policy_authorization, policy_authz, policy_openfga_opa, privacy_copyright, readme,
    security_privacy, security_privacy_copyright, validate_provider_boundary,
    visibility_enforcement_points, CloudFallbackDecision, CloudFallbackRequest, ContentLicense,
    ContentUse, DeploymentEnvironment, DerivedObject, LocalModelCertificationInput,
    LocalModelCertificationLevel, PermissionPrincipalRole, ProviderEndpoint, RedactionOutcome,
    SecurityGovernanceAction, SecurityGovernanceCommand, SecurityGovernanceRepository,
    SECURITY_GOVERNANCE_DECISION_RECORDED_EVENT, SECURITY_GOVERNANCE_METRIC_MODULE,
    SECURITY_GOVERNANCE_REQUIRED_METRICS,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, EntityId, FormalWritePath, PrincipalScope,
    TrpgError, Visibility, VisibilityLabel,
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
fn data_retention_deletion_rejects_legal_hold() {
    let mut command = command(
        PermissionPrincipalRole::Workflow,
        SecurityGovernanceAction::DeleteRetainedData,
    );
    command.payload.legal_hold = true;
    let mut repository = SecurityGovernanceRepository::default();

    let err = data_retention_deletion::evaluate(&mut repository, &command)
        .expect_err("legal hold blocks deletion");

    assert_eq!(err, TrpgError::PolicyDenied);
    assert!(repository.events().is_empty());
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
    let endpoint = ProviderEndpoint {
        provider_type: "ollama".to_owned(),
        base_url: "http://0.0.0.0:11434/v1".to_owned(),
        api_key: "ollama".to_owned(),
        environment: DeploymentEnvironment::Production,
        authenticated: false,
    };

    let err = validate_provider_boundary(&endpoint).expect_err("prod local exposure is blocked");

    assert_eq!(
        err,
        TrpgError::InvalidConfiguration("unauthenticated_local_provider_exposed")
    );
    assert!(is_placeholder_api_key("sk-no-key-required"));
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

#[test]
fn permission_matrix_covers_provider_certification_and_fallback() {
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
    assert_eq!(
        evaluate_cloud_fallback(CloudFallbackRequest {
            cloud_fallback_enabled: false,
            cloud_call_attempted: true,
            user_notice: false,
            snapshot_recorded: false,
        }),
        CloudFallbackDecision::DenyAndAudit
    );
    assert_eq!(
        evaluate_cloud_fallback(CloudFallbackRequest {
            cloud_fallback_enabled: true,
            cloud_call_attempted: true,
            user_notice: true,
            snapshot_recorded: true,
        }),
        CloudFallbackDecision::Allow
    );
    assert_eq!(
        permission_matrix::MODULE,
        "security_governance::permission_matrix"
    );
}
