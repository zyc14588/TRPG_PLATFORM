use crate::readme::redact_for_observability;
use trpg_identity::{AuthenticationContext, GlobalRole, PrincipalKind};
use trpg_privacy::{ConfirmedDeletionRecord, DeletionRequestEvidence, DeletionRequestPort};
use trpg_security_governance::formal_commit_audit::{FormalAuthorization, FormalCommitAuthorizer};
use trpg_security_governance::{
    evaluate_derived_visibility, DerivationRequest, DerivedObject, RedactionOutcome,
};
use trpg_shared_kernel::{
    AuthorityMode, CanonicalCommitEvent, CanonicalCommitPort, CanonicalCommitRequest,
    CanonicalCommittedEvent, CommandEnvelope, EntityId, EventEnvelope, EventStore, KernelResult,
    PrincipalScope, ProvenanceKind, TrpgError,
};

pub const SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT: &str =
    "platform.security_privacy_copyright.reviewed";
pub const DATA_DELETION_REQUESTED_EVENT: &str =
    "platform.security_privacy_copyright.data_deletion_requested";
pub const SECURITY_PRIVACY_COPYRIGHT_METRIC_MODULE: &str = "security_privacy_copyright";
pub const SECURITY_PRIVACY_COPYRIGHT_REQUIRED_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_visibility_redaction_total",
    "trpg_data_deletion_request_total",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewSecurityPrivacyCopyrightPolicy {
    pub asset_id: String,
    pub license_tag: String,
    pub detail: String,
    pub export_intent: ExportIntent,
}

/// A typed destination prevents a caller from supplying an `export_allowed`
/// decision. The service derives that decision from the command envelope's
/// governed visibility label and this exact audience.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportIntent {
    ReviewOnly,
    ExportTo(ExportAudience),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ExportAudience {
    Public,
    Party,
    Player(EntityId),
}

