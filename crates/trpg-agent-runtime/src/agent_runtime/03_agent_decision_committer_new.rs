
impl AgentDecisionCommitter {
    pub fn new(identity_verifier: IdentityVerifier) -> AgentResult<Self> {
        Ok(Self {
            identity_verifier,
            tool_executor: Arc::new(RejectingAgentToolExecutor),
        })
    }

    pub fn with_tool_executor(
        identity_verifier: IdentityVerifier,
        tool_executor: Arc<dyn AgentToolExecutor>,
    ) -> AgentResult<Self> {
        Ok(Self {
            identity_verifier,
            tool_executor,
        })
    }

    pub fn commit(
        &self,
        store: &mut EventStore<AgentEventPayload>,
        command: &CommandEnvelope<AgentDecision>,
        workflow_authentication: &AuthenticationContext,
        decision: AgentDecision,
        now_unix_ms: u64,
    ) -> AgentResult<Vec<EventEnvelope<AgentEventPayload>>> {
        let contract = self
            .identity_verifier
            .authority_contract(command.authenticated_context().resource().campaign_id())
            .map_err(|_| AgentError::Core(TrpgError::AuthorityViolation))?;
        contract
            .validate_command(command)
            .map_err(AgentError::from)?;
        self.identity_verifier
            .verify_actor(
                workflow_authentication,
                &command.actor,
                command.authenticated_context().resource().campaign_id(),
                now_unix_ms,
            )
            .map_err(|_| AgentError::Core(TrpgError::InternalIdentityInvalid))?;
        if command.write_path == FormalWritePath::DirectAgent {
            return Err(AgentError::AgentDirectStateWriteForbidden);
        }
        self.identity_verifier
            .verify(&decision.authentication, now_unix_ms)
            .map_err(|_| AgentError::Core(TrpgError::InternalIdentityInvalid))?;
        if command.payload != decision {
            return Err(AgentError::Core(TrpgError::DecisionDraftChanged));
        }
        decision
            .authentication
            .require_campaign(contract.campaign_id())
            .map_err(|error| match error {
                trpg_identity::IdentityError::CampaignScopeMismatch => {
                    AgentError::Core(TrpgError::CampaignScopeMismatch)
                }
                _ => AgentError::Core(TrpgError::InternalIdentityInvalid),
            })?;
        validate_requester_identity(&decision.tool_request, &decision.authentication)?;

        if contract.mode() == &AuthorityMode::AiKp
            && decision.authentication.subject_id() != contract.authority_owner()
        {
            return Err(AgentError::Core(TrpgError::AuthorityOwnerMismatch));
        }

        if !decision.tool_request.is_formal_state_change() {
            if contract.mode() == &AuthorityMode::HumanKp
                || decision.tool_request.tool() != AgentTool::NarrationOnly
            {
                let resource = command.authenticated_context().resource();
                let draft_version = store
                    .inner
                    .current_stream_version(resource.campaign_id(), resource.resource_id());
                let draft_command = derived_command(command, "draft", draft_version)?;
                return Ok(vec![store.append(
                    &draft_command,
                    "DraftDecisionCreated",
                    AgentEventPayload::DraftDecisionCreated {
                        downgraded_to: decision.tool_request.tool().as_str(),
                    },
                )?]);
            }

            // AI_KP narration is an official, non-rules-mutating turn event.
            // It still crosses the same policy/audit/canonical custody path;
            // only the tool side-effect stage is intentionally absent.
            let (authorization, canonical) = {
                let custody = store.formal_custody()?;
                (
                    custody.authorizer.authorize(
                        workflow_authentication,
                        Some(&decision.authentication),
                        command,
                        "ai_keeper_orchestrator",
                        now_unix_ms,
                    )?,
                    Arc::clone(&custody.canonical),
                )
            };
            let decision_command =
                derived_command(command, "decision", command.expected_version)?;
            return persist_agent_formal_batch(
                store,
                command,
                &authorization,
                &canonical,
                vec![(
                    decision_command,
                    "DecisionCommitted",
                    AgentEventPayload::DecisionCommitted {
                        decision_id: decision.decision_id,
                        player_visible_text: redact_player_visible_text(
                            &decision.player_visible_text,
                        ),
                        linked_records: decision.linked_records,
                        audit_fields: decision.audit_fields,
                        seal: AgentFormalEventSeal::new(),
                    },
                )],
            );
        }

        let tool_decision =
            evaluate_agent_tool_request(&command.authority_mode, &decision.tool_request);
        if tool_decision.draft_only {
            let resource = command.authenticated_context().resource();
            let draft_version = store
                .inner
                .current_stream_version(resource.campaign_id(), resource.resource_id());
            let draft_command = derived_command(command, "draft", draft_version)?;
            return Ok(vec![store.append(
                &draft_command,
                "DraftDecisionCreated",
                AgentEventPayload::DraftDecisionCreated {
                    downgraded_to: tool_decision
                        .downgraded_to
                        .unwrap_or(AgentTool::NarrationOnly)
                        .as_str(),
                },
            )?]);
        }
        if let Some(error) = tool_decision.error {
            return Err(if error == AgentError::ToolPermissionDenied.code() {
                AgentError::ToolPermissionDenied
            } else {
                AgentError::HumanKpDraftOnly
            });
        }

        // Preserve the original derived request hashes so an exact network
        // retry resolves through EventStore's scoped idempotency index before
        // optimistic concurrency is evaluated.
        let next_version = command.expected_version;
        let tool_command = derived_command(command, "tool", next_version)?;
        let execution_command = derived_command(command, "execution", next_version + 1)?;
        let decision_command = derived_command(command, "decision", next_version + 2)?;
        let requested_role = match decision.authentication.kind() {
            PrincipalKind::AgentRun { class, .. } => match class {
                IdentityAgentClass::AiKeeperOrchestrator => "ai_keeper_orchestrator",
                IdentityAgentClass::KeeperCopilot => "keeper_copilot",
                IdentityAgentClass::AtmosphereWriter => "atmosphere_writer",
                IdentityAgentClass::MemoryCurator => "memory_curator",
            },
            _ => return Err(AgentError::Core(TrpgError::InternalIdentityInvalid)),
        };
        let (authorization, canonical) = {
            let custody = store.formal_custody()?;
            (
                custody.authorizer.authorize(
                    workflow_authentication,
                    Some(&decision.authentication),
                    command,
                    requested_role,
                    now_unix_ms,
                )?,
                Arc::clone(&custody.canonical),
            )
        };
        // Formal OpenFGA/OPA authorization is deliberately complete before
        // any tool side effect. A durable receipt lookup also precedes tool
        // execution so an exact or cold retry reuses the canonical result.
        let commit_key = CanonicalCommitKey {
            commit_id: format!(
                "{}_{}",
                contract.campaign_id().as_str(),
                command.command_id.as_str()
            ),
            campaign_id: contract.campaign_id().to_string(),
            stream_id: command
                .authenticated_context()
                .resource()
                .resource_id()
                .to_string(),
            idempotency_key: command.idempotency_key.clone(),
            expected_version: command.expected_version,
        };
        let execution = match canonical.load_receipt(&commit_key)? {
            Some(receipt) => {
                agent_execution_from_receipt(&receipt, decision.tool_request.tool().as_str())?
            }
            None => self.tool_executor.execute(&decision)?,
        };
        let execution_id = EntityId::new(&execution.execution_id)?;
        if !execution.result_hash.starts_with("sha256:")
            || execution.result_hash.len() != 71
            || !execution
                .result_hash
                .bytes()
                .skip(7)
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(AgentError::Core(TrpgError::InvalidConfiguration(
                "agent_tool_execution_result",
            )));
        }
        persist_agent_formal_batch(
            store,
            command,
            &authorization,
            &canonical,
            vec![
                (
                    tool_command,
                    "ToolRequestApproved",
                    AgentEventPayload::ToolRequestApproved {
                        tool: decision.tool_request.tool().as_str(),
                        decision: tool_decision,
                        seal: AgentFormalEventSeal::new(),
                    },
                ),
                (
                    execution_command,
                    "ToolExecutionSucceeded",
                    AgentEventPayload::ToolExecutionSucceeded {
                        tool: decision.tool_request.tool().as_str(),
                        execution_id,
                        result_hash: execution.result_hash,
                        seal: AgentFormalEventSeal::new(),
                    },
                ),
                (
                    decision_command,
                    "DecisionCommitted",
                    AgentEventPayload::DecisionCommitted {
                        decision_id: decision.decision_id,
                        player_visible_text: redact_player_visible_text(
                            &decision.player_visible_text,
                        ),
                        linked_records: decision.linked_records,
                        audit_fields: decision.audit_fields,
                        seal: AgentFormalEventSeal::new(),
                    },
                ),
            ],
        )
    }
}

