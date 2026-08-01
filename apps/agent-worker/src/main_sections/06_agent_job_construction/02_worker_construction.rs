fn map_agent_workflow_error(error: trpg_runtime::durable_workflow::WorkflowStoreError) -> AgentJobError {
    match error {
        trpg_runtime::durable_workflow::WorkflowStoreError::Connection
        | trpg_runtime::durable_workflow::WorkflowStoreError::Database(_) => {
            AgentJobError::retryable("AGENT_JOB_DATABASE_UNAVAILABLE")
        }
        trpg_runtime::durable_workflow::WorkflowStoreError::NotFound => {
            AgentJobError::terminal("AGENT_JOB_NOT_FOUND")
        }
        trpg_runtime::durable_workflow::WorkflowStoreError::VersionConflict { .. }
        | trpg_runtime::durable_workflow::WorkflowStoreError::StateConflict => {
            AgentJobError::retryable("AGENT_JOB_CONCURRENT_UPDATE")
        }
        trpg_runtime::durable_workflow::WorkflowStoreError::IdempotencyConflict => {
            AgentJobError::terminal("AGENT_JOB_IDEMPOTENCY_CONFLICT")
        }
        trpg_runtime::durable_workflow::WorkflowStoreError::Configuration(_)
        | trpg_runtime::durable_workflow::WorkflowStoreError::Migration
        | trpg_runtime::durable_workflow::WorkflowStoreError::Validation(_)
        | trpg_runtime::durable_workflow::WorkflowStoreError::IntegrityViolation(_) => {
            AgentJobError::terminal("AGENT_JOB_STORAGE_INTEGRITY_ERROR")
        }
    }
}