impl ExportAudience {
    fn principal(&self) -> PrincipalScope {
        match self {
            Self::Public => PrincipalScope::Public,
            Self::Party => PrincipalScope::PartyMember,
            Self::Player(player_id) => PrincipalScope::Player(player_id.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ExportDisposition {
    ReviewOnly,
    Authorized { audience: ExportAudience },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestDataDeletion {
    pub job_id: String,
    pub subject_id: String,
    pub retention_policy: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum SecurityPrivacyCopyrightEvent {
    SecurityPrivacyCopyrightReviewed {
        asset_id: String,
        license_tag: String,
        detail: String,
        export: ExportDisposition,
    },
    DataDeletionRequested {
        job_id: String,
        subject_id: String,
        requested_by: String,
        retention_policy: String,
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecurityPrivacyCopyrightError {
    AssetIdRequired,
    LicenseTagRequired,
    SubjectIdRequired,
    RetentionPolicyRequired,
    RestrictedVisibilityExportDenied,
}

impl From<SecurityPrivacyCopyrightError> for TrpgError {
    fn from(error: SecurityPrivacyCopyrightError) -> Self {
        match error {
            SecurityPrivacyCopyrightError::AssetIdRequired => {
                TrpgError::InvalidConfiguration("asset_id_required")
            }
            SecurityPrivacyCopyrightError::LicenseTagRequired => {
                TrpgError::InvalidConfiguration("license_tag_required")
            }
            SecurityPrivacyCopyrightError::SubjectIdRequired => {
                TrpgError::InvalidConfiguration("subject_id_required")
            }
            SecurityPrivacyCopyrightError::RetentionPolicyRequired => {
                TrpgError::InvalidConfiguration("retention_policy_required")
            }
            SecurityPrivacyCopyrightError::RestrictedVisibilityExportDenied => {
                TrpgError::VisibilityDenied
            }
        }
    }
}

pub type SecurityPrivacyCopyrightEventEnvelope = EventEnvelope<SecurityPrivacyCopyrightEvent>;
pub type SecurityPrivacyCopyrightRepository = EventStore<SecurityPrivacyCopyrightEvent>;

pub struct SecurityPrivacyCopyrightService;

impl SecurityPrivacyCopyrightService {
    pub fn review_security_privacy_copyright_policy(
        repository: &mut SecurityPrivacyCopyrightRepository,
        authorizer: &FormalCommitAuthorizer,
        workflow_authentication: &AuthenticationContext,
        authorizing_authentication: Option<&AuthenticationContext>,
        command: &CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy>,
        now_unix_ms: u64,
    ) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
        if command.payload.asset_id.trim().is_empty() {
            return Err(SecurityPrivacyCopyrightError::AssetIdRequired.into());
        }
        if command.payload.license_tag.trim().is_empty() {
            return Err(SecurityPrivacyCopyrightError::LicenseTagRequired.into());
        }
        authorizer.authorize(
            workflow_authentication,
            authorizing_authentication,
            command,
            "security_privacy_reviewer",
            now_unix_ms,
        )?;
        let export = derive_export_disposition(command)?;

        repository.append(
            command,
            SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT,
            SecurityPrivacyCopyrightEvent::SecurityPrivacyCopyrightReviewed {
                asset_id: command.payload.asset_id.clone(),
                license_tag: command.payload.license_tag.clone(),
                detail: redact_for_observability(&command.visibility, &command.payload.detail),
                export,
            },
        )
    }

    pub async fn request_data_deletion(
        repository: &mut SecurityPrivacyCopyrightRepository,
        deletion_requests: &impl DeletionRequestPort,
        authorizer: &FormalCommitAuthorizer,
        workflow_authentication: &AuthenticationContext,
        authorizing_authentication: Option<&AuthenticationContext>,
        command: &CommandEnvelope<RequestDataDeletion>,
        now_unix_ms: u64,
    ) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
        let (_authorization, event_payload, _evidence) = prepare_deletion_request(
            authorizer,
            workflow_authentication,
            authorizing_authentication,
            command,
            now_unix_ms,
        )?;

        // Preserve validation/error ordering for callers, but never create a
        // side-table job from this legacy two-phase path. Only the canonical
        // commit-first service below may persist a deletion workflow.
        repository.validate_append(command, DATA_DELETION_REQUESTED_EVENT, &event_payload)?;
        let _ = deletion_requests;
        Err(TrpgError::InvalidConfiguration(
            "canonical_deletion_commit_required",
        ))
    }
}

fn prepare_deletion_request(
    authorizer: &FormalCommitAuthorizer,
    workflow_authentication: &AuthenticationContext,
    authorizing_authentication: Option<&AuthenticationContext>,
    command: &CommandEnvelope<RequestDataDeletion>,
    now_unix_ms: u64,
) -> KernelResult<(
    FormalAuthorization,
    SecurityPrivacyCopyrightEvent,
    DeletionRequestEvidence,
)> {
    if command.payload.subject_id.trim().is_empty() {
        return Err(SecurityPrivacyCopyrightError::SubjectIdRequired.into());
    }
    if command.payload.retention_policy.trim().is_empty() {
        return Err(SecurityPrivacyCopyrightError::RetentionPolicyRequired.into());
    }
    let authorizing_authentication =
        authorizing_authentication.ok_or(TrpgError::AuthorizationDenied)?;
    let requester_may_delete_subject = authorizing_authentication.subject_id().as_str()
        == command.payload.subject_id
        || matches!(
            authorizing_authentication.kind(),
            PrincipalKind::UserSession {
                global_role: GlobalRole::Moderator | GlobalRole::ServerOwner,
                ..
            }
        );
    if !requester_may_delete_subject {
        return Err(TrpgError::AuthorizationDenied);
    }
    let authorization = authorizer.authorize_campaign_member_scoped_action(
        workflow_authentication,
        Some(authorizing_authentication),
        command,
        "delete_personal_data",
        "data_subject",
        &command.payload.subject_id,
        "privacy_deletion_requester",
        now_unix_ms,
    )?;
    let event_payload = SecurityPrivacyCopyrightEvent::DataDeletionRequested {
        job_id: command.payload.job_id.clone(),
        subject_id: command.payload.subject_id.clone(),
        requested_by: authorization.canonical_audit().actor_id.clone(),
        retention_policy: command.payload.retention_policy.clone(),
        reason: redact_for_observability(&command.visibility, &command.payload.reason),
    };
    let evidence = DeletionRequestEvidence::new(
        authorization.contract().campaign_id().as_str(),
        command.command_id.as_str(),
        command.correlation_id.as_str(),
        command.causation_id.as_str(),
        DATA_DELETION_REQUESTED_EVENT,
    )
    .map_err(|_| TrpgError::InvalidConfiguration("deletion_evidence_invalid"))?;
    Ok((authorization, event_payload, evidence))
}

/// Production deletion request path. Authorization is performed before the
/// pending job is created, and only the exact HMAC-bearing event identity
/// returned by the canonical Event Store can make that job executable.
#[allow(clippy::too_many_arguments)]
pub async fn request_data_deletion_canonical(
    deletion_requests: &impl DeletionRequestPort,
    authorizer: &FormalCommitAuthorizer,
    canonical: &dyn CanonicalCommitPort,
    workflow_authentication: &AuthenticationContext,
    authorizing_authentication: Option<&AuthenticationContext>,
    command: &CommandEnvelope<RequestDataDeletion>,
    now_unix_ms: u64,
) -> KernelResult<CanonicalCommittedEvent> {
    let (authorization, event_payload, evidence) = prepare_deletion_request(
        authorizer,
        workflow_authentication,
        authorizing_authentication,
        command,
        now_unix_ms,
    )?;
    let requested_by = authorization.canonical_audit().actor_id.clone();

    // Bind the workflow to the same canonical JSON representation that the
    // Event Store validates and hashes. Serializing a Rust enum directly can
    // preserve declaration order while the store normalizes object keys,
    // producing semantically identical but byte-different receipt checks.
    let payload_value =
        serde_json::to_value(&event_payload).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let payload_json =
        serde_json::to_string(&payload_value).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let contract = authorization.contract();
    let request = CanonicalCommitRequest {
        commit_id: format!(
            "{}_{}",
            contract.campaign_id().as_str(),
            command.command_id.as_str()
        ),
        campaign_id: contract.campaign_id().to_string(),
        idempotency_key: command.idempotency_key.clone(),
        expected_version: command.expected_version,
        command_id: command.command_id.to_string(),
        authenticated_actor_id: command.actor.id().to_string(),
        authenticated_actor_role: command.actor.canonical_role_name().to_owned(),
        authenticated_actor_origin: command.actor.canonical_origin_wire(),
        authority_mode: authority_mode_name(&command.authority_mode).to_owned(),
        authority_contract_version: contract.version(),
        authority_contract_id: contract.contract_id().to_string(),
        authority_owner: contract.authority_owner().to_string(),
        visibility_label: command.visibility.label().as_str().to_owned(),
        visibility_subject: command
            .visibility
            .subject_id()
            .map(ToString::to_string)
            .unwrap_or_else(|| "not_applicable".to_owned()),
        data_subject_id: command.payload.subject_id.clone(),
        provenance_kind: provenance_kind_name(&command.fact_provenance.kind).to_owned(),
        provenance_reference: command.fact_provenance.reference.to_string(),
        provenance_recorded_by: command.fact_provenance.recorded_by.to_string(),
        correlation_id: command.correlation_id.to_string(),
        causation_id: command.causation_id.to_string(),
        trace_id: command.authenticated_context().trace_id().to_string(),
        events: vec![CanonicalCommitEvent {
            event_type: DATA_DELETION_REQUESTED_EVENT.to_owned(),
            payload_json: payload_json.clone(),
        }],
        audit: authorization.canonical_audit().clone(),
    };
    let receipt = canonical.commit(&request)?;
    canonical.verify_receipt(&request, &receipt)?;
    let expected_stream_version = command
        .expected_version
        .checked_add(1)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    if receipt.first_stream_version != expected_stream_version
        || receipt.last_stream_version != expected_stream_version
        || receipt.events.len() != 1
    {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let event = receipt
        .events
        .into_iter()
        .next()
        .filter(|event| event.stream_version == expected_stream_version)
        .filter(|event| event.event_type == DATA_DELETION_REQUESTED_EVENT)
        .filter(|event| event.payload_json == payload_json)
        .filter(|event| event.command_id == request.command_id)
        .filter(|event| event.idempotency_key == format!("{}:0000", request.idempotency_key))
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    deletion_requests
        .record_confirmed_deletion(ConfirmedDeletionRecord {
            job_id: &command.payload.job_id,
            subject_id: &command.payload.subject_id,
            requested_by: &requested_by,
            retention_policy: &command.payload.retention_policy,
            evidence: &evidence,
            canonical_event_sequence: event.sequence,
            canonical_event_integrity_hash: &event.event_integrity_hash,
        })
        .await
        .map_err(|_| TrpgError::InvalidConfiguration("deletion_evidence_persistence_failed"))?;
    Ok(event)
}

fn authority_mode_name(mode: &AuthorityMode) -> &'static str {
    match mode {
        AuthorityMode::HumanKp => "human_kp",
        AuthorityMode::AiKp => "ai_kp",
    }
}

fn provenance_kind_name(kind: &ProvenanceKind) -> &'static str {
    match kind {
        ProvenanceKind::UserStatement => "user_statement",
        ProvenanceKind::HumanKeeperStatement => "human_keeper_statement",
        ProvenanceKind::RulesEngineDecision => "rules_engine_decision",
        ProvenanceKind::ToolResult => "tool_result",
        ProvenanceKind::AgentProposal => "agent_proposal",
        ProvenanceKind::ImportedSource => "imported_source",
        ProvenanceKind::SystemFixture => "system_fixture",
    }
}

fn derive_export_disposition(
    command: &CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy>,
) -> KernelResult<ExportDisposition> {
    let ExportIntent::ExportTo(audience) = &command.payload.export_intent else {
        return Ok(ExportDisposition::ReviewOnly);
    };
    let sources = [command.visibility.clone()];
    let target = audience.principal();
    let decision = evaluate_derived_visibility(DerivationRequest {
        sources: &sources,
        processor: &PrincipalScope::System,
        target_audience: &target,
        target: DerivedObject::PlayerExport,
    });
    if decision.outcome != RedactionOutcome::Visible {
        return Err(SecurityPrivacyCopyrightError::RestrictedVisibilityExportDenied.into());
    }
    Ok(ExportDisposition::Authorized {
        audience: audience.clone(),
    })
}

pub fn review_security_privacy_copyright_policy(
    repository: &mut SecurityPrivacyCopyrightRepository,
    authorizer: &FormalCommitAuthorizer,
    workflow_authentication: &AuthenticationContext,
    authorizing_authentication: Option<&AuthenticationContext>,
    command: &CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy>,
    now_unix_ms: u64,
) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
    SecurityPrivacyCopyrightService::review_security_privacy_copyright_policy(
        repository,
        authorizer,
        workflow_authentication,
        authorizing_authentication,
        command,
        now_unix_ms,
    )
}

pub async fn request_data_deletion(
    repository: &mut SecurityPrivacyCopyrightRepository,
    deletion_requests: &impl DeletionRequestPort,
    authorizer: &FormalCommitAuthorizer,
    workflow_authentication: &AuthenticationContext,
    authorizing_authentication: Option<&AuthenticationContext>,
    command: &CommandEnvelope<RequestDataDeletion>,
    now_unix_ms: u64,
) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
    SecurityPrivacyCopyrightService::request_data_deletion(
        repository,
        deletion_requests,
        authorizer,
        workflow_authentication,
        authorizing_authentication,
        command,
        now_unix_ms,
    )
    .await
}
