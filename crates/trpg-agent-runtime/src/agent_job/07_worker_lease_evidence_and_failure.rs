impl AgentJobWorker {
    async fn refresh_provider_lease(
        &self,
        job: &DurableAgentJob,
        cancellation: &ProviderCancellation,
        started_unix_ms: i64,
        started: Instant,
    ) -> AgentJobResult<()> {
        let now_unix_ms = observed_now(started_unix_ms, started)?;
        if now_unix_ms >= job.deadline_unix_ms {
            cancellation.cancel();
            return Err(AgentJobError::terminal("AGENT_JOB_DEADLINE_EXCEEDED"));
        }
        if self.repository.cancellation_requested(&job.job_id).await? {
            cancellation.cancel();
            return Err(AgentJobError::terminal("AGENT_JOB_CANCELLED"));
        }
        let lease_duration_ms = duration_millis_i64(self.configuration.lease_duration)?;
        let claim_owner = job
            .claim_owner
            .as_deref()
            .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?;
        let claim_token = job
            .claim_token
            .as_deref()
            .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?;
        if !self
            .repository
            .heartbeat(
                &job.job_id,
                claim_owner,
                claim_token,
                now_unix_ms,
                lease_duration_ms,
            )
            .await?
        {
            cancellation.cancel();
            return Err(AgentJobError::retryable("AGENT_JOB_LEASE_LOST"));
        }
        Ok(())
    }

    async fn ensure_active(&self, job: &DurableAgentJob, now_unix_ms: i64) -> AgentJobResult<()> {
        if now_unix_ms >= job.deadline_unix_ms {
            return Err(AgentJobError::terminal("AGENT_JOB_DEADLINE_EXCEEDED"));
        }
        if self.repository.cancellation_requested(&job.job_id).await? {
            return Err(AgentJobError::terminal("AGENT_JOB_CANCELLED"));
        }
        Ok(())
    }

    fn validate_provider_binding(&self, job: &DurableAgentJob) -> AgentJobResult<()> {
        let route = self.provider.startup_route_snapshot();
        if self.provider.provider_id().as_str() != job.provider_id
            || provider_type_name(self.provider.provider_type()) != job.provider_type
            || self.provider.model_id() != job.model_id
            || self.provider.model_artifact_sha256() != job.model_artifact_sha256
        {
            return Err(AgentJobError::terminal(
                "AGENT_JOB_PROVIDER_BINDING_MISMATCH",
            ));
        }
        self.validate_executed_route(job, &route, ModelOperation::CapabilityProbe)
    }

    fn validate_executed_route(
        &self,
        job: &DurableAgentJob,
        route: &ExecutedModelRouteSnapshot,
        operation: ModelOperation,
    ) -> AgentJobResult<()> {
        if route.route_authorization_event_id.as_str() != job.route_authorization_event_id
            || route.provider_id.as_str() != job.provider_id
            || provider_type_name(route.provider_type) != job.provider_type
            || route.model_id != job.model_id
            || route.operation != operation
            || route.fallback_policy != "none_no_automatic_fallback"
            || route.privacy_boundary != "explicit_route_authorization_event"
        {
            return Err(AgentJobError::terminal(
                "AGENT_JOB_PROVIDER_BINDING_MISMATCH",
            ));
        }
        Ok(())
    }

    fn ensure_local_model_certification(&self, job: &DurableAgentJob) -> AgentJobResult<()> {
        if job.authority_mode == "AI_KP" && job.provider_type != "cloud" {
            self.local_certification
                .as_ref()
                .ok_or_else(|| AgentJobError::terminal("LOCAL_MODEL_LEVEL_4_REQUIRED"))?
                .ensure_ai_keeper(self.provider.as_ref())?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn append_evidence(
        &self,
        job: &DurableAgentJob,
        phase: &str,
        prompt_template_hash: &str,
        tool_schema_hash: &str,
        retrieval_hash: &str,
        input_hash: &str,
        output_hash: &str,
        input_tokens: i64,
        output_tokens: i64,
        latency_ms: i64,
        tool_call_count: i32,
        linked_event_sequences: &[i64],
        now_unix_ms: i64,
    ) -> AgentJobResult<()> {
        let retention_until_unix_ms = now_unix_ms
            .checked_add(EVIDENCE_RETENTION_MS)
            .ok_or_else(|| AgentJobError::terminal("AGENT_EVIDENCE_RETENTION_INVALID"))?;
        let visibility_scope: VisibilityScope = serde_json::from_str(&job.visibility_scope_json)
            .map_err(|_| AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"))?;
        self.repository
            .append_evidence(&AgentJobEvidenceDraft {
                job_id: job.job_id.clone(),
                attempt: job.attempt,
                phase: phase.to_owned(),
                model_id: job.model_id.clone(),
                runtime_version: AGENT_RUNTIME_VERSION.to_owned(),
                prompt_template_hash: prompt_template_hash.to_owned(),
                tool_schema_hash: tool_schema_hash.to_owned(),
                retrieval_hash: retrieval_hash.to_owned(),
                input_hash: input_hash.to_owned(),
                output_hash: output_hash.to_owned(),
                input_tokens,
                output_tokens,
                latency_ms,
                tool_call_count,
                linked_event_sequences: linked_event_sequences.to_vec(),
                visibility_label: visibility_scope.output_label,
                retention_until_unix_ms,
            })
            .await
    }

    async fn fail_job(
        &self,
        job: DurableAgentJob,
        error: AgentJobError,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        let target = if error.is_retryable() && job.attempt < self.configuration.max_attempts {
            WorkflowState::RetryableFailed
        } else {
            WorkflowState::TerminalFailed
        };
        let next_attempt_at = if target == WorkflowState::RetryableFailed {
            Some(
                now_unix_ms
                    .checked_add(retry_delay_ms(job.attempt))
                    .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_RETRY_INVALID"))?,
            )
        } else {
            None
        };
        let current_state = if job.state == WorkflowState::Claimed {
            WorkflowState::Claimed
        } else {
            job.state
        };
        if agent_state_can_fail(current_state) {
            let failed = self
                .repository
                .transition(
                    &self
                        .transition_draft(
                            &job,
                            current_state,
                            target,
                            "failure",
                            None,
                            None,
                            None,
                            Some(error.code()),
                            now_unix_ms,
                        )?
                        .with_next_attempt(next_attempt_at),
                )
                .await?;
            let _ = self
                .append_evidence(
                    &failed,
                    "failed",
                    EMPTY_SHA256,
                    EMPTY_SHA256,
                    EMPTY_SHA256,
                    EMPTY_SHA256,
                    EMPTY_SHA256,
                    0,
                    0,
                    0,
                    0,
                    &[],
                    now_unix_ms,
                )
                .await;
        }
        Ok(if target == WorkflowState::RetryableFailed {
            AgentJobOutcome::RetryScheduled {
                job_id: job.job_id,
                error_code: error.code(),
            }
        } else {
            AgentJobOutcome::TerminalFailure {
                job_id: job.job_id,
                error_code: error.code(),
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn transition_draft(
        &self,
        job: &DurableAgentJob,
        from_state: WorkflowState,
        to_state: WorkflowState,
        phase: &str,
        decision_json: Option<String>,
        tool_result_json: Option<String>,
        linked_event_sequences: Option<Vec<i64>>,
        error_code: Option<&'static str>,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobTransitionDraft> {
        Ok(AgentJobTransitionDraft {
            job_id: job.job_id.clone(),
            claim_owner: job
                .claim_owner
                .clone()
                .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?,
            claim_token: job
                .claim_token
                .clone()
                .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?,
            expected_version: job.version,
            from_state,
            to_state,
            idempotency_key: format!("{}:attempt:{}:{}", job.idempotency_key, job.attempt, phase),
            correlation_id: job.job_id.clone(),
            causation_id: format!("agent-job-input-{}", job.input_event_sequence),
            decision_json,
            tool_result_json,
            linked_event_sequences,
            error_code: error_code.map(str::to_owned),
            next_attempt_at_unix_ms: None,
            now_unix_ms,
        })
    }
}
