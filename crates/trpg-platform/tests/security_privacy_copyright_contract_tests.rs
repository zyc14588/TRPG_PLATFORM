use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use trpg_platform::security_privacy_copyright::{
    request_data_deletion, request_data_deletion_canonical,
    review_security_privacy_copyright_policy, ExportAudience, ExportIntent, RequestDataDeletion,
    ReviewSecurityPrivacyCopyrightPolicy, SecurityPrivacyCopyrightEvent,
    SecurityPrivacyCopyrightEventEnvelope, SecurityPrivacyCopyrightRepository,
    DATA_DELETION_REQUESTED_EVENT, SECURITY_PRIVACY_COPYRIGHT_METRIC_MODULE,
    SECURITY_PRIVACY_COPYRIGHT_REQUIRED_METRICS, SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT,
};
use trpg_security_governance::formal_commit_audit::{FormalCommitAudit, FormalCommitAuthorizer};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::security_privacy::{
    ConfirmedDeletionRecord, DeletionEvidenceStatus, DeletionJob, DeletionJobStatus,
    DeletionRequestEvidence, DeletionRequestPort, PrivacyError,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, FormalWritePath,
    KernelResult, PrincipalScope, TrpgError, Visibility, VisibilityLabel,
};

static NEXT_AUDIT_ID: AtomicU64 = AtomicU64::new(1);

fn contract() -> AuthorityContract {
    trpg_test_support::authority_contract("camp_human_archive", AuthorityMode::HumanKp, 1)
        .expect("valid platform privacy authority contract")
}

fn audit() -> FormalCommitAudit {
    let id = NEXT_AUDIT_ID.fetch_add(1, Ordering::Relaxed);
    FormalCommitAudit::open(
        std::env::temp_dir().join(format!(
            "p05-platform-privacy-audit-{}-{id}.jsonl",
            std::process::id()
        )),
        "p05-platform-privacy-audit-key",
        &[0x73; 32],
    )
    .expect("open dedicated platform privacy audit")
}

fn custody_with_policy(
    policy: OpenFgaOpaPolicyAdapter,
) -> (FormalCommitAuthorizer, trpg_identity::AuthenticationContext) {
    let contract = contract();
    let (identity, authentication) =
        trpg_test_support::formal_commit_identity_for_contract(&contract);
    (
        FormalCommitAuthorizer::new(identity, policy, audit()),
        authentication,
    )
}

fn permitted_custody() -> (FormalCommitAuthorizer, trpg_identity::AuthenticationContext) {
    let endpoints = trpg_test_support::formal_commit_policy_endpoints();
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            endpoints.openfga,
            "/stores/test/check",
            PolicyBackend::OpenFga,
            endpoints.openfga_model,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            endpoints.opa,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            endpoints.opa_revision,
        )
        .unwrap(),
    )
    .unwrap();
    custody_with_policy(policy)
}

fn permitted_deletion_custody() -> (
    FormalCommitAuthorizer,
    trpg_identity::AuthenticationContext,
    trpg_identity::AuthenticationContext,
) {
    use trpg_identity::{CampaignRole, WorkloadRole};

    let contract = contract();
    let mut identity = trpg_test_support::identity_service_for_contract(&contract);
    let session = identity
        .login(
            "test-authority-registrar@example.test",
            "test authority password long enough",
            200,
        )
        .unwrap();
    let requester = identity
        .authenticate_session(Some(session.token.expose()), 201)
        .unwrap();
    identity
        .grant_membership(
            &requester,
            contract.campaign_id().as_str(),
            requester.subject_id().as_str(),
            CampaignRole::CampaignOwner,
            201,
        )
        .unwrap();
    let credential = identity
        .issue_workload_credential("workflow_001", WorkloadRole::WorkflowEngine, 1, u64::MAX)
        .unwrap();
    let workflow = identity.authenticate_workload(&credential, 2).unwrap();
    let endpoints = trpg_test_support::formal_commit_policy_endpoints();
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            endpoints.openfga,
            "/stores/test/check",
            PolicyBackend::OpenFga,
            endpoints.openfga_model,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            endpoints.opa,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            endpoints.opa_revision,
        )
        .unwrap(),
    )
    .unwrap();
    (
        FormalCommitAuthorizer::new(identity.verifier(), policy, audit()),
        workflow,
        requester,
    )
}

