
#[allow(clippy::too_many_arguments)]
pub fn authorize_campaign_membership_change(
    policy: &OpenFgaOpaPolicyAdapter,
    audit: &mut impl AuditSink,
    identity_verifier: &IdentityVerifier,
    authentication: &AuthenticationContext,
    acting_membership: Option<&CampaignMembership>,
    authority_mode: &trpg_shared_kernel::AuthorityMode,
    campaign_id: &trpg_shared_kernel::EntityId,
    target_user_id: &str,
    requested_role: CampaignRole,
    trace_id: &str,
    now_unix_ms: u64,
) -> KernelResult<()> {
    identity_verifier
        .verify(authentication, now_unix_ms)
        .map_err(|_| TrpgError::InternalIdentityInvalid)?;
    let (principal_role, authentication_reference) = match authentication.kind() {
        PrincipalKind::UserSession {
            session_id,
            global_role: GlobalRole::ServerOwner,
        } => (PermissionPrincipalRole::ServerOwner, session_id.as_str()),
        PrincipalKind::UserSession {
            session_id,
            global_role: GlobalRole::Moderator,
        } => (PermissionPrincipalRole::Moderator, session_id.as_str()),
        PrincipalKind::UserSession {
            session_id,
            global_role: GlobalRole::User,
        } => {
            let membership = acting_membership.ok_or(TrpgError::AuthorizationDenied)?;
            if membership.user_id() != authentication.subject_id()
                || membership.campaign_id() != campaign_id
            {
                return Err(TrpgError::AuthorizationDenied);
            }
            let role = match membership.role() {
                CampaignRole::CampaignOwner => PermissionPrincipalRole::CampaignOwner,
                CampaignRole::HumanKeeper => PermissionPrincipalRole::HumanKp,
                CampaignRole::Player => PermissionPrincipalRole::Player,
                CampaignRole::Spectator => PermissionPrincipalRole::Spectator,
            };
            (role, session_id.as_str())
        }
        PrincipalKind::Workload { .. } | PrincipalKind::AgentRun { .. } => {
            return Err(TrpgError::AuthorizationDenied);
        }
    };
    if trace_id.trim().is_empty() || target_user_id.trim().is_empty() {
        return Err(TrpgError::InvalidConfiguration(
            "membership_policy_context_invalid",
        ));
    }
    let request = PolicyAuthorizationRequest {
        actor_id: authentication.subject_id().to_string(),
        principal_role: principal_role.as_str().to_owned(),
        campaign_id: campaign_id.to_string(),
        resource_type: "campaign_membership".to_owned(),
        resource_id: target_user_id.to_owned(),
        action: SecurityGovernanceAction::ManageCampaignMembership
            .as_str()
            .to_owned(),
        authority_mode: authority_mode_name(authority_mode).to_owned(),
        requested_role: Some(campaign_role_name(requested_role).to_owned()),
        target_visibility: "system_only".to_owned(),
        target_visibility_subject: None,
        trace_id: trace_id.to_owned(),
    };
    if !permission_allows(
        principal_role,
        Some(authority_mode),
        SecurityGovernanceAction::ManageCampaignMembership,
    ) {
        append_identity_policy_audit(
            audit,
            authentication,
            authentication_reference,
            &request,
            AuditDecision::Deny,
            "local-permission-deny",
            "local-permission-matrix-v1",
            "local-permission-deny",
            "local-permission-matrix-v1",
        )?;
        return Err(TrpgError::PolicyDenied);
    }

    let evidence = match policy.evaluate(&request) {
        Ok(evidence) => evidence,
        Err(error) => {
            let (openfga_revision, opa_revision) = policy.revision_snapshot();
            append_identity_policy_audit(
                audit,
                authentication,
                authentication_reference,
                &request,
                AuditDecision::Unavailable,
                "policy-unavailable",
                openfga_revision,
                "policy-unavailable",
                opa_revision,
            )?;
            return Err(error);
        }
    };
    evidence.validate()?;
    let allowed = evidence.openfga.allowed && evidence.opa.allowed;
    append_identity_policy_audit(
        audit,
        authentication,
        authentication_reference,
        &request,
        if allowed {
            AuditDecision::Permit
        } else {
            AuditDecision::Deny
        },
        &evidence.openfga.decision_id,
        &evidence.openfga.policy_revision,
        &evidence.opa.decision_id,
        &evidence.opa.policy_revision,
    )?;
    if allowed {
        Ok(())
    } else {
        Err(TrpgError::PolicyDenied)
    }
}

