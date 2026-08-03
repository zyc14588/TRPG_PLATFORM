#[async_trait]
pub trait AgentJobDecisionPort: Send + Sync {
    async fn authorize_execution(
        &self,
        job: &DurableAgentJob,
        now_unix_ms: i64,
    ) -> AgentJobResult<()>;

    async fn commit_ai_decision(
        &self,
        job: &DurableAgentJob,
        decision: &AgentStructuredDecision,
        tool_result: Option<&AgentJobToolResult>,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobCommitReceipt>;
}

pub struct ProductionAgentIdentityConfiguration<'a> {
    pub database_url: &'a str,
    pub postgres_ca_certificate_pem: Option<&'a [u8]>,
    pub redis_url: &'a str,
    pub redis_namespace: &'a str,
    pub signing_key: &'a [u8; 32],
    pub session_ttl_ms: u64,
    pub argon2_concurrency: usize,
    pub redis_root_certificate: Option<&'a [u8]>,
    pub redis_client_certificate: Option<&'a [u8]>,
    pub redis_client_private_key: Option<&'a [u8]>,
    pub workload_id: &'a str,
    pub internal_credential_ttl_ms: u64,
}

pub struct GovernedAgentDecisionPort {
    identity: Arc<Mutex<IdentityService>>,
    committer: AgentDecisionCommitter,
    prepared_tools: Arc<PreparedAgentToolExecutor>,
    events: Arc<Mutex<AgentEventStore<AgentEventPayload>>>,
    workload_id: String,
    internal_credential_ttl_ms: u64,
}

#[derive(Debug, Default)]
struct PreparedAgentToolExecutor {
    results: Mutex<HashMap<String, AgentToolExecutionOutput>>,
}

impl PreparedAgentToolExecutor {
    fn prepare(&self, decision_id: &str, result: AgentToolExecutionOutput) -> AgentJobResult<()> {
        let mut results = self
            .results
            .lock()
            .map_err(|_| AgentJobError::retryable("AGENT_TOOL_RESULT_LOCK_UNAVAILABLE"))?;
        if results
            .get(decision_id)
            .is_some_and(|existing| existing != &result)
        {
            return Err(AgentJobError::terminal("AGENT_TOOL_RESULT_CONFLICT"));
        }
        results.insert(decision_id.to_owned(), result);
        Ok(())
    }

    fn clear(&self, decision_id: &str) {
        if let Ok(mut results) = self.results.lock() {
            results.remove(decision_id);
        }
    }
}

impl AgentToolExecutor for PreparedAgentToolExecutor {
    fn execute(
        &self,
        decision: &AgentDecision,
    ) -> crate::agent_runtime::AgentResult<AgentToolExecutionOutput> {
        self.results
            .lock()
            .map_err(|_| AgentError::ToolPermissionDenied)?
            .get(decision.decision_id.as_str())
            .cloned()
            .ok_or(AgentError::ToolPermissionDenied)
    }
}

impl fmt::Debug for GovernedAgentDecisionPort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernedAgentDecisionPort")
            .field("identity", &"[PERSISTENT IDENTITY SERVICE]")
            .field("committer", &self.committer)
            .field("events", &"[CANONICAL AGENT EVENT CUSTODY]")
            .field("workload_id", &self.workload_id)
            .field(
                "internal_credential_ttl_ms",
                &self.internal_credential_ttl_ms,
            )
            .finish()
    }
}

impl GovernedAgentDecisionPort {
    pub fn from_prepared_postgres(
        configuration: ProductionAgentIdentityConfiguration<'_>,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical: Arc<dyn CanonicalCommitPort>,
    ) -> AgentJobResult<Self> {
        if configuration.workload_id.trim().is_empty()
            || configuration.internal_credential_ttl_ms == 0
        {
            return Err(AgentJobError::terminal(
                "AGENT_IDENTITY_CONFIGURATION_INVALID",
            ));
        }
        let identity = IdentityService::from_prepared_postgres_with_security_and_redis_tls(
            configuration.database_url,
            configuration.postgres_ca_certificate_pem,
            configuration.redis_url,
            configuration.redis_namespace,
            configuration.signing_key,
            configuration.session_ttl_ms,
            configuration.argon2_concurrency,
            configuration.redis_root_certificate,
            configuration.redis_client_certificate,
            configuration.redis_client_private_key,
        )
        .map_err(|_| AgentJobError::terminal("AGENT_IDENTITY_UNAVAILABLE"))?;
        let verifier = identity.verifier();
        let authorizer = FormalCommitAuthorizer::new(
            verifier.clone(),
            policy,
            FormalCommitAudit::from_file_log(audit),
        );
        let prepared_tools = Arc::new(PreparedAgentToolExecutor::default());
        let committer = AgentDecisionCommitter::with_tool_executor(
            verifier,
            Arc::clone(&prepared_tools) as Arc<dyn AgentToolExecutor>,
        )
        .map_err(|_| AgentJobError::terminal("AGENT_COMMITTER_INVALID"))?;
        let events = AgentEventStore::with_formal_custody(authorizer, canonical);
        Ok(Self {
            identity: Arc::new(Mutex::new(identity)),
            committer,
            prepared_tools,
            events: Arc::new(Mutex::new(events)),
            workload_id: configuration.workload_id.to_owned(),
            internal_credential_ttl_ms: configuration.internal_credential_ttl_ms,
        })
    }
}
