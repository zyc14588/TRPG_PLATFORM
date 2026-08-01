impl AgentJobWorker {
    async fn continue_persisted_decision(
        &self,
        mut job: DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        self.ensure_active(&job, now_unix_ms).await?;
        let decision = persisted_decision(&job)?;
        if job.authority_mode == "HUMAN_KP" {
            let Some(approval) = self.repository.load_approval(&job.job_id).await? else {
                return Ok(AgentJobOutcome::AwaitingHumanApproval { job_id: job.job_id });
            };
            job = self
                .repository
                .transition(&self.transition_draft(
                    &job,
                    WorkflowState::AwaitingTool,
                    WorkflowState::Committing,
                    "human_approval",
                    None,
                    None,
                    Some(vec![approval.approval_event_sequence]),
                    None,
                    now_unix_ms,
                )?)
                .await?;
            return self
                .complete_human_approval(job, approval, now_unix_ms)
                .await;
        }

        let tool_result = if let Some(call) = decision.tool.as_ref() {
            if self.configuration.max_tool_loops < 1 {
                return Err(AgentJobError::terminal("AGENT_TOOL_LOOP_LIMIT_EXCEEDED"));
            }
            let tool = parse_agent_tool(&call.name)?;
            let request = ToolRequest::formal(AgentKind::AiKeeperOrchestrator, tool);
            let gate =
                evaluate_agent_tool_request(&trpg_shared_kernel::AuthorityMode::AiKp, &request);
            if !gate.tool_authorized || gate.draft_only || gate.requires_human_confirmation {
                return Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED"));
            }
            let result = self
                .tools
                .execute(
                    &job,
                    call,
                    &format!("{}:tool", job.idempotency_key),
                    now_unix_ms,
                )
                .await?;
            validate_tool_result(&result)?;
            let result_json = serde_json::to_string(&result)
                .map_err(|_| AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"))?;
            self.append_evidence(
                &job,
                "tool",
                EMPTY_SHA256,
                EMPTY_SHA256,
                EMPTY_SHA256,
                EMPTY_SHA256,
                &sha256_label(result_json.as_bytes()),
                0,
                0,
                0,
                1,
                &[],
                now_unix_ms,
            )
            .await?;
            Some((result, result_json))
        } else {
            None
        };
        job = self
            .repository
            .transition(&self.transition_draft(
                &job,
                WorkflowState::AwaitingTool,
                WorkflowState::Committing,
                "tool_complete",
                None,
                tool_result.as_ref().map(|(_, json)| json.clone()),
                None,
                None,
                now_unix_ms,
            )?)
            .await?;
        self.commit_persisted(job, now_unix_ms).await
    }

    async fn commit_persisted(
        &self,
        mut job: DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        self.ensure_active(&job, now_unix_ms).await?;
        let authority = self.repository.load_authority(&job.campaign_id).await?;
        validate_authority_snapshot(&job, &authority)?;
        let decision = persisted_decision(&job)?;
        let tool_result = job
            .tool_result_json
            .as_deref()
            .map(serde_json::from_str::<AgentJobToolResult>)
            .transpose()
            .map_err(|_| AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"))?;
        let receipt = self
            .decisions
            .commit_ai_decision(&job, &decision, tool_result.as_ref(), now_unix_ms)
            .await?;
        if receipt.event_sequences.is_empty()
            || receipt
                .event_sequences
                .windows(2)
                .any(|window| window[0] >= window[1])
        {
            return Err(AgentJobError::terminal("AGENT_CANONICAL_RECEIPT_INVALID"));
        }
        self.append_evidence(
            &job,
            "canonical_commit",
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            &sha256_label(job.decision_json.as_deref().unwrap_or_default().as_bytes()),
            EMPTY_SHA256,
            0,
            0,
            0,
            i32::from(tool_result.is_some()),
            &receipt.event_sequences,
            now_unix_ms,
        )
        .await?;
        self.append_evidence(
            &job,
            "completed",
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            0,
            0,
            0,
            0,
            &receipt.event_sequences,
            now_unix_ms,
        )
        .await?;
        job = self
            .repository
            .transition(&self.transition_draft(
                &job,
                WorkflowState::Committing,
                WorkflowState::Completed,
                "completed",
                None,
                None,
                Some(receipt.event_sequences.clone()),
                None,
                now_unix_ms,
            )?)
            .await?;
        Ok(AgentJobOutcome::Completed {
            job_id: job.job_id,
            event_sequences: receipt.event_sequences,
        })
    }

    async fn complete_human_approval(
        &self,
        mut job: DurableAgentJob,
        approval: DurableAgentApproval,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobOutcome> {
        let sequences = vec![approval.approval_event_sequence];
        self.append_evidence(
            &job,
            "completed",
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            EMPTY_SHA256,
            0,
            0,
            0,
            0,
            &sequences,
            now_unix_ms,
        )
        .await?;
        job = self
            .repository
            .transition(&self.transition_draft(
                &job,
                WorkflowState::Committing,
                WorkflowState::Completed,
                "human_approval_completed",
                None,
                None,
                Some(sequences.clone()),
                None,
                now_unix_ms,
            )?)
            .await?;
        Ok(AgentJobOutcome::Completed {
            job_id: job.job_id,
            event_sequences: sequences,
        })
    }

    async fn execute_provider_with_lease(
        &self,
        job: &DurableAgentJob,
        request: &ModelChatRequest,
        cancellation: &ProviderCancellation,
        started_unix_ms: i64,
    ) -> AgentJobResult<
        crate::model_provider::ProviderExecution<crate::model_provider::ModelChatResponse>,
    > {
        let deadline_delay_ms = job
            .deadline_unix_ms
            .checked_sub(started_unix_ms)
            .filter(|remaining| *remaining > 0)
            .and_then(|remaining| u64::try_from(remaining).ok())
            .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_DEADLINE_EXCEEDED"))?;
        let deadline = tokio::time::sleep(Duration::from_millis(deadline_delay_ms));
        tokio::pin!(deadline);
        let started = Instant::now();
        let mut capability_probe = Box::pin(self.provider.probe_capabilities(cancellation));
        let capabilities = loop {
            tokio::select! {
                biased;
                _ = &mut deadline => {
                    cancellation.cancel();
                    return Err(AgentJobError::terminal("AGENT_JOB_DEADLINE_EXCEEDED"));
                }
                result = &mut capability_probe => {
                    let capabilities = result.map_err(map_provider_error)?;
                    self.refresh_provider_lease(
                        job,
                        cancellation,
                        started_unix_ms,
                        started,
                    ).await?;
                    break capabilities;
                }
                _ = tokio::time::sleep(self.configuration.heartbeat_interval) => {
                    self.refresh_provider_lease(
                        job,
                        cancellation,
                        started_unix_ms,
                        started,
                    ).await?;
                }
            }
        };
        self.validate_executed_route(job, &capabilities.route, ModelOperation::CapabilityProbe)?;
        if !capabilities.output.chat
            || !capabilities.output.structured_output
            || !capabilities.output.tool_requests
        {
            return Err(AgentJobError::terminal(
                "MODEL_PROVIDER_CAPABILITY_REQUIRED",
            ));
        }

        let mut execution = Box::pin(self.provider.chat(request, cancellation));
        loop {
            tokio::select! {
                biased;
                _ = &mut deadline => {
                    cancellation.cancel();
                    return Err(AgentJobError::terminal("AGENT_JOB_DEADLINE_EXCEEDED"));
                }
                result = &mut execution => {
                    let result = result.map_err(map_provider_error)?;
                    self.refresh_provider_lease(
                        job,
                        cancellation,
                        started_unix_ms,
                        started,
                    ).await?;
                    self.validate_executed_route(job, &result.route, ModelOperation::Chat)?;
                    return Ok(result);
                }
                _ = tokio::time::sleep(self.configuration.heartbeat_interval) => {
                    self.refresh_provider_lease(
                        job,
                        cancellation,
                        started_unix_ms,
                        started,
                    ).await?;
                }
            }
        }
    }

}