#[allow(clippy::too_many_arguments)]
fn append_identity_policy_audit(
    audit: &mut impl AuditSink,
    authentication: &AuthenticationContext,
    authentication_reference: &str,
    request: &PolicyAuthorizationRequest,
    decision: AuditDecision,
    openfga_decision_id: &str,
    openfga_policy_revision: &str,
    opa_decision_id: &str,
    opa_policy_revision: &str,
) -> KernelResult<()> {
    audit.append(AuditRecordDraft {
        actor_id: authentication.subject_id().to_string(),
        actor_origin: "user_session".to_owned(),
        authentication_reference: authentication_reference.to_owned(),
        campaign_id: request.campaign_id.clone(),
        resource_type: request.resource_type.clone(),
        resource_id: request.resource_id.clone(),
        action: request.action.clone(),
        requested_role: request
            .requested_role
            .clone()
            .unwrap_or_else(|| "not_applicable".to_owned()),
        visibility_label: request.target_visibility.clone(),
        visibility_subject: request
            .target_visibility_subject
            .clone()
            .unwrap_or_else(|| "not_applicable".to_owned()),
        provenance_kind: "tool_result".to_owned(),
        provenance_reference: request.trace_id.clone(),
        provenance_recorded_by: "policy_adapter".to_owned(),
        decision,
        openfga_decision_id: openfga_decision_id.to_owned(),
        openfga_policy_revision: openfga_policy_revision.to_owned(),
        opa_decision_id: opa_decision_id.to_owned(),
        opa_policy_revision: opa_policy_revision.to_owned(),
        trace_id: request.trace_id.clone(),
    })?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_policy_audit(
    audit: &mut FileAuditLog,
    command: &CommandEnvelope<SecurityGovernanceCommand>,
    request: &PolicyAuthorizationRequest,
    decision: AuditDecision,
    openfga_decision_id: &str,
    openfga_policy_revision: &str,
    opa_decision_id: &str,
    opa_policy_revision: &str,
) -> KernelResult<()> {
    audit.append(AuditRecordDraft {
        actor_id: request.actor_id.clone(),
        actor_origin: actor_origin_name(command.actor.origin()).to_owned(),
        authentication_reference: authentication_reference(&command.actor),
        campaign_id: request.campaign_id.clone(),
        resource_type: request.resource_type.clone(),
        resource_id: request.resource_id.clone(),
        action: request.action.clone(),
        requested_role: request
            .requested_role
            .clone()
            .unwrap_or_else(|| "not_applicable".to_owned()),
        visibility_label: visibility_name(command.visibility.label()).to_owned(),
        visibility_subject: command
            .visibility
            .subject_id()
            .map(ToString::to_string)
            .unwrap_or_else(|| "not_applicable".to_owned()),
        provenance_kind: provenance_kind_name(&command.fact_provenance.kind).to_owned(),
        provenance_reference: command.fact_provenance.reference.to_string(),
        provenance_recorded_by: command.fact_provenance.recorded_by.to_string(),
        decision,
        openfga_decision_id: openfga_decision_id.to_owned(),
        openfga_policy_revision: openfga_policy_revision.to_owned(),
        opa_decision_id: opa_decision_id.to_owned(),
        opa_policy_revision: opa_policy_revision.to_owned(),
        trace_id: request.trace_id.clone(),
    })?;
    Ok(())
}

fn validate_security_governance_preflight(
    module: &'static str,
    command: &CommandEnvelope<SecurityGovernanceCommand>,
) -> KernelResult<()> {
    if module.trim().is_empty() {
        return Err(TrpgError::InvalidConfiguration("module_required"));
    }
    validate_command_envelope(command)?;
    if !command.payload.target_visibility.is_well_formed() {
        return Err(TrpgError::VisibilityDenied);
    }
    Ok(())
}

fn principal_role_from_authenticated_actor<T>(
    command: &CommandEnvelope<T>,
) -> KernelResult<PermissionPrincipalRole> {
    if matches!(command.actor.origin(), ActorOrigin::AgentRun { .. }) {
        return Ok(PermissionPrincipalRole::Agent);
    }
    Ok(match command.actor.role() {
        ActorRole::ServerOwner => PermissionPrincipalRole::ServerOwner,
        ActorRole::CampaignOwner => PermissionPrincipalRole::CampaignOwner,
        ActorRole::HumanKeeper => PermissionPrincipalRole::HumanKp,
        ActorRole::AiKeeper => PermissionPrincipalRole::AiKp,
        ActorRole::Investigator => PermissionPrincipalRole::Player,
        ActorRole::Moderator => PermissionPrincipalRole::Moderator,
        ActorRole::Spectator => PermissionPrincipalRole::Spectator,
        ActorRole::Workflow => PermissionPrincipalRole::Workflow,
        ActorRole::RulesEngine => PermissionPrincipalRole::RulesEngine,
        ActorRole::System => PermissionPrincipalRole::System,
    })
}

impl PermissionPrincipalRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ServerOwner => "server_owner",
            Self::CampaignOwner => "campaign_owner",
            Self::Moderator => "moderator",
            Self::HumanKp => "human_kp",
            Self::AiKp => "ai_kp",
            Self::Player => "player",
            Self::Workflow => "workflow",
            Self::RulesEngine => "rules_engine",
            Self::System => "system",
            Self::Agent => "agent",
            Self::Provider => "provider",
            Self::Spectator => "spectator",
        }
    }
}

