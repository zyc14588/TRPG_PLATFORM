#[async_trait]
impl AgentJobRepository for FileAgentJobRepository {
    async fn load(&self, job_id: &str) -> Result<Option<DurableAgentJob>, AgentJobError> {
        let state = self.load_state();
        Ok((state.job_id == job_id).then(|| self.durable_job(&state)))
    }

    async fn claim_due(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> Result<Option<DurableAgentJob>, AgentJobError> {
        crash_boundary("claim_before");
        let mut state = self.load_state();
        let current = parse_state(&state.state);
        let due = match current {
            WorkflowState::Requested | WorkflowState::RetryableFailed => true,
            WorkflowState::Claimed
            | WorkflowState::AgentRunning
            | WorkflowState::AwaitingTool
            | WorkflowState::Committing => state
                .lease_expires_at_unix_ms
                .is_some_and(|expires_at| expires_at <= now_unix_ms),
            _ => false,
        };
        if !due {
            return Ok(None);
        }
        let resume = match current {
            WorkflowState::Requested => WorkflowState::Requested,
            WorkflowState::Claimed => state
                .resume_state
                .as_deref()
                .map(parse_state)
                .unwrap_or(WorkflowState::Requested),
            WorkflowState::RetryableFailed => state
                .resume_state
                .as_deref()
                .map(parse_state)
                .unwrap_or(WorkflowState::AgentRunning),
            active => active,
        };
        state.attempt += 1;
        state.version += 1;
        state.state = WorkflowState::Claimed.as_str().to_owned();
        state.resume_state = Some(resume.as_str().to_owned());
        state.claim_owner = Some(claim_owner.to_owned());
        state.claim_token = Some(format!("claim-token-{}", state.attempt));
        state.heartbeat_at_unix_ms = Some(now_unix_ms);
        state.lease_expires_at_unix_ms = Some(
            now_unix_ms
                .checked_add(lease_duration_ms)
                .expect("test lease must not overflow"),
        );
        self.store_state(&state);
        crash_boundary("claim_after");
        Ok(Some(self.durable_job(&state)))
    }

    async fn transition(
        &self,
        draft: &AgentJobTransitionDraft,
    ) -> Result<DurableAgentJob, AgentJobError> {
        if draft.from_state == WorkflowState::AgentRunning
            && draft.to_state == WorkflowState::AwaitingTool
        {
            crash_boundary("provider_after");
        }
        if draft.from_state == WorkflowState::AwaitingTool
            && draft.to_state == WorkflowState::Committing
        {
            crash_boundary("tool_after");
        }
        let mut state = self.load_state();
        if state.job_id != draft.job_id
            || parse_state(&state.state) != draft.from_state
            || state.version != draft.expected_version
            || state.claim_owner.as_deref() != Some(draft.claim_owner.as_str())
            || state.claim_token.as_deref() != Some(draft.claim_token.as_str())
        {
            return Err(AgentJobError::retryable("AGENT_JOB_CAS_CONFLICT"));
        }
        state.version += 1;
        state.state = draft.to_state.as_str().to_owned();
        state.resume_state = if draft.to_state == WorkflowState::RetryableFailed {
            Some(draft.from_state.as_str().to_owned())
        } else {
            None
        };
        if let Some(decision) = &draft.decision_json {
            state.decision_json = Some(decision.clone());
        }
        if let Some(tool_result) = &draft.tool_result_json {
            state.tool_result_json = Some(tool_result.clone());
        }
        if let Some(linked) = &draft.linked_event_sequences {
            state.linked_event_sequences = linked.clone();
        }
        state.error_code = draft.error_code.clone();
        if matches!(
            draft.to_state,
            WorkflowState::Completed
                | WorkflowState::RetryableFailed
                | WorkflowState::TerminalFailed
        ) {
            state.claim_owner = None;
            state.claim_token = None;
            state.lease_expires_at_unix_ms = None;
            state.heartbeat_at_unix_ms = None;
        }
        if matches!(
            draft.to_state,
            WorkflowState::Completed | WorkflowState::TerminalFailed
        ) {
            state.decision_json = None;
            state.tool_result_json = None;
        }
        self.store_state(&state);
        Ok(self.durable_job(&state))
    }

    async fn heartbeat(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> Result<bool, AgentJobError> {
        let mut state = self.load_state();
        if state.job_id != job_id
            || state.claim_owner.as_deref() != Some(claim_owner)
            || state.claim_token.as_deref() != Some(claim_token)
        {
            return Ok(false);
        }
        state.heartbeat_at_unix_ms = Some(now_unix_ms);
        state.lease_expires_at_unix_ms = Some(
            now_unix_ms
                .checked_add(lease_duration_ms)
                .expect("test lease must not overflow"),
        );
        self.store_state(&state);
        Ok(true)
    }

    async fn cancellation_requested(&self, job_id: &str) -> Result<bool, AgentJobError> {
        Ok(self.load_state().job_id != job_id)
    }

    async fn load_authority(
        &self,
        campaign_id: &str,
    ) -> Result<DurableAgentAuthoritySnapshot, AgentJobError> {
        assert_eq!(campaign_id, "campaign_ar09_kill9");
        Ok(DurableAgentAuthoritySnapshot {
            contract_id: "authority_ar09_kill9".to_owned(),
            campaign_id: campaign_id.to_owned(),
            authority_mode: "AI_KP".to_owned(),
            authority_owner: "ai_keeper_ar09_kill9".to_owned(),
            contract_version: 1,
            prompt_version: "prompt_v1".to_owned(),
            agent_pack_version: "agent_pack_v1".to_owned(),
            tool_schema_version: "tool_v1".to_owned(),
            model_route_snapshot: "route_snapshot_ar09_kill9".to_owned(),
        })
    }

    async fn load_context(
        &self,
        job_id: &str,
    ) -> Result<DurableAgentContextSnapshot, AgentJobError> {
        assert_eq!(self.load_state().job_id, job_id);
        Ok(DurableAgentContextSnapshot {
            input_payload_json: r#"{"event":"npc_turn_requested"}"#.to_owned(),
            chunks: vec![DurableAgentContextChunk {
                chunk_id: "chunk_ar09_kill9".to_owned(),
                source_event_sequence: 9,
                visibility_label: "public".to_owned(),
                visibility_subject: None,
                fact_provenance_json: r#"{"kind":"tool_result"}"#.to_owned(),
                chunk_hash: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_owned(),
                content: "The corridor is quiet.".to_owned(),
            }],
        })
    }

    async fn load_approval(
        &self,
        _job_id: &str,
    ) -> Result<Option<DurableAgentApproval>, AgentJobError> {
        Ok(None)
    }

    async fn append_evidence(&self, draft: &AgentJobEvidenceDraft) -> Result<(), AgentJobError> {
        let mut state = self.load_state();
        let key = format!("{}:{}", draft.attempt, draft.phase);
        let encoded = serde_json::to_string(&json!({
            "model_id": draft.model_id,
            "runtime_version": draft.runtime_version,
            "prompt_template_hash": draft.prompt_template_hash,
            "tool_schema_hash": draft.tool_schema_hash,
            "retrieval_hash": draft.retrieval_hash,
            "input_hash": draft.input_hash,
            "output_hash": draft.output_hash,
            "input_tokens": draft.input_tokens,
            "output_tokens": draft.output_tokens,
            "latency_ms": draft.latency_ms,
            "tool_call_count": draft.tool_call_count,
            "linked_event_sequences": draft.linked_event_sequences,
            "visibility_label": draft.visibility_label,
            "retention_until_unix_ms": draft.retention_until_unix_ms,
        }))
        .expect("evidence must serialize");
        if let Some(existing) = state.evidence.get(&key) {
            if existing != &encoded {
                return Err(AgentJobError::terminal("EVIDENCE_IDEMPOTENCY_CONFLICT"));
            }
            return Ok(());
        }
        state.evidence.insert(key, encoded);
        self.store_state(&state);
        Ok(())
    }
}
