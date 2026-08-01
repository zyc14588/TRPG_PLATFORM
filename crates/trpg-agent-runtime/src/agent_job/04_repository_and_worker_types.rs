#[async_trait]
pub trait AgentJobRepository: Send + Sync {
    async fn load(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentJob>>;

    async fn claim_due(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<Option<DurableAgentJob>>;

    async fn transition(&self, draft: &AgentJobTransitionDraft) -> AgentJobResult<DurableAgentJob>;

    async fn heartbeat(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<bool>;

    async fn cancellation_requested(&self, job_id: &str) -> AgentJobResult<bool>;

    async fn load_authority(
        &self,
        campaign_id: &str,
    ) -> AgentJobResult<DurableAgentAuthoritySnapshot>;

    async fn load_context(&self, job_id: &str) -> AgentJobResult<DurableAgentContextSnapshot>;

    async fn load_approval(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentApproval>>;

    async fn append_evidence(&self, draft: &AgentJobEvidenceDraft) -> AgentJobResult<()>;
}

#[async_trait]
impl AgentJobRepository for DurableWorkflowStore {
    async fn load(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentJob>> {
        self.load_agent_job(job_id).await.map_err(map_store_error)
    }

    async fn claim_due(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<Option<DurableAgentJob>> {
        self.claim_due_agent_job(claim_owner, now_unix_ms, lease_duration_ms)
            .await
            .map_err(map_store_error)
    }

    async fn transition(&self, draft: &AgentJobTransitionDraft) -> AgentJobResult<DurableAgentJob> {
        self.transition_agent_job(draft)
            .await
            .map_err(map_store_error)
    }

    async fn heartbeat(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<bool> {
        self.heartbeat_agent_job(
            job_id,
            claim_owner,
            claim_token,
            now_unix_ms,
            lease_duration_ms,
        )
        .await
        .map_err(map_store_error)
    }

    async fn cancellation_requested(&self, job_id: &str) -> AgentJobResult<bool> {
        self.agent_job_cancellation_requested(job_id)
            .await
            .map_err(map_store_error)
    }

    async fn load_authority(
        &self,
        campaign_id: &str,
    ) -> AgentJobResult<DurableAgentAuthoritySnapshot> {
        self.load_agent_authority_snapshot(campaign_id)
            .await
            .map_err(map_store_error)
    }

    async fn load_context(&self, job_id: &str) -> AgentJobResult<DurableAgentContextSnapshot> {
        self.load_agent_job_context(job_id)
            .await
            .map_err(map_store_error)
    }

    async fn load_approval(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentApproval>> {
        self.load_agent_job_approval(job_id)
            .await
            .map_err(map_store_error)
    }

    async fn append_evidence(&self, draft: &AgentJobEvidenceDraft) -> AgentJobResult<()> {
        self.append_agent_job_evidence(draft)
            .await
            .map_err(map_store_error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentJobOutcome {
    Idle,
    Completed {
        job_id: String,
        event_sequences: Vec<i64>,
    },
    AwaitingHumanApproval {
        job_id: String,
    },
    RetryScheduled {
        job_id: String,
        error_code: &'static str,
    },
    TerminalFailure {
        job_id: String,
        error_code: &'static str,
    },
}

pub struct AgentJobWorker {
    repository: Arc<dyn AgentJobRepository>,
    provider: Arc<dyn ExecutableModelProvider>,
    tools: Arc<dyn AgentJobToolPort>,
    decisions: Arc<dyn AgentJobDecisionPort>,
    local_certification: Option<CertifiedLocalModel>,
    configuration: AgentJobExecutionConfig,
}

impl fmt::Debug for AgentJobWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgentJobWorker")
            .field("repository", &"[AGENT JOB REPOSITORY]")
            .field("provider", &self.provider.startup_route_snapshot())
            .field("tools", &"[GOVERNED TOOL PORT]")
            .field("decisions", &"[CANONICAL DECISION PORT]")
            .field("local_certification", &self.local_certification)
            .field("configuration", &self.configuration)
            .finish()
    }
}