impl SecurityGovernanceAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PauseRoom => "pause_room",
            Self::OverrideDiceRoll => "override_dice_roll",
            Self::MutePlayer => "mute_player",
            Self::ChangeGameDecision => "change_game_decision",
            Self::ConfirmAgentDraft => "confirm_agent_draft",
            Self::RequestReconsideration => "request_reconsideration",
            Self::OverrideAiDecision => "override_ai_decision",
            Self::WriteOfficialState => "write_official_state",
            Self::ExportPlayerReport => "export_player_report",
            Self::GeneratePartySummary => "generate_party_summary",
            Self::IndexRagChunk => "index_rag_chunk",
            Self::ConnectProvider => "connect_provider",
            Self::DeletePersonalData => "delete_personal_data",
            Self::RecordAudit => "record_audit",
            Self::ImportCopyrightedFullText => "import_copyrighted_full_text",
            Self::ManageCampaignMembership => "manage_campaign_membership",
        }
    }
}

fn actor_origin_name(origin: &ActorOrigin) -> &'static str {
    match origin {
        ActorOrigin::UserSession { .. } => "user_session",
        ActorOrigin::Workload { .. } => "workload",
        ActorOrigin::AgentRun { .. } => "agent_run",
    }
}

fn authentication_reference(actor: &trpg_shared_kernel::Actor) -> String {
    match actor.origin() {
        ActorOrigin::UserSession { session_id } => session_id.as_str().to_owned(),
        ActorOrigin::Workload { .. } => actor.id().as_str().to_owned(),
        ActorOrigin::AgentRun { run_id, .. } => run_id.as_str().to_owned(),
    }
}

fn visibility_name(label: &VisibilityLabel) -> &'static str {
    label.as_str()
}

fn provenance_kind_name(kind: &trpg_shared_kernel::ProvenanceKind) -> &'static str {
    match kind {
        trpg_shared_kernel::ProvenanceKind::UserStatement => "user_statement",
        trpg_shared_kernel::ProvenanceKind::HumanKeeperStatement => "human_keeper_statement",
        trpg_shared_kernel::ProvenanceKind::RulesEngineDecision => "rules_engine_decision",
        trpg_shared_kernel::ProvenanceKind::ToolResult => "tool_result",
        trpg_shared_kernel::ProvenanceKind::AgentProposal => "agent_proposal",
        trpg_shared_kernel::ProvenanceKind::ImportedSource => "imported_source",
        trpg_shared_kernel::ProvenanceKind::SystemFixture => "system_fixture",
    }
}

fn campaign_role_name(role: CampaignRole) -> &'static str {
    match role {
        CampaignRole::CampaignOwner => "campaign_owner",
        CampaignRole::HumanKeeper => "human_keeper",
        CampaignRole::Player => "player",
        CampaignRole::Spectator => "spectator",
    }
}

fn authority_mode_name(mode: &trpg_shared_kernel::AuthorityMode) -> &'static str {
    match mode {
        trpg_shared_kernel::AuthorityMode::HumanKp => "human_kp",
        trpg_shared_kernel::AuthorityMode::AiKp => "ai_kp",
    }
}
