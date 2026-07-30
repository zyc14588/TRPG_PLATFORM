#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentJobRouteConfiguration {
    pub provider_id: String,
    pub provider_type: String,
    pub model_id: String,
    pub model_artifact_sha256: String,
    pub route_authorization_event_id: String,
}

impl AgentJobRouteConfiguration {
    fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("TRPG_MODEL_PROVIDER_ID", self.provider_id.as_str()),
            (
                "TRPG_MODEL_ROUTE_AUTHORIZATION_EVENT_ID",
                self.route_authorization_event_id.as_str(),
            ),
        ] {
            EntityId::new(value).map_err(|_| format!("{name}_INVALID"))?;
        }
        if self.model_id.trim().is_empty() || self.model_id.len() > 256 {
            return Err("TRPG_MODEL_ID_INVALID".to_owned());
        }
        if !matches!(
            self.provider_type.as_str(),
            "cloud" | "ollama" | "llama_cpp"
        ) {
            return Err("TRPG_MODEL_PROVIDER_TYPE_INVALID".to_owned());
        }
        if !self
            .model_artifact_sha256
            .strip_prefix("sha256:")
            .is_some_and(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
        {
            return Err("TRPG_MODEL_ARTIFACT_SHA256_INVALID".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct AgentJobGateway {
    workflow: DurableWorkflowStore,
    route: AgentJobRouteConfiguration,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestAgentJobBody {
    command: ApiCommandFields,
    campaign_id: String,
    job_id: String,
    rag_snapshot_id: String,
    input: serde_json::Value,
    deadline_unix_ms: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApproveAgentJobBody {
    command: ApiCommandFields,
    campaign_id: String,
    job_id: String,
}

impl ApiApplication {
    fn request_agent_job(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        let body: RequestAgentJobBody = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id
            || body.command.expected_version != 0
            || EntityId::new(&body.job_id).is_err()
            || EntityId::new(&body.rag_snapshot_id).is_err()
            || body.command.idempotency_key.trim().is_empty()
            || body.command.idempotency_key.len() > 160
        {
            return agent_job_error(400, "AGENT_JOB_REQUEST_INVALID");
        }
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let now_i64 = match i64::try_from(now) {
            Ok(now) => now,
            Err(_) => return internal_error(),
        };
        if body.deadline_unix_ms <= now_i64
            || body.deadline_unix_ms.saturating_sub(now_i64) > 300_000
        {
            return agent_job_error(400, "AGENT_JOB_DEADLINE_INVALID");
        }
        let context = match self.authorized_core_context(
            request,
            campaign_id,
            "agent_job",
            &body.job_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            "AGENT_JOB",
            "request_agent_job",
            now,
        ) {
            Ok(context) => context,
            Err(response) => return response,
        };
        let Some(custody) = &self.canonical_custody else {
            return agent_job_error(503, "AGENT_JOB_GATEWAY_UNAVAILABLE");
        };
        let Some(gateway) = custody.agent_jobs.as_ref() else {
            return agent_job_error(503, "AGENT_JOB_GATEWAY_UNAVAILABLE");
        };
        let authority = match custody.runtime.lock() {
            Ok(runtime) => {
                runtime.block_on(gateway.workflow.load_agent_authority_snapshot(campaign_id))
            }
            Err(_) => return internal_error(),
        };
        let authority = match authority {
            Ok(authority) => authority,
            Err(error) => return workflow_error(error),
        };
        if authority.contract_id != context.authority_contract_id()
            || authority.authority_mode.to_ascii_lowercase() != context.authority_mode()
            || authority.authority_owner != context.authority_owner()
            || authority.contract_version != context.authority_contract_version()
            || authority.model_route_snapshot != gateway.route.route_authorization_event_id
        {
            return agent_job_error(409, "AGENT_JOB_AUTHORITY_ROUTE_MISMATCH");
        }
        let (actor_id, agent_kind, visibility_scope, visibility_label) =
            match context.authority_mode() {
                "ai_kp" => (
                    authority.authority_owner.clone(),
                    "ai_keeper_orchestrator",
                    json!({
                        "allowed_labels": ["public", "party_visible"],
                        "subject_id": null,
                        "output_label": "party_visible"
                    }),
                    "party_visible",
                ),
                "human_kp"
                    if context.actor_id() == authority.authority_owner
                        && context.actor_role() == "human_keeper" =>
                {
                    (
                        authority.authority_owner.clone(),
                        "keeper_copilot",
                        json!({
                            "allowed_labels": ["public", "party_visible", "keeper_only"],
                            "subject_id": null,
                            "output_label": "keeper_only"
                        }),
                        "keeper_only",
                    )
                }
                _ => return agent_job_error(403, "AGENT_JOB_AUTHORITY_FORBIDDEN"),
            };
        let event_payload = json!({
            "agent_kind": agent_kind,
            "authority_contract_id": authority.contract_id,
            "authority_contract_version": authority.contract_version,
            "authority_mode": authority.authority_mode,
            "deadline_unix_ms": body.deadline_unix_ms,
            "input": body.input,
            "job_id": body.job_id,
            "model_id": gateway.route.model_id,
            "provider_id": gateway.route.provider_id,
            "rag_snapshot_id": body.rag_snapshot_id,
            "requested_by": context.actor_id(),
            "route_authorization_event_id": gateway.route.route_authorization_event_id,
            "visibility_scope": visibility_scope,
        });
        let source_event = match commit_agent_gateway_event(
            custody,
            &context,
            &body.command,
            "AgentJobRequested",
            &event_payload,
            visibility_label,
            CanonicalAgentActor::Workflow,
        ) {
            Ok(event) => event,
            Err(response) => return response,
        };
        let draft = AgentJobEnqueueDraft {
            job_id: body.job_id.clone(),
            campaign_id: campaign_id.to_owned(),
            actor_id,
            agent_kind: agent_kind.to_owned(),
            authority_contract_id: authority.contract_id,
            authority_mode: authority.authority_mode,
            authority_contract_version: authority.contract_version,
            input_event_sequence: match i64::try_from(source_event.sequence) {
                Ok(sequence) => sequence,
                Err(_) => return internal_error(),
            },
            input_stream_version: match i64::try_from(source_event.stream_version) {
                Ok(version) => version,
                Err(_) => return internal_error(),
            },
            visibility_scope_json: visibility_scope.to_string(),
            rag_snapshot_id: body.rag_snapshot_id,
            provider_id: gateway.route.provider_id.clone(),
            provider_type: gateway.route.provider_type.clone(),
            model_id: gateway.route.model_id.clone(),
            model_artifact_sha256: gateway.route.model_artifact_sha256.clone(),
            route_authorization_event_id: gateway.route.route_authorization_event_id.clone(),
            prompt_template_id: "keeper_turn".to_owned(),
            prompt_template_version: authority.prompt_version,
            tool_schema_version: authority.tool_schema_version,
            idempotency_key: body.command.idempotency_key,
            deadline_unix_ms: body.deadline_unix_ms,
        };
        let enqueued = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(gateway.workflow.enqueue_agent_job(&draft)),
            Err(_) => return internal_error(),
        };
        match enqueued {
            Ok(job) if job.state == WorkflowState::Requested => HttpResponse::json(
                202,
                json!({
                    "input_event_sequence": job.input_event_sequence,
                    "job_id": job.job_id,
                    "state": job.state.as_str(),
                }),
            ),
            Ok(_) => agent_job_error(409, "AGENT_JOB_STATE_CONFLICT"),
            Err(error) => workflow_error(error),
        }
    }

    fn approve_agent_job(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        job_id: &str,
    ) -> HttpResponse {
        let body: ApproveAgentJobBody = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id
            || body.job_id != job_id
            || EntityId::new(job_id).is_err()
            || body.command.idempotency_key.trim().is_empty()
            || body.command.idempotency_key.len() > 160
        {
            return agent_job_error(400, "AGENT_JOB_APPROVAL_INVALID");
        }
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let context = match self.authorized_core_context(
            request,
            campaign_id,
            "agent_job",
            job_id,
            &body.command,
            Visibility::new(VisibilityLabel::KeeperOnly),
            false,
            "AGENT_JOB",
            "approve_agent_job",
            now,
        ) {
            Ok(context) => context,
            Err(response) => return response,
        };
        let Some(custody) = &self.canonical_custody else {
            return agent_job_error(503, "AGENT_JOB_GATEWAY_UNAVAILABLE");
        };
        let Some(gateway) = custody.agent_jobs.as_ref() else {
            return agent_job_error(503, "AGENT_JOB_GATEWAY_UNAVAILABLE");
        };
        let job = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(gateway.workflow.load_agent_job(job_id)),
            Err(_) => return internal_error(),
        };
        let job = match job {
            Ok(Some(job)) => job,
            Ok(None) => return agent_job_error(404, "AGENT_JOB_NOT_FOUND"),
            Err(error) => return workflow_error(error),
        };
        if context.authority_mode() != "human_kp"
            || context.actor_role() != "human_keeper"
            || context.actor_id() != context.authority_owner()
            || job.campaign_id != campaign_id
            || job.authority_mode != "HUMAN_KP"
            || job.authority_contract_id != context.authority_contract_id()
            || job.authority_contract_version != context.authority_contract_version()
            || job.state != WorkflowState::AwaitingTool
            || body.command.expected_version != job.input_stream_version
        {
            return agent_job_error(403, "AGENT_JOB_APPROVAL_FORBIDDEN");
        }
        let decision = match job
            .decision_json
            .as_deref()
            .and_then(|decision| serde_json::from_str::<serde_json::Value>(decision).ok())
        {
            Some(decision) => decision,
            None => return agent_job_error(409, "AGENT_JOB_DRAFT_UNAVAILABLE"),
        };
        let event_payload = json!({
            "approved_by": context.actor_id(),
            "decision": decision,
            "job_id": job_id,
        });
        let approval_event = match commit_agent_gateway_event(
            custody,
            &context,
            &body.command,
            "AgentDraftApproved",
            &event_payload,
            "keeper_only",
            CanonicalAgentActor::RequestingHuman,
        ) {
            Ok(event) => event,
            Err(response) => return response,
        };
        let approval_event_sequence = match i64::try_from(approval_event.sequence) {
            Ok(sequence) => sequence,
            Err(_) => return internal_error(),
        };
        let approval = AgentJobApprovalDraft {
            approval_id: format!("approval_{job_id}"),
            job_id: job_id.to_owned(),
            approval_event_sequence,
            approved_by: context.actor_id().to_owned(),
            idempotency_key: approval_event.idempotency_key,
        };
        let recorded = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(gateway.workflow.record_agent_job_approval(&approval)),
            Err(_) => return internal_error(),
        };
        match recorded {
            Ok(approval) => HttpResponse::json(
                202,
                json!({
                    "approval_event_sequence": approval.approval_event_sequence,
                    "job_id": job_id,
                    "state": "APPROVED",
                }),
            ),
            Err(error) => workflow_error(error),
        }
    }
}

#[derive(Clone, Copy)]
enum CanonicalAgentActor {
    Workflow,
    RequestingHuman,
}

fn commit_agent_gateway_event(
    custody: &CanonicalCustody,
    context: &AuthorizedCoreApiContext,
    command: &ApiCommandFields,
    event_type: &str,
    payload: &serde_json::Value,
    visibility_label: &str,
    actor: CanonicalAgentActor,
) -> Result<trpg_shared_kernel::CanonicalCommittedEvent, HttpResponse> {
    let authority_contract_version = u64::try_from(context.authority_contract_version())
        .map_err(|_| agent_job_error(409, "AGENT_JOB_AUTHORITY_INVALID"))?;
    let expected_version = u64::try_from(command.expected_version)
        .map_err(|_| agent_job_error(400, "AGENT_JOB_EXPECTED_VERSION_INVALID"))?;
    let payload_json = serde_json::to_string(payload).map_err(|_| internal_error())?;
    let (authenticated_actor_id, authenticated_actor_role, authenticated_actor_origin) =
        match actor {
            CanonicalAgentActor::Workflow => (
                context.workflow_actor_id().to_owned(),
                context.workflow_actor_role().to_owned(),
                EventActorOriginWire::Workload {
                    role: "workflow_engine".to_owned(),
                },
            ),
            CanonicalAgentActor::RequestingHuman => (
                context.actor_id().to_owned(),
                context.actor_role().to_owned(),
                EventActorOriginWire::UserSession {
                    session_id: context.authentication_reference().to_owned(),
                },
            ),
        };
    let provenance_kind = if context.actor_role() == "human_keeper" {
        "human_keeper_statement"
    } else {
        "user_statement"
    };
    let commit = CanonicalCommitRequest {
        commit_id: format!("{}_{}", context.campaign_id(), command.command_id),
        campaign_id: context.campaign_id().to_owned(),
        idempotency_key: command.idempotency_key.clone(),
        expected_version,
        command_id: command.command_id.clone(),
        authenticated_actor_id,
        authenticated_actor_role,
        authenticated_actor_origin,
        authority_mode: context.authority_mode().to_owned(),
        authority_contract_version,
        authority_contract_id: context.authority_contract_id().to_owned(),
        authority_owner: context.authority_owner().to_owned(),
        visibility_label: visibility_label.to_owned(),
        visibility_subject: "not_applicable".to_owned(),
        data_subject_id: "not_applicable".to_owned(),
        provenance_kind: provenance_kind.to_owned(),
        provenance_reference: command.command_id.clone(),
        provenance_recorded_by: context.actor_id().to_owned(),
        correlation_id: command.correlation_id.clone(),
        causation_id: command.causation_id.clone(),
        trace_id: command.trace_id.clone(),
        events: vec![CanonicalCommitEvent {
            event_type: event_type.to_owned(),
            payload_json: payload_json.clone(),
        }],
        audit: context.policy_audit().clone(),
    };
    let receipt = custody
        .canonical
        .commit(&commit)
        .and_then(|receipt| {
            custody.canonical.verify_receipt(&commit, &receipt)?;
            Ok(receipt)
        })
        .map_err(|_| agent_job_error(409, "AGENT_JOB_CANONICAL_COMMIT_FAILED"))?;
    let expected_stream_version = expected_version
        .checked_add(1)
        .ok_or_else(|| agent_job_error(409, "AGENT_JOB_EXPECTED_VERSION_INVALID"))?;
    if receipt.first_stream_version != expected_stream_version
        || receipt.last_stream_version != expected_stream_version
    {
        return Err(agent_job_error(
            409,
            "AGENT_JOB_CANONICAL_RECEIPT_INVALID",
        ));
    }
    receipt
        .events
        .into_iter()
        .next()
        .filter(|event| event.stream_version == expected_stream_version)
        .filter(|event| event.event_type == event_type)
        .filter(|event| {
            serde_json::from_str::<serde_json::Value>(&event.payload_json).ok() == Some(payload.clone())
        })
        .ok_or_else(|| agent_job_error(409, "AGENT_JOB_CANONICAL_RECEIPT_INVALID"))
}

fn workflow_error(error: WorkflowStoreError) -> HttpResponse {
    match error {
        WorkflowStoreError::NotFound => agent_job_error(404, "AGENT_JOB_NOT_FOUND"),
        WorkflowStoreError::VersionConflict { .. }
        | WorkflowStoreError::StateConflict
        | WorkflowStoreError::IdempotencyConflict => {
            agent_job_error(409, "AGENT_JOB_CONFLICT")
        }
        WorkflowStoreError::Validation(_) => agent_job_error(400, "AGENT_JOB_REQUEST_INVALID"),
        WorkflowStoreError::Configuration(_)
        | WorkflowStoreError::Connection
        | WorkflowStoreError::Migration
        | WorkflowStoreError::Database(_)
        | WorkflowStoreError::IntegrityViolation(_) => {
            agent_job_error(503, "AGENT_JOB_GATEWAY_UNAVAILABLE")
        }
    }
}

fn agent_job_error(status: u16, code: &str) -> HttpResponse {
    HttpResponse::json(status, json!({"error": code}))
}

#[cfg(test)]
mod agent_job_gateway_configuration_tests {
    use super::*;

    fn route() -> AgentJobRouteConfiguration {
        AgentJobRouteConfiguration {
            provider_id: "provider_ar09".to_owned(),
            provider_type: "cloud".to_owned(),
            model_id: "publisher/model-ar09".to_owned(),
            model_artifact_sha256:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
            route_authorization_event_id: "route_authorization_ar09".to_owned(),
        }
    }

    #[test]
    fn production_agent_route_accepts_real_model_identifiers() {
        assert_eq!(route().validate(), Ok(()));
    }

    #[test]
    fn production_agent_route_is_exact_and_fail_closed() {
        let mut invalid_provider = route();
        invalid_provider.provider_type = "local_openai_compatible".to_owned();
        assert_eq!(
            invalid_provider.validate(),
            Err("TRPG_MODEL_PROVIDER_TYPE_INVALID".to_owned())
        );

        let mut uppercase_digest = route();
        uppercase_digest.model_artifact_sha256 =
            "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .to_owned();
        assert_eq!(
            uppercase_digest.validate(),
            Err("TRPG_MODEL_ARTIFACT_SHA256_INVALID".to_owned())
        );
    }
}