fn agent_execution_from_receipt(
    receipt: &CanonicalCommitReceipt,
    expected_tool: &str,
) -> AgentResult<AgentToolExecutionOutput> {
    if receipt.events.len() != 3 {
        return Err(AgentError::Core(TrpgError::AuditIntegrityViolation));
    }
    let event = receipt
        .events
        .get(1)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    if event.event_type != "ToolExecutionSucceeded" {
        return Err(AgentError::Core(TrpgError::AuditIntegrityViolation));
    }
    let payload: serde_json::Value = serde_json::from_str(&event.payload_json)
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let execution = payload
        .get("ToolExecutionSucceeded")
        .and_then(serde_json::Value::as_object)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    let tool = execution
        .get("tool")
        .and_then(serde_json::Value::as_str)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    let execution_id = execution
        .get("execution_id")
        .and_then(serde_json::Value::as_str)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    let result_hash = execution
        .get("result_hash")
        .and_then(serde_json::Value::as_str)
        .ok_or(TrpgError::AuditIntegrityViolation)?;
    if tool != expected_tool {
        return Err(AgentError::Core(TrpgError::AuditIntegrityViolation));
    }
    Ok(AgentToolExecutionOutput {
        execution_id: execution_id.to_owned(),
        result_hash: result_hash.to_owned(),
    })
}
