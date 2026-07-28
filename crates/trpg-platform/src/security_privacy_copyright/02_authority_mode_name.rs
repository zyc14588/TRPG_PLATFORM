
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
