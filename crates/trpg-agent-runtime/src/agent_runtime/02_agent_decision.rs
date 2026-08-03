
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentDecision {
    pub decision_id: EntityId,
    pub tool_request: ToolRequest,
    pub player_visible_text: String,
    pub keeper_notes: Vec<String>,
    pub linked_records: Vec<&'static str>,
    pub audit_fields: Vec<&'static str>,
    authentication: AuthenticationContext,
}

impl AgentDecision {
    pub fn new(
        decision_id: impl Into<String>,
        tool_request: ToolRequest,
        player_visible_text: impl Into<String>,
        authentication: &AuthenticationContext,
    ) -> AgentResult<Self> {
        validate_requester_identity(&tool_request, authentication)?;
        let player_visible_text = player_visible_text.into();
        Ok(Self {
            decision_id: EntityId::new(decision_id).map_err(AgentError::from)?,
            tool_request,
            player_visible_text: redact_player_visible_text(&player_visible_text),
            keeper_notes: Vec::new(),
            linked_records: vec!["DecisionRecord", "GameEvent", "ToolResult"],
            audit_fields: vec![
                "agent_pack_version",
                "prompt_version",
                "model_provider",
                "context_hash",
                "tool_calls",
                "visibility_labels",
            ],
            authentication: authentication.clone(),
        })
    }
}

fn validate_requester_identity(
    request: &ToolRequest,
    authentication: &AuthenticationContext,
) -> AgentResult<()> {
    let PrincipalKind::AgentRun { class, .. } = authentication.kind() else {
        return Err(AgentError::Core(TrpgError::InternalIdentityInvalid));
    };
    let expected = match class {
        IdentityAgentClass::AiKeeperOrchestrator => AgentKind::AiKeeperOrchestrator,
        IdentityAgentClass::KeeperCopilot => AgentKind::KeeperCopilot,
        IdentityAgentClass::AtmosphereWriter => AgentKind::AtmosphereWriter,
        IdentityAgentClass::MemoryCurator => AgentKind::MemoryCurator,
    };
    if request.requested_by() != expected {
        return Err(AgentError::Core(TrpgError::InternalIdentityInvalid));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum AgentEventPayload {
    ToolRequestApproved {
        tool: &'static str,
        decision: ToolDecision,
        seal: AgentFormalEventSeal,
    },
    ToolExecutionSucceeded {
        tool: &'static str,
        execution_id: EntityId,
        result: serde_json::Value,
        result_hash: String,
        seal: AgentFormalEventSeal,
    },
    DecisionCommitted {
        decision_id: EntityId,
        player_visible_text: String,
        linked_records: Vec<&'static str>,
        audit_fields: Vec<&'static str>,
        seal: AgentFormalEventSeal,
    },
    DraftDecisionCreated {
        downgraded_to: &'static str,
    },
    AgentContextAssembled {
        visible_fact_count: usize,
    },
}

/// Opaque constructor token carried by formal agent events.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct AgentFormalEventSeal {
    _private: (),
}

impl AgentFormalEventSeal {
    fn new() -> Self {
        Self { _private: () }
    }
}

pub fn validate_agent_command<T>(
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
) -> AgentResult<()> {
    if command.write_path == FormalWritePath::DirectAgent {
        return Err(AgentError::AgentDirectStateWriteForbidden);
    }
    contract.validate_command(command).map_err(AgentError::from)
}

fn derived_command<T: Clone>(
    command: &CommandEnvelope<T>,
    suffix: &str,
    expected_version: u64,
) -> AgentResult<CommandEnvelope<T>> {
    let mut derived = command.clone();
    derived.command_id = EntityId::new(format!("{}_{}", command.command_id.as_str(), suffix))?;
    derived.idempotency_key = format!("{}:{}", command.idempotency_key, suffix);
    derived.expected_version = expected_version;
    Ok(derived)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentToolExecutionOutput {
    pub execution_id: String,
    pub result: serde_json::Value,
    pub result_hash: String,
}

pub trait AgentToolExecutor: Send + Sync {
    fn execute(&self, decision: &AgentDecision) -> AgentResult<AgentToolExecutionOutput>;
}

#[derive(Debug)]
struct RejectingAgentToolExecutor;

impl AgentToolExecutor for RejectingAgentToolExecutor {
    fn execute(&self, _decision: &AgentDecision) -> AgentResult<AgentToolExecutionOutput> {
        Err(AgentError::ToolPermissionDenied)
    }
}

#[derive(Clone)]
pub struct AgentDecisionCommitter {
    identity_verifier: IdentityVerifier,
    tool_executor: Arc<dyn AgentToolExecutor>,
}

impl std::fmt::Debug for AgentDecisionCommitter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentDecisionCommitter")
            .field("identity_verifier", &self.identity_verifier)
            .field("tool_executor", &"[TRUSTED TOOL EXECUTOR]")
            .finish()
    }
}