fn real_deletion_custody(
    workflow_id: &str,
    workload_role: trpg_identity::WorkloadRole,
) -> (
    FormalCommitAuthorizer,
    trpg_identity::AuthenticationContext,
    trpg_identity::AuthenticationContext,
) {
    use trpg_identity::CampaignRole;

    let contract = contract();
    let mut identity = trpg_test_support::identity_service_for_contract(&contract);
    let session = identity
        .login(
            "test-authority-registrar@example.test",
            "test authority password long enough",
            200,
        )
        .unwrap();
    let requester = identity
        .authenticate_session(Some(session.token.expose()), 201)
        .unwrap();
    identity
        .grant_membership(
            &requester,
            contract.campaign_id().as_str(),
            requester.subject_id().as_str(),
            CampaignRole::CampaignOwner,
            201,
        )
        .unwrap();
    let credential = identity
        .issue_workload_credential(workflow_id, workload_role, 1, u64::MAX)
        .unwrap();
    let workflow = identity.authenticate_workload(&credential, 2).unwrap();
    let openfga_address = std::env::var("P02_OPENFGA_ADDRESS")
        .expect("P02_OPENFGA_ADDRESS must identify the real policy service")
        .parse()
        .expect("valid P02 OpenFGA address");
    let openfga_store =
        std::env::var("P02_OPENFGA_STORE_ID").expect("P02_OPENFGA_STORE_ID is required");
    let openfga_model =
        std::env::var("P02_OPENFGA_MODEL_ID").expect("P02_OPENFGA_MODEL_ID is required");
    let opa_address = std::env::var("P02_OPA_ADDRESS")
        .expect("P02_OPA_ADDRESS must identify the real policy service")
        .parse()
        .expect("valid P02 OPA address");
    let opa_revision = std::env::var("P02_OPA_REVISION")
        .unwrap_or_else(|_| "opa-security-governance-v3".to_owned());
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga_address,
            format!("/stores/{openfga_store}/check"),
            PolicyBackend::OpenFga,
            openfga_model,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            opa_address,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            opa_revision,
        )
        .unwrap(),
    )
    .unwrap();
    (
        FormalCommitAuthorizer::new(identity.verifier(), policy, audit()),
        workflow,
        requester,
    )
}

fn deletion_custody_without_campaign_membership() -> (
    FormalCommitAuthorizer,
    trpg_identity::AuthenticationContext,
    trpg_identity::AuthenticationContext,
) {
    use trpg_identity::WorkloadRole;

    let contract = contract();
    let mut identity = trpg_test_support::identity_service_for_contract(&contract);
    let session = identity
        .login(
            "test-authority-registrar@example.test",
            "test authority password long enough",
            200,
        )
        .unwrap();
    let requester = identity
        .authenticate_session(Some(session.token.expose()), 201)
        .unwrap();
    let credential = identity
        .issue_workload_credential("workflow_001", WorkloadRole::WorkflowEngine, 1, u64::MAX)
        .unwrap();
    let workflow = identity.authenticate_workload(&credential, 2).unwrap();
    let endpoints = trpg_test_support::formal_commit_policy_endpoints();
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            endpoints.openfga,
            "/stores/test/check",
            PolicyBackend::OpenFga,
            endpoints.openfga_model,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            endpoints.opa,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            endpoints.opa_revision,
        )
        .unwrap(),
    )
    .unwrap();
    (
        FormalCommitAuthorizer::new(identity.verifier(), policy, audit()),
        workflow,
        requester,
    )
}

fn unused_loopback_address() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve loopback address");
    listener.local_addr().expect("loopback address")
}

fn unavailable_custody() -> (FormalCommitAuthorizer, trpg_identity::AuthenticationContext) {
    let openfga = unused_loopback_address();
    let opa = unused_loopback_address();
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga,
            "/stores/test/check",
            PolicyBackend::OpenFga,
            "unavailable-openfga-v1",
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            opa,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            "unavailable-opa-v1",
        )
        .unwrap(),
    )
    .unwrap();
    custody_with_policy(policy)
}

fn execute_review(
    repository: &mut SecurityPrivacyCopyrightRepository,
    command: &CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy>,
) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
    let (authorizer, authentication) = permitted_custody();
    review_security_privacy_copyright_policy(
        repository,
        &authorizer,
        &authentication,
        None,
        command,
        2,
    )
}

