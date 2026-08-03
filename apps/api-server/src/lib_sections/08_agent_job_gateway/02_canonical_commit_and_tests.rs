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
    let receipt = if event_type == "AgentJobRequested" {
        custody
            .runtime
            .lock()
            .map_err(|_| internal_error())?
            .block_on(custody.store.commit_agent_job_request(&commit, payload))
            .map_err(|_| agent_job_error(409, "AGENT_JOB_CANONICAL_COMMIT_FAILED"))?
    } else {
        custody
            .canonical
            .commit(&commit)
            .and_then(|receipt| {
                custody.canonical.verify_receipt(&commit, &receipt)?;
                Ok(receipt)
            })
            .map_err(|_| agent_job_error(409, "AGENT_JOB_CANONICAL_COMMIT_FAILED"))?
    };
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

fn ai_kp_agent_job_request_role_allowed(actor_role: &str) -> bool {
    matches!(
        actor_role,
        "investigator" | "campaign_owner" | "server_owner"
    )
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

    #[test]
    fn ai_kp_agent_job_role_gate_allows_players_and_denies_spectators() {
        for role in ["investigator", "campaign_owner", "server_owner"] {
            assert!(ai_kp_agent_job_request_role_allowed(role));
        }
        for role in [
            "spectator",
            "human_keeper",
            "moderator",
            "ai_keeper",
            "workflow",
        ] {
            assert!(!ai_kp_agent_job_request_role_allowed(role));
        }
    }
}
