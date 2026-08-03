#[async_trait]
impl AgentJobDecisionPort for GovernedAgentDecisionPort {
    async fn authorize_execution(
        &self,
        job: &DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<()> {
        let identity = Arc::clone(&self.identity);
        let workload_id = self.workload_id.clone();
        let internal_credential_ttl_ms = self.internal_credential_ttl_ms;
        let job = job.clone();
        tokio::task::spawn_blocking(move || {
            let now_unix_ms = u64::try_from(now_unix_ms)
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_TIME_INVALID"))?;
            let expires_at_unix_ms = now_unix_ms
                .checked_add(internal_credential_ttl_ms)
                .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_TIME_INVALID"))?;
            let campaign_id = EntityId::new(&job.campaign_id)
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_CAMPAIGN_INVALID"))?;
            let agent_class = match (job.authority_mode.as_str(), job.agent_kind.as_str()) {
                ("AI_KP", "ai_keeper_orchestrator") => IdentityAgentClass::AiKeeperOrchestrator,
                ("HUMAN_KP", "keeper_copilot") => IdentityAgentClass::KeeperCopilot,
                _ => {
                    return Err(AgentJobError::terminal(
                        "AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH",
                    ));
                }
            };
            let mut identity = identity
                .lock()
                .map_err(|_| AgentJobError::retryable("AGENT_IDENTITY_LOCK_UNAVAILABLE"))?;
            let contract = identity
                .authority_contract(&campaign_id)
                .map_err(|_| AgentJobError::retryable("AGENT_IDENTITY_UNAVAILABLE"))?
                .ok_or_else(|| AgentJobError::terminal("AGENT_AUTHORITY_CONTRACT_REQUIRED"))?;
            let expected_mode = if job.authority_mode == "AI_KP" {
                trpg_shared_kernel::AuthorityMode::AiKp
            } else {
                trpg_shared_kernel::AuthorityMode::HumanKp
            };
            if contract.contract_id().as_str() != job.authority_contract_id
                || contract.version()
                    != u64::try_from(job.authority_contract_version).map_err(|_| {
                        AgentJobError::terminal("AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH")
                    })?
                || contract.mode() != &expected_mode
                || (expected_mode == trpg_shared_kernel::AuthorityMode::AiKp
                    && contract.authority_owner().as_str() != job.actor_id)
            {
                return Err(AgentJobError::terminal(
                    "AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH",
                ));
            }
            let workload_credential = identity
                .issue_workload_credential(
                    &workload_id,
                    IdentityWorkloadRole::WorkflowEngine,
                    now_unix_ms,
                    expires_at_unix_ms,
                )
                .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
            let workflow_authentication = identity
                .authenticate_workload(&workload_credential, now_unix_ms)
                .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
            identity
                .command_actor(&workflow_authentication, &campaign_id, now_unix_ms)
                .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
            let agent_credential = identity
                .issue_agent_run_credential(
                    &format!("run_{}", job.job_id),
                    &job.actor_id,
                    &job.campaign_id,
                    agent_class,
                    now_unix_ms,
                    expires_at_unix_ms,
                )
                .map_err(|_| AgentJobError::terminal("AGENT_RUN_IDENTITY_INVALID"))?;
            let agent_authentication = identity
                .authenticate_agent_run(&agent_credential, now_unix_ms)
                .map_err(|_| AgentJobError::terminal("AGENT_RUN_IDENTITY_INVALID"))?;
            agent_authentication
                .require_campaign(&campaign_id)
                .map_err(|_| AgentJobError::terminal("AGENT_RUN_IDENTITY_INVALID"))?;
            Ok(())
        })
        .await
        .map_err(|_| AgentJobError::retryable("AGENT_DECISION_TASK_UNAVAILABLE"))?
    }

    async fn commit_ai_decision(
        &self,
        job: &DurableAgentJob,
        decision: &AgentStructuredDecision,
        tool_result: Option<&AgentJobToolResult>,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobCommitReceipt> {
        let identity = Arc::clone(&self.identity);
        let committer = self.committer.clone();
        let prepared_tools = Arc::clone(&self.prepared_tools);
        let events = Arc::clone(&self.events);
        let workload_id = self.workload_id.clone();
        let internal_credential_ttl_ms = self.internal_credential_ttl_ms;
        let job = job.clone();
        let decision = decision.clone();
        let tool_result = tool_result.cloned();
        tokio::task::spawn_blocking(move || {
            let tool_result = tool_result.as_ref();
            if job.authority_mode != "AI_KP" || job.agent_kind != "ai_keeper_orchestrator" {
                return Err(AgentJobError::terminal(
                    "AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH",
                ));
            }
            let tool = match (decision.tool.as_ref(), tool_result) {
                (None, None) => AgentTool::NarrationOnly,
                (Some(call), Some(result)) => {
                    validate_tool_result(result)?;
                    parse_agent_tool(&call.name)?
                }
                _ => return Err(AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID")),
            };
            let now_unix_ms = u64::try_from(now_unix_ms)
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_TIME_INVALID"))?;
            let expires_at_unix_ms = now_unix_ms
                .checked_add(internal_credential_ttl_ms)
                .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_TIME_INVALID"))?;
            let campaign_id = EntityId::new(&job.campaign_id)
                .map_err(|_| AgentJobError::terminal("AGENT_JOB_CAMPAIGN_INVALID"))?;
            let mut identity = identity
                .lock()
                .map_err(|_| AgentJobError::retryable("AGENT_IDENTITY_LOCK_UNAVAILABLE"))?;
            let contract = identity
                .authority_contract(&campaign_id)
                .map_err(|_| AgentJobError::retryable("AGENT_IDENTITY_UNAVAILABLE"))?
                .ok_or_else(|| AgentJobError::terminal("AGENT_AUTHORITY_CONTRACT_REQUIRED"))?;
            if contract.contract_id().as_str() != job.authority_contract_id
                || contract.version()
                    != u64::try_from(job.authority_contract_version).map_err(|_| {
                        AgentJobError::terminal("AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH")
                    })?
                || contract.authority_owner().as_str() != job.actor_id
                || contract.mode() != &trpg_shared_kernel::AuthorityMode::AiKp
            {
                return Err(AgentJobError::terminal(
                    "AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH",
                ));
            }
            let workload_credential = identity
                .issue_workload_credential(
                    &workload_id,
                    IdentityWorkloadRole::WorkflowEngine,
                    now_unix_ms,
                    expires_at_unix_ms,
                )
                .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
            let workflow_authentication = identity
                .authenticate_workload(&workload_credential, now_unix_ms)
                .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
            let agent_credential = identity
                .issue_agent_run_credential(
                    &format!("run_{}", job.job_id),
                    &job.actor_id,
                    &job.campaign_id,
                    IdentityAgentClass::AiKeeperOrchestrator,
                    now_unix_ms,
                    expires_at_unix_ms,
                )
                .map_err(|_| AgentJobError::terminal("AGENT_RUN_IDENTITY_INVALID"))?;
            let agent_authentication = identity
                .authenticate_agent_run(&agent_credential, now_unix_ms)
                .map_err(|_| AgentJobError::terminal("AGENT_RUN_IDENTITY_INVALID"))?;
            let workflow_actor = identity
                .command_actor(&workflow_authentication, &campaign_id, now_unix_ms)
                .map_err(|_| AgentJobError::terminal("AGENT_WORKLOAD_IDENTITY_INVALID"))?;
            drop(identity);

            let scope: VisibilityScope = serde_json::from_str(&job.visibility_scope_json)
                .map_err(|_| AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"))?;
            let visibility =
                Visibility::try_from_parts(&scope.output_label, scope.subject_id.as_deref())
                    .map_err(|_| AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"))?;
            let context = AuthenticatedCommandContext::new(
                workflow_actor,
                ResourceRef::new(&job.campaign_id, "agent_turn", &job.input_stream_id)
                    .map_err(|_| AgentJobError::terminal("AGENT_JOB_RESOURCE_INVALID"))?,
                contract.binding().map_err(|_| {
                    AgentJobError::terminal("AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH")
                })?,
                format!("trace_{}", job.job_id),
                now_unix_ms,
                expires_at_unix_ms,
            )
            .map_err(|_| AgentJobError::terminal("AGENT_COMMAND_CONTEXT_INVALID"))?;
            let tool_request = ToolRequest::formal(AgentKind::AiKeeperOrchestrator, tool);
            let agent_decision = AgentDecision::new(
                format!("decision_{}", job.job_id),
                tool_request,
                &decision.player_visible_text,
                &agent_authentication,
            )
            .map_err(|error| AgentJobError::terminal(error.code()))?;
            let command = CommandEnvelope::new(
                agent_decision.clone(),
                CommandMetadata {
                    command_id: EntityId::new(format!("command_agent_decision_{}", job.job_id))
                        .map_err(|_| AgentJobError::terminal("AGENT_COMMAND_ID_INVALID"))?,
                    idempotency_key: format!("{}:canonical", job.idempotency_key),
                    expected_version: u64::try_from(job.input_stream_version)
                        .map_err(|_| AgentJobError::terminal("AGENT_INPUT_VERSION_INVALID"))?,
                    authority_mode: trpg_shared_kernel::AuthorityMode::AiKp,
                    visibility,
                    fact_provenance: FactProvenance {
                        kind: ProvenanceKind::AgentProposal,
                        reference: EntityId::new(format!(
                            "event_sequence_{}",
                            job.input_event_sequence
                        ))
                        .map_err(|_| AgentJobError::terminal("AGENT_PROVENANCE_INVALID"))?,
                        recorded_by: EntityId::new(&job.actor_id)
                            .map_err(|_| AgentJobError::terminal("AGENT_PROVENANCE_INVALID"))?,
                    },
                    correlation_id: EntityId::new(&job.job_id)
                        .map_err(|_| AgentJobError::terminal("AGENT_CORRELATION_INVALID"))?,
                    causation_id: EntityId::new(format!(
                        "event_sequence_{}",
                        job.input_event_sequence
                    ))
                    .map_err(|_| AgentJobError::terminal("AGENT_CAUSATION_INVALID"))?,
                    write_path: FormalWritePath::WorkflowDecision,
                    authenticated_context: context,
                },
            );
            let mut events = events
                .lock()
                .map_err(|_| AgentJobError::retryable("AGENT_EVENT_CUSTODY_LOCK_UNAVAILABLE"))?;
            if let Some(result) = tool_result {
                prepared_tools.prepare(
                    agent_decision.decision_id.as_str(),
                    AgentToolExecutionOutput {
                        execution_id: result.execution_id.clone(),
                        result: result.result.clone(),
                        result_hash: result.result_hash.clone(),
                    },
                )?;
            }
            let commit_result = committer.commit(
                &mut events,
                &command,
                &workflow_authentication,
                agent_decision.clone(),
                now_unix_ms,
            );
            prepared_tools.clear(agent_decision.decision_id.as_str());
            let committed = commit_result.map_err(|error| AgentJobError::terminal(error.code()))?;
            verify_committed_tool_result(&committed, tool_result)?;
            let event_sequences = committed
                .into_iter()
                .map(|event| {
                    i64::try_from(event.sequence)
                        .map_err(|_| AgentJobError::terminal("AGENT_CANONICAL_RECEIPT_INVALID"))
                })
                .collect::<AgentJobResult<Vec<_>>>()?;
            Ok(AgentJobCommitReceipt { event_sequences })
        })
        .await
        .map_err(|_| AgentJobError::retryable("AGENT_DECISION_TASK_UNAVAILABLE"))?
    }
}