fn review_command() -> CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy> {
    let mut command = trpg_test_support::governed_command_for_contract(
        &contract(),
        ReviewSecurityPrivacyCopyrightPolicy {
            asset_id: "handout_001".to_owned(),
            license_tag: "original_campaign_asset".to_owned(),
            detail: "keeper_only_handout_notes".to_owned(),
            export_intent: ExportIntent::ExportTo(ExportAudience::Public),
        },
        ActorRole::Workflow,
    );
    command.visibility = Visibility::new(VisibilityLabel::Public);
    command
}

fn deletion_command() -> CommandEnvelope<RequestDataDeletion> {
    trpg_test_support::governed_command_for_contract(
        &contract(),
        RequestDataDeletion {
            job_id: "deletion_001".to_owned(),
            subject_id: "player_001".to_owned(),
            retention_policy: "audit_log_retained_private_payload_removed".to_owned(),
            reason: "player privacy request".to_owned(),
        },
        ActorRole::Workflow,
    )
}

#[derive(Default)]
struct RecordingDeletionPort {
    called: AtomicBool,
    confirmed: AtomicBool,
    requested_by: Mutex<Option<String>>,
}

#[derive(Debug)]
struct ForgedHashCanonicalPort {
    inner: Arc<dyn trpg_shared_kernel::CanonicalCommitPort>,
}

impl trpg_shared_kernel::CanonicalCommitPort for ForgedHashCanonicalPort {
    fn commit(
        &self,
        request: &trpg_shared_kernel::CanonicalCommitRequest,
    ) -> trpg_shared_kernel::KernelResult<trpg_shared_kernel::CanonicalCommitReceipt> {
        let mut receipt = self.inner.commit(request)?;
        // This is deliberately well-shaped but was not produced by the
        // trusted keyed store or its external witness.
        let forged = &mut receipt.events[0].event_integrity_hash;
        let final_nibble = forged.pop().ok_or(TrpgError::AuditIntegrityViolation)?;
        forged.push(if final_nibble == '0' { '1' } else { '0' });
        Ok(receipt)
    }

    fn verify_receipt(
        &self,
        request: &trpg_shared_kernel::CanonicalCommitRequest,
        receipt: &trpg_shared_kernel::CanonicalCommitReceipt,
    ) -> trpg_shared_kernel::KernelResult<()> {
        self.inner.verify_receipt(request, receipt)
    }
}

#[async_trait]
impl DeletionRequestPort for RecordingDeletionPort {
    async fn request_deletion(
        &self,
        job_id: &str,
        subject_id: &str,
        requested_by: &str,
        retention_policy: &str,
        evidence: &DeletionRequestEvidence,
    ) -> Result<DeletionJob, PrivacyError> {
        self.called.store(true, Ordering::SeqCst);
        *self.requested_by.lock().expect("record requested_by") = Some(requested_by.to_owned());
        Ok(DeletionJob {
            job_id: job_id.to_owned(),
            campaign_id: evidence.campaign_id().to_string(),
            subject_id: subject_id.to_owned(),
            requested_by: requested_by.to_owned(),
            retention_policy: retention_policy.to_owned(),
            status: DeletionJobStatus::Requested,
            failure_code: None,
            evidence_status: DeletionEvidenceStatus::Pending,
            canonical_event_sequence: None,
            canonical_event_integrity_hash: None,
            targets: Vec::new(),
        })
    }

    async fn confirm_deletion_evidence(
        &self,
        job_id: &str,
        evidence: &DeletionRequestEvidence,
        canonical_event_sequence: u64,
        canonical_event_integrity_hash: &str,
    ) -> Result<DeletionJob, PrivacyError> {
        self.confirmed.store(true, Ordering::SeqCst);
        Ok(DeletionJob {
            job_id: job_id.to_owned(),
            campaign_id: evidence.campaign_id().to_string(),
            subject_id: "player_001".to_owned(),
            requested_by: "test_authority_registrar".to_owned(),
            retention_policy: "audit_log_retained_private_payload_removed".to_owned(),
            status: DeletionJobStatus::Requested,
            failure_code: None,
            evidence_status: DeletionEvidenceStatus::Confirmed,
            canonical_event_sequence: Some(canonical_event_sequence),
            canonical_event_integrity_hash: Some(canonical_event_integrity_hash.to_owned()),
            targets: Vec::new(),
        })
    }

