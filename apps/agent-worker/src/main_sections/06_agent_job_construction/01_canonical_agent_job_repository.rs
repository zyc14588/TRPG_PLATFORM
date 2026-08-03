#[derive(Clone)]
struct CanonicalAgentJobRepository {
    workflow: DurableWorkflowStore,
    canonical: PostgresCanonicalStore,
}

impl CanonicalAgentJobRepository {
    fn new(workflow: DurableWorkflowStore, canonical: PostgresCanonicalStore) -> Self {
        Self {
            workflow,
            canonical,
        }
    }
}

#[trpg_agent_runtime::repository_async_trait]
impl AgentJobRepository for CanonicalAgentJobRepository {
    async fn load(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentJob>> {
        self.workflow
            .load_agent_job(job_id)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn claim_due(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<Option<DurableAgentJob>> {
        self.workflow
            .claim_due_agent_job(claim_owner, now_unix_ms, lease_duration_ms)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn transition(
        &self,
        draft: &AgentJobTransitionDraft,
    ) -> AgentJobResult<DurableAgentJob> {
        self.workflow
            .transition_agent_job(draft)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn heartbeat(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<bool> {
        self.workflow
            .heartbeat_agent_job(
                job_id,
                claim_owner,
                claim_token,
                now_unix_ms,
                lease_duration_ms,
            )
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn cancellation_requested(&self, job_id: &str) -> AgentJobResult<bool> {
        self.workflow
            .agent_job_cancellation_requested(job_id)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn load_authority(
        &self,
        campaign_id: &str,
    ) -> AgentJobResult<DurableAgentAuthoritySnapshot> {
        self.workflow
            .load_agent_authority_snapshot(campaign_id)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn load_context(&self, job_id: &str) -> AgentJobResult<DurableAgentContextSnapshot> {
        let job = self
            .workflow
            .load_agent_job(job_id)
            .await
            .map_err(map_agent_workflow_error)?
            .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_NOT_FOUND"))?;
        let mut context = self
            .workflow
            .load_agent_job_context(job_id)
            .await
            .map_err(map_agent_workflow_error)?;
        let after_sequence = job
            .input_event_sequence
            .checked_sub(1)
            .ok_or_else(|| AgentJobError::terminal("AGENT_CANONICAL_INPUT_INVALID"))?;
        let mut events = self
            .canonical
            .load_replay_page(&job.campaign_id, after_sequence, 1)
            .await
            .map_err(map_agent_canonical_context_error)?;
        let event = events
            .pop()
            .ok_or_else(|| AgentJobError::terminal("AGENT_CANONICAL_INPUT_MISSING"))?;
        let visibility_scope: serde_json::Value =
            serde_json::from_str(&job.visibility_scope_json)
                .map_err(|_| AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"))?;
        let visibility_allowed = visibility_scope
            .get("allowed_labels")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|labels| {
                labels
                    .iter()
                    .any(|label| label.as_str() == Some(&event.visibility_label))
            });
        let payload_matches = event
            .payload
            .get("job_id")
            .and_then(serde_json::Value::as_str)
            == Some(job.job_id.as_str())
            && event
                .payload
                .get("authority_contract_id")
                .and_then(serde_json::Value::as_str)
                == Some(job.authority_contract_id.as_str())
            && event
                .payload
                .get("route_authorization_event_id")
                .and_then(serde_json::Value::as_str)
                == Some(job.route_authorization_event_id.as_str())
            && event
                .payload
                .get("rag_snapshot_id")
                .and_then(serde_json::Value::as_str)
                == Some(job.rag_snapshot_id.as_str())
            && event
                .payload
                .get("provider_id")
                .and_then(serde_json::Value::as_str)
                == Some(job.provider_id.as_str())
            && event
                .payload
                .get("model_id")
                .and_then(serde_json::Value::as_str)
                == Some(job.model_id.as_str())
            && event
                .payload
                .get("agent_kind")
                .and_then(serde_json::Value::as_str)
                == Some(job.agent_kind.as_str())
            && event
                .payload
                .get("authority_mode")
                .and_then(serde_json::Value::as_str)
                == Some(job.authority_mode.as_str())
            && event
                .payload
                .get("authority_contract_version")
                .and_then(serde_json::Value::as_i64)
                == Some(job.authority_contract_version)
            && event.payload.get("visibility_scope") == Some(&visibility_scope)
            && event.payload.get("input").is_some();
        if event.sequence != job.input_event_sequence
            || event.stream_version != job.input_stream_version
            || event.expected_version.checked_add(1) != Some(event.stream_version)
            || event.stream_id != job.input_stream_id
            || event.event_type != "AgentJobRequested"
            || event.campaign_id != job.campaign_id
            || event.authority_mode != job.authority_mode.to_ascii_lowercase()
            || event.authority_contract_id != job.authority_contract_id
            || event.authority_contract_version != job.authority_contract_version
            || event.authority_owner
                != self
                    .workflow
                    .load_agent_authority_snapshot(&job.campaign_id)
                    .await
                    .map_err(map_agent_workflow_error)?
                    .authority_owner
            || event.resource_type != "agent_job"
            || event.resource_id != job.job_id
            || event.integrity_status != "verified_hmac"
            || event.request_hash_source != "formal_commit"
            || event.event_integrity_hash.is_none()
            || !visibility_allowed
            || !payload_matches
        {
            return Err(AgentJobError::terminal(
                "AGENT_CANONICAL_INPUT_BINDING_MISMATCH",
            ));
        }
        context.input_payload_json = serde_json::to_string(&event.payload)
            .map_err(|_| AgentJobError::terminal("AGENT_CANONICAL_INPUT_INVALID"))?;
        Ok(context)
    }

    async fn load_approval(
        &self,
        job_id: &str,
    ) -> AgentJobResult<Option<DurableAgentApproval>> {
        self.workflow
            .load_agent_job_approval(job_id)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn append_evidence(&self, draft: &AgentJobEvidenceDraft) -> AgentJobResult<()> {
        self.workflow
            .append_agent_job_evidence(draft)
            .await
            .map_err(map_agent_workflow_error)
    }
}
