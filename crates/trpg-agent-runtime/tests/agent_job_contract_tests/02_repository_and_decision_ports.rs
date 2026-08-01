#[async_trait]
impl AgentJobRepository for MemoryRepository {
    async fn load(&self, job_id: &str) -> Result<Option<DurableAgentJob>, AgentJobError> {
        let state = self.state.lock().unwrap();
        Ok((state.job.job_id == job_id).then(|| state.job.clone()))
    }

    async fn claim_due(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> Result<Option<DurableAgentJob>, AgentJobError> {
        let mut state = self.state.lock().unwrap();
        if matches!(
            state.job.state,
            WorkflowState::Completed | WorkflowState::TerminalFailed
        ) || (state.job.state == WorkflowState::RetryableFailed
            && state
                .job
                .next_attempt_at_unix_ms
                .is_some_and(|next| next > now_unix_ms))
            || (state.job.state == WorkflowState::AwaitingTool
                && state.job.authority_mode == "HUMAN_KP"
                && state.approval.is_none()
                && !state.cancellation_requested
                && state.job.deadline_unix_ms > now_unix_ms)
        {
            return Ok(None);
        }
        let resume = if state.job.state == WorkflowState::RetryableFailed {
            state
                .job
                .resume_state
                .unwrap_or(WorkflowState::RetryableFailed)
        } else {
            state.job.state
        };
        state.job.state = WorkflowState::Claimed;
        state.job.resume_state = Some(resume);
        state.job.version += 1;
        state.job.attempt += 1;
        state.job.claim_owner = Some(claim_owner.to_owned());
        state.job.claim_token = Some(format!("claim_{}", state.job.attempt));
        state.job.lease_expires_at_unix_ms = Some(now_unix_ms.saturating_add(lease_duration_ms));
        Ok(Some(state.job.clone()))
    }

    async fn transition(
        &self,
        draft: &AgentJobTransitionDraft,
    ) -> Result<DurableAgentJob, AgentJobError> {
        let mut state = self.state.lock().unwrap();
        if let Some(existing) = state.transitions.get(&draft.idempotency_key) {
            return Ok(existing.clone());
        }
        if state.job.job_id != draft.job_id
            || state.job.version != draft.expected_version
            || state.job.state != draft.from_state
            || state.job.claim_owner.as_deref() != Some(draft.claim_owner.as_str())
            || state.job.claim_token.as_deref() != Some(draft.claim_token.as_str())
        {
            return Err(AgentJobError::retryable("MEMORY_REPOSITORY_CAS_CONFLICT"));
        }
        state.job.state = draft.to_state;
        state.job.resume_state = if draft.to_state == WorkflowState::RetryableFailed {
            Some(draft.from_state)
        } else {
            None
        };
        state.job.version += 1;
        if let Some(decision) = &draft.decision_json {
            state.job.decision_json = Some(decision.clone());
        }
        if let Some(tool_result) = &draft.tool_result_json {
            state.job.tool_result_json = Some(tool_result.clone());
        }
        if matches!(
            draft.to_state,
            WorkflowState::Completed | WorkflowState::TerminalFailed
        ) {
            state.job.decision_json = None;
            state.job.tool_result_json = None;
        }
        if let Some(sequences) = &draft.linked_event_sequences {
            state.job.linked_event_sequences = sequences.clone();
        }
        state.job.error_code = draft.error_code.clone();
        state.job.next_attempt_at_unix_ms = draft.next_attempt_at_unix_ms;
        if matches!(
            draft.to_state,
            WorkflowState::Completed
                | WorkflowState::RetryableFailed
                | WorkflowState::TerminalFailed
        ) {
            state.job.claim_owner = None;
            state.job.claim_token = None;
            state.job.lease_expires_at_unix_ms = None;
        }
        let updated = state.job.clone();
        state
            .transitions
            .insert(draft.idempotency_key.clone(), updated.clone());
        Ok(updated)
    }

    async fn heartbeat(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> Result<bool, AgentJobError> {
        let mut state = self.state.lock().unwrap();
        let matches = state.job.job_id == job_id
            && state.job.claim_owner.as_deref() == Some(claim_owner)
            && state.job.claim_token.as_deref() == Some(claim_token);
        if matches {
            state.job.heartbeat_at_unix_ms = Some(now_unix_ms);
            state.job.lease_expires_at_unix_ms =
                Some(now_unix_ms.saturating_add(lease_duration_ms));
        }
        Ok(matches)
    }

    async fn cancellation_requested(&self, _job_id: &str) -> Result<bool, AgentJobError> {
        Ok(self.state.lock().unwrap().cancellation_requested)
    }

    async fn load_authority(
        &self,
        _campaign_id: &str,
    ) -> Result<DurableAgentAuthoritySnapshot, AgentJobError> {
        Ok(self.state.lock().unwrap().authority.clone())
    }

    async fn load_context(
        &self,
        _job_id: &str,
    ) -> Result<DurableAgentContextSnapshot, AgentJobError> {
        Ok(self.state.lock().unwrap().context.clone())
    }

    async fn load_approval(
        &self,
        _job_id: &str,
    ) -> Result<Option<DurableAgentApproval>, AgentJobError> {
        Ok(self.state.lock().unwrap().approval.clone())
    }

    async fn append_evidence(&self, draft: &AgentJobEvidenceDraft) -> Result<(), AgentJobError> {
        let mut state = self.state.lock().unwrap();
        let key = (draft.attempt, draft.phase.clone());
        if let Some(existing) = state.evidence.get(&key) {
            if existing != draft {
                return Err(AgentJobError::terminal("EVIDENCE_IDEMPOTENCY_CONFLICT"));
            }
            return Ok(());
        }
        state.evidence.insert(key, draft.clone());
        Ok(())
    }
}

#[derive(Default)]
struct CountingToolPort {
    receipts: Mutex<HashMap<String, AgentJobToolResult>>,
    executions: AtomicU64,
}

#[async_trait]
impl AgentJobToolPort for CountingToolPort {
    async fn execute(
        &self,
        _job: &DurableAgentJob,
        _call: &AgentJobToolCall,
        idempotency_key: &str,
        _now_unix_ms: i64,
    ) -> Result<AgentJobToolResult, AgentJobError> {
        let mut receipts = self.receipts.lock().unwrap();
        if let Some(receipt) = receipts.get(idempotency_key) {
            return Ok(receipt.clone());
        }
        self.executions.fetch_add(1, Ordering::SeqCst);
        let receipt = AgentJobToolResult {
            execution_id: "tool_execution_ar09".to_owned(),
            result: serde_json::json!({}),
            result_hash: "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
                .to_owned(),
        };
        receipts.insert(idempotency_key.to_owned(), receipt.clone());
        Ok(receipt)
    }
}

#[derive(Default)]
struct CountingDecisionPort {
    receipts: Mutex<HashMap<String, AgentJobCommitReceipt>>,
    next_sequence: AtomicI64,
    canonical_events: AtomicU64,
    authorization_checks: AtomicU64,
    deny_authorization: bool,
    fail_after_first_commit: AtomicBool,
}

impl CountingDecisionPort {
    fn crash_after_first_commit() -> Self {
        Self {
            fail_after_first_commit: AtomicBool::new(true),
            ..Self::default()
        }
    }

    fn deny_authorization() -> Self {
        Self {
            deny_authorization: true,
            ..Self::default()
        }
    }
}

#[async_trait]
impl AgentJobDecisionPort for CountingDecisionPort {
    async fn authorize_execution(
        &self,
        _job: &DurableAgentJob,
        _now_unix_ms: i64,
    ) -> Result<(), AgentJobError> {
        self.authorization_checks.fetch_add(1, Ordering::SeqCst);
        if self.deny_authorization {
            Err(AgentJobError::terminal(
                "AGENT_EXECUTION_AUTHORIZATION_DENIED",
            ))
        } else {
            Ok(())
        }
    }

    async fn commit_ai_decision(
        &self,
        job: &DurableAgentJob,
        _decision: &AgentStructuredDecision,
        _tool_result: Option<&AgentJobToolResult>,
        _now_unix_ms: i64,
    ) -> Result<AgentJobCommitReceipt, AgentJobError> {
        let mut receipts = self.receipts.lock().unwrap();
        if let Some(receipt) = receipts.get(&job.idempotency_key) {
            return Ok(receipt.clone());
        }
        let sequence = self.next_sequence.fetch_add(1, Ordering::SeqCst) + 1;
        self.canonical_events.fetch_add(1, Ordering::SeqCst);
        let receipt = AgentJobCommitReceipt {
            event_sequences: vec![sequence],
        };
        receipts.insert(job.idempotency_key.clone(), receipt.clone());
        if self.fail_after_first_commit.swap(false, Ordering::SeqCst) {
            return Err(AgentJobError::retryable(
                "INJECTED_CRASH_AFTER_CANONICAL_COMMIT",
            ));
        }
        Ok(receipt)
    }
}