    async fn record_confirmed_deletion(
        &self,
        record: ConfirmedDeletionRecord<'_>,
    ) -> Result<DeletionJob, PrivacyError> {
        self.called.store(true, Ordering::SeqCst);
        self.confirmed.store(true, Ordering::SeqCst);
        *self.requested_by.lock().expect("record requested_by") =
            Some(record.requested_by.to_owned());
        Ok(DeletionJob {
            job_id: record.job_id.to_owned(),
            campaign_id: record.evidence.campaign_id().to_string(),
            subject_id: record.subject_id.to_owned(),
            requested_by: record.requested_by.to_owned(),
            retention_policy: record.retention_policy.to_owned(),
            status: DeletionJobStatus::Requested,
            failure_code: None,
            evidence_status: DeletionEvidenceStatus::Confirmed,
            canonical_event_sequence: Some(record.canonical_event_sequence),
            canonical_event_integrity_hash: Some(record.canonical_event_integrity_hash.to_owned()),
            targets: Vec::new(),
        })
    }
}

#[test]
fn security_privacy_copyright_rejects_authority_contract_violation() {
    let command = trpg_test_support::governed_command(
        review_command().payload,
        ActorRole::AiKeeper,
        AuthorityMode::HumanKp,
    );
    let mut repository = SecurityPrivacyCopyrightRepository::default();

    let err = execute_review(&mut repository, &command).expect_err("authority mismatch denied");

    assert_eq!(err, TrpgError::AuthorityViolation);
    assert!(repository.events().is_empty());
}

#[test]
fn security_privacy_copyright_keeps_visibility_and_fact_provenance_on_replay() {
    let mut command = review_command();
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.payload.export_intent = ExportIntent::ReviewOnly;
    let mut repository = SecurityPrivacyCopyrightRepository::default();

    let event = execute_review(&mut repository, &command).expect("review recorded");

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
fn security_privacy_copyright_fails_closed_when_authoritative_policy_is_unavailable() {
    let command = review_command();
    let mut repository = SecurityPrivacyCopyrightRepository::default();
    let (authorizer, authentication) = unavailable_custody();

    let err = review_security_privacy_copyright_policy(
        &mut repository,
        &authorizer,
        &authentication,
        None,
        &command,
        2,
    )
    .expect_err("unavailable OpenFGA/OPA cannot be replaced by a caller permit flag");

    assert_eq!(err, TrpgError::PolicyUnavailable);
    assert!(repository.events().is_empty());
}

#[test]
fn security_privacy_copyright_rejects_restricted_visibility_export() {
    let mut command = review_command();
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.payload.export_intent = ExportIntent::ExportTo(ExportAudience::Public);
    let mut repository = SecurityPrivacyCopyrightRepository::default();

    let err =
        execute_review(&mut repository, &command).expect_err("restricted visibility export denied");

    assert_eq!(err, TrpgError::VisibilityDenied);
    assert!(repository.events().is_empty());
}

#[test]
fn caller_safe_flag_cannot_authorize_keeper_only_export() {
    let mut command = review_command();
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.payload.export_intent = ExportIntent::ExportTo(ExportAudience::Public);
    let mut repository = SecurityPrivacyCopyrightRepository::default();

    let error = execute_review(&mut repository, &command)
        .expect_err("caller classification flags cannot authorize a keeper-only export");

    assert_eq!(error, TrpgError::VisibilityDenied);
    assert!(repository.events().is_empty());
}

#[test]
fn security_privacy_copyright_enforces_expected_version_and_idempotency() {
    let mut repository = SecurityPrivacyCopyrightRepository::default();
    let command = review_command();

    execute_review(&mut repository, &command).expect("first command recorded");

    let mut stale = review_command();
    stale.idempotency_key = "idem_stale".to_owned();
    let err = execute_review(&mut repository, &stale).expect_err("stale expected version denied");
    assert_eq!(
        err,
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1,
        }
    );

    let mut duplicate = command;
    duplicate.expected_version = 1;
    let err =
        execute_review(&mut repository, &duplicate).expect_err("duplicate idempotency denied");
    assert_eq!(err, TrpgError::DuplicateCommand);
}

