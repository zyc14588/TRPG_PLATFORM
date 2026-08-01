impl AgentJobWorker {
    pub fn new(
        repository: Arc<dyn AgentJobRepository>,
        provider: Arc<dyn ExecutableModelProvider>,
        tools: Arc<dyn AgentJobToolPort>,
        decisions: Arc<dyn AgentJobDecisionPort>,
        local_certification: Option<CertifiedLocalModel>,
        configuration: AgentJobExecutionConfig,
    ) -> AgentJobResult<Self> {
        configuration.validate()?;
        Ok(Self {
            repository,
            provider,
            tools,
            decisions,
            local_certification,
            configuration,
        })
    }

    pub async fn run_once(&self, now_unix_ms: i64) -> AgentJobResult<AgentJobOutcome> {
        let lease_duration_ms = duration_millis_i64(self.configuration.lease_duration)?;
        let Some(job) = self
            .repository
            .claim_due(
                &self.configuration.claim_owner,
                now_unix_ms,
                lease_duration_ms,
            )
            .await?
        else {
            return Ok(AgentJobOutcome::Idle);
        };
        match self.execute_claimed(job.clone(), now_unix_ms).await {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                let current = self
                    .repository
                    .load(&job.job_id)
                    .await?
                    .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_NOT_FOUND"))?;
                self.fail_job(current, error, now_unix_ms).await
            }
        }
    }

    async fn execute_claimed(
        &self,
        mut job: DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        self.ensure_active(&job, now_unix_ms).await?;
        if job.attempt > self.configuration.max_attempts {
            return Err(AgentJobError::terminal("AGENT_JOB_ATTEMPT_LIMIT_EXCEEDED"));
        }
        let authority = self.repository.load_authority(&job.campaign_id).await?;
        validate_authority_snapshot(&job, &authority)?;
        self.decisions
            .authorize_execution(&job, now_unix_ms)
            .await?;
        let resume_state = job.resume_state.unwrap_or(WorkflowState::Requested);
        match resume_state {
            WorkflowState::Committing => {
                job = self
                    .transition_claimed_to(&job, WorkflowState::Committing, now_unix_ms)
                    .await?;
                self.commit_persisted(job, now_unix_ms).await
            }
            WorkflowState::AwaitingTool => {
                job = self
                    .transition_claimed_to(&job, WorkflowState::AwaitingTool, now_unix_ms)
                    .await?;
                self.continue_persisted_decision(job, now_unix_ms).await
            }
            WorkflowState::Requested
            | WorkflowState::Claimed
            | WorkflowState::AgentRunning
            | WorkflowState::RetryableFailed => {
                job = self
                    .transition_claimed_to(&job, WorkflowState::AgentRunning, now_unix_ms)
                    .await?;
                self.run_model(job, now_unix_ms).await
            }
            _ => Err(AgentJobError::terminal("AGENT_JOB_RESUME_STATE_INVALID")),
        }
    }

    async fn transition_claimed_to(
        &self,
        job: &DurableAgentJob,
        target: WorkflowState,
        now_unix_ms: i64,
    ) -> AgentJobResult<DurableAgentJob> {
        self.repository
            .transition(&self.transition_draft(
                job,
                WorkflowState::Claimed,
                target,
                phase_id(target),
                None,
                None,
                None,
                None,
                now_unix_ms,
            )?)
            .await
    }

    async fn run_model(
        &self,
        mut job: DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        self.validate_provider_binding(&job)?;
        self.ensure_local_model_certification(&job)?;
        let empty_hash = EMPTY_SHA256.to_owned();
        self.append_evidence(
            &job,
            "authority",
            &empty_hash,
            &empty_hash,
            &empty_hash,
            &empty_hash,
            &empty_hash,
            0,
            0,
            0,
            0,
            &[],
            now_unix_ms,
        )
        .await?;

        let context = self.repository.load_context(&job.job_id).await?;
        let visibility_scope = validate_context_scope(&job, &context)?;
        let (request, hashes) = build_model_request(
            &job,
            &context,
            &visibility_scope,
            self.configuration.max_context_bytes,
        )?;
        self.append_evidence(
            &job,
            "context",
            &hashes.prompt_template_hash,
            &hashes.tool_schema_hash,
            &hashes.retrieval_hash,
            &hashes.input_hash,
            &empty_hash,
            0,
            0,
            0,
            0,
            &[],
            now_unix_ms,
        )
        .await?;

        let started = Instant::now();
        let cancellation = ProviderCancellation::default();
        let execution = self
            .execute_provider_with_lease(&job, &request, &cancellation, now_unix_ms)
            .await?;
        let latency_ms = i64::try_from(started.elapsed().as_millis())
            .map_err(|_| AgentJobError::terminal("AGENT_JOB_LATENCY_OVERFLOW"))?;
        if execution.output.usage.input_tokens > self.configuration.max_input_tokens
            || execution.output.usage.output_tokens > self.configuration.max_output_tokens
            || execution.output.tool_calls.len() > self.configuration.max_tool_calls
        {
            return Err(AgentJobError::terminal("AGENT_JOB_BUDGET_EXCEEDED"));
        }
        let decision = validate_structured_decision(
            execution.output.structured_output.as_ref(),
            &execution.output.tool_calls,
            self.configuration.max_tool_calls,
        )?;
        let injection =
            evaluate_prompt_injection(&context.input_payload_json, &decision.player_visible_text);
        if injection.detected {
            return Err(AgentJobError::terminal("PROMPT_INJECTION_DETECTED"));
        }
        let decision_json = serde_json::to_string(&decision)
            .map_err(|_| AgentJobError::terminal("AGENT_OUTPUT_SCHEMA_INVALID"))?;
        let output_hash = sha256_label(decision_json.as_bytes());
        self.append_evidence(
            &job,
            "provider",
            &hashes.prompt_template_hash,
            &hashes.tool_schema_hash,
            &hashes.retrieval_hash,
            &hashes.input_hash,
            &output_hash,
            i64::try_from(execution.output.usage.input_tokens)
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_BUDGET_EXCEEDED"))?,
            i64::try_from(execution.output.usage.output_tokens)
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_BUDGET_EXCEEDED"))?,
            latency_ms,
            i32::try_from(decision.tool.iter().count())
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_BUDGET_EXCEEDED"))?,
            &[],
            observed_now(now_unix_ms, started)?,
        )
        .await?;
        job = self
            .repository
            .transition(&self.transition_draft(
                &job,
                WorkflowState::AgentRunning,
                WorkflowState::AwaitingTool,
                "provider_output",
                Some(decision_json),
                None,
                None,
                None,
                observed_now(now_unix_ms, started)?,
            )?)
            .await?;
        self.continue_persisted_decision(job, observed_now(now_unix_ms, started)?)
            .await
    }

}