fn map_agent_canonical_context_error(error: CanonicalStoreError) -> AgentJobError {
    match error {
        CanonicalStoreError::Connection { .. }
        | CanonicalStoreError::WitnessWrite { .. }
        | CanonicalStoreError::WitnessFinalizationPending { .. } => {
            AgentJobError::retryable("AGENT_CANONICAL_INPUT_UNAVAILABLE")
        }
        CanonicalStoreError::Configuration(_)
        | CanonicalStoreError::Validation(_)
        | CanonicalStoreError::Migration { .. }
        | CanonicalStoreError::MigrationChecksumMismatch { .. }
        | CanonicalStoreError::PrimaryWrite { .. }
        | CanonicalStoreError::VersionConflict { .. }
        | CanonicalStoreError::IdempotencyConflict
        | CanonicalStoreError::IntegrityViolation(_) => {
            AgentJobError::terminal("AGENT_CANONICAL_INPUT_INVALID")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn optional_agent_job_worker_from_environment(
    workflow: DurableWorkflowStore,
    provider: Option<HttpModelProvider<MountedFileSecretResolver>>,
    read_canonical: PostgresCanonicalStore,
    commit_canonical: Option<PostgresCanonicalStore>,
    secret_manager: &Arc<SecretManager<MountedFileSecretResolver>>,
    database_url: &SecretValue,
    redis_url: &SecretValue,
    witness_url: &SecretValue,
    worker_id: &str,
    redis_ca: Option<&[u8]>,
    redis_client_certificate: Option<&[u8]>,
    redis_client_private_key: Option<&[u8]>,
) -> Result<Option<AgentJobWorker>, String> {
    let Some(provider) = provider else {
        return Ok(None);
    };
    let commit_canonical = commit_canonical
        .ok_or_else(|| "AGENT_CANONICAL_STORE_REQUIRED".to_owned())?;
    let canonical_repository = read_canonical;
    let canonical_runtime = Arc::new(Mutex::new(
        tokio::runtime::Runtime::new()
            .map_err(|_| "AGENT_CANONICAL_RUNTIME_INITIALIZATION_FAILED".to_owned())?,
    ));
    let canonical = Arc::new(PostgresCanonicalCommitPort::new(
        canonical_runtime,
        commit_canonical,
    ));
    let (policy, audit) = agent_policy_and_audit_from_environment(secret_manager)?;
    let identity_signing_key =
        resolve_mounted_secret(secret_manager, "TRPG_IDENTITY_SIGNING_KEY")?
            .to_key32()
            .map_err(|_| "IDENTITY_SIGNING_KEY_INVALID".to_owned())?;
    let postgres_ca = optional_file_bytes("TRPG_POSTGRES_CA_CERT_PATH")?;
    let redis_namespace = std::env::var("TRPG_IDENTITY_REDIS_NAMESPACE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "trpg:identity".to_owned());
    let session_ttl_ms = bounded_environment_u64(
        "TRPG_IDENTITY_SESSION_TTL_MS",
        86_400_000,
        1_000,
        604_800_000,
    )?;
    let internal_credential_ttl_ms = bounded_environment_u64(
        "TRPG_AGENT_INTERNAL_CREDENTIAL_TTL_MS",
        60_000,
        1_000,
        300_000,
    )?;
    let argon2_concurrency = usize::try_from(bounded_environment_u64(
        "TRPG_IDENTITY_ARGON2_CONCURRENCY",
        4,
        1,
        64,
    )?)
    .map_err(|_| "TRPG_IDENTITY_ARGON2_CONCURRENCY_INVALID".to_owned())?;
    let mut decision_port = None;
    database_url
        .expose_utf8_to(|database| {
            redis_url.expose_utf8_to(|redis| {
                identity_signing_key.expose_to(|signing_key| {
                    decision_port = Some(GovernedAgentDecisionPort::from_prepared_postgres(
                        ProductionAgentIdentityConfiguration {
                            database_url: database,
                            postgres_ca_certificate_pem: postgres_ca.as_deref(),
                            redis_url: redis,
                            redis_namespace: &redis_namespace,
                            signing_key,
                            session_ttl_ms,
                            argon2_concurrency,
                            redis_root_certificate: redis_ca,
                            redis_client_certificate,
                            redis_client_private_key,
                            workload_id: worker_id,
                            internal_credential_ttl_ms,
                        },
                        policy,
                        audit,
                        canonical,
                    ));
                });
            })
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?
        .map_err(|_| "REDIS_URL_SECRET_INVALID".to_owned())?;
    let decision_port = decision_port
        .ok_or_else(|| "AGENT_DECISION_PORT_CONSTRUCTION_NOT_ATTEMPTED".to_owned())?
        .map_err(|error| error.code().to_owned())?;
    let certification = local_model_certification_from_environment(
        &provider,
        secret_manager,
        witness_url,
    )?;
    let configuration = AgentJobExecutionConfig {
        claim_owner: worker_id.to_owned(),
        lease_duration: Duration::from_millis(bounded_environment_u64(
            "TRPG_AGENT_JOB_LEASE_MS",
            30_000,
            1_000,
            300_000,
        )?),
        heartbeat_interval: Duration::from_millis(bounded_environment_u64(
            "TRPG_AGENT_JOB_HEARTBEAT_MS",
            5_000,
            100,
            60_000,
        )?),
        max_attempts: i32::try_from(bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_ATTEMPTS",
            5,
            1,
            20,
        )?)
        .map_err(|_| "TRPG_AGENT_JOB_MAX_ATTEMPTS_INVALID".to_owned())?,
        max_context_bytes: usize::try_from(bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_CONTEXT_BYTES",
            262_144,
            1_024,
            1_048_576,
        )?)
        .map_err(|_| "TRPG_AGENT_JOB_MAX_CONTEXT_BYTES_INVALID".to_owned())?,
        max_input_tokens: bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_INPUT_TOKENS",
            32_768,
            1,
            1_000_000,
        )?,
        max_output_tokens: bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_OUTPUT_TOKENS",
            4_096,
            1,
            100_000,
        )?,
        max_tool_calls: usize::try_from(bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_TOOL_CALLS",
            1,
            1,
            1,
        )?)
        .map_err(|_| "TRPG_AGENT_JOB_MAX_TOOL_CALLS_INVALID".to_owned())?,
        max_tool_loops: usize::try_from(bounded_environment_u64(
            "TRPG_AGENT_JOB_MAX_TOOL_LOOPS",
            1,
            1,
            1,
        )?)
        .map_err(|_| "TRPG_AGENT_JOB_MAX_TOOL_LOOPS_INVALID".to_owned())?,
    };
    let provider: Arc<dyn ExecutableModelProvider> = Arc::new(provider);
    let repository: Arc<dyn AgentJobRepository> = Arc::new(CanonicalAgentJobRepository::new(
        workflow.clone(),
        canonical_repository,
    ));
    let decisions: Arc<dyn trpg_agent_runtime::agent_job::AgentJobDecisionPort> =
        Arc::new(decision_port);
    let tools: Arc<dyn trpg_agent_runtime::agent_job::AgentJobToolPort> =
        Arc::new(GovernedAgentJobToolPort::new(
            workflow.clone(),
            Arc::new(Coc7AgentSkillCheckRules),
        ));
    AgentJobWorker::new(
        repository,
        provider,
        tools,
        decisions,
        certification,
        configuration,
    )
    .map(Some)
    .map_err(|error| error.code().to_owned())
}