#[test]
fn security_privacy_copyright_rejects_direct_agent_write_path() {
    let mut command = review_command();
    command.write_path = FormalWritePath::DirectAgent;
    let mut repository = SecurityPrivacyCopyrightRepository::default();

    let err =
        execute_review(&mut repository, &command).expect_err("direct agent state write denied");

    assert_eq!(err, TrpgError::DirectAgentStateWrite);
    assert!(repository.events().is_empty());
}

#[tokio::test]
async fn legacy_two_phase_deletion_path_fails_closed_without_side_effects() {
    let mut command = deletion_command();
    command.visibility =
        Visibility::private_to_player(EntityId::new("player_001").expect("valid player id"));
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
    .expect_err("legacy two-phase deletion must require the canonical path");

    assert_eq!(
        error,
        TrpgError::InvalidConfiguration("canonical_deletion_commit_required")
    );
    assert!(!deletion_port.called.load(Ordering::SeqCst));
    assert!(!deletion_port.confirmed.load(Ordering::SeqCst));
    assert!(repository.events().is_empty());
}

#[tokio::test]
async fn canonical_deletion_confirmation_uses_store_generated_hmac_evidence() {
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
    let (authorizer, workflow, requester) = permitted_deletion_custody();

    let event = request_data_deletion_canonical(
        &deletion_port,
        &authorizer,
        canonical.as_ref(),
        &workflow,
        Some(&requester),
        &command,
        202,
    )
    .await
    .expect("canonical deletion event confirms the pending job");

    assert!(deletion_port.called.load(Ordering::SeqCst));
    assert!(deletion_port.confirmed.load(Ordering::SeqCst));
    assert!(event.event_integrity_hash.starts_with("hmac-sha256:"));
    assert_eq!(event.event_type, DATA_DELETION_REQUESTED_EVENT);
}

#[tokio::test]
async fn canonical_deletion_uses_real_policy_and_denial_creates_no_pending_job() {
    let mut command = deletion_command();
    command.visibility =
        Visibility::private_to_player(EntityId::new("player_001").expect("valid player id"));
    command.fact_provenance = trpg_shared_kernel::FactProvenance::new(
        trpg_shared_kernel::ProvenanceKind::UserStatement,
        command.command_id.as_str(),
        "test_authority_registrar",
    )
    .unwrap();

    let permitted_port = RecordingDeletionPort::default();
    let permitted_canonical = trpg_test_support::test_canonical_commit_port();
    let (permitted_authorizer, permitted_workflow, permitted_requester) =
        real_deletion_custody("workflow_001", trpg_identity::WorkloadRole::WorkflowEngine);
    request_data_deletion_canonical(
        &permitted_port,
        &permitted_authorizer,
        permitted_canonical.as_ref(),
        &permitted_workflow,
        Some(&permitted_requester),
        &command,
        202,
    )
    .await
    .expect("checked-in OpenFGA and OPA policies permit the seeded workflow");
    assert!(permitted_port.called.load(Ordering::SeqCst));
    assert!(permitted_port.confirmed.load(Ordering::SeqCst));

    let mut denied_command = trpg_test_support::governed_command_for_contract(
        &contract(),
        command.payload.clone(),
        ActorRole::RulesEngine,
    );
    denied_command.visibility =
        Visibility::private_to_player(EntityId::new("player_001").expect("valid player id"));
    denied_command.fact_provenance = trpg_shared_kernel::FactProvenance::new(
        trpg_shared_kernel::ProvenanceKind::UserStatement,
        denied_command.command_id.as_str(),
        "test_authority_registrar",
    )
    .unwrap();
    let denied_port = RecordingDeletionPort::default();
    let denied_canonical = trpg_test_support::test_canonical_commit_port();
    let (denied_authorizer, denied_workflow, denied_requester) =
        real_deletion_custody("rules_001", trpg_identity::WorkloadRole::RulesEngine);
    let error = request_data_deletion_canonical(
        &denied_port,
        &denied_authorizer,
        denied_canonical.as_ref(),
        &denied_workflow,
        Some(&denied_requester),
        &denied_command,
        202,
    )
    .await
    .expect_err("an ungranted workflow must be denied by the real OpenFGA model");
    assert_eq!(error, TrpgError::PolicyDenied);
    assert!(!denied_port.called.load(Ordering::SeqCst));
    assert!(!denied_port.confirmed.load(Ordering::SeqCst));
}

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
