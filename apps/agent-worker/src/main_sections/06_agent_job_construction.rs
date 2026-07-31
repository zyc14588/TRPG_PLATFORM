#[derive(Clone)]
struct CanonicalAgentJobRepository {
    workflow: DurableWorkflowStore,
    canonical: PostgresCanonicalStore,
}

impl CanonicalAgentJobRepository {
    fn new(workflow: DurableWorkflowStore, canonical: PostgresCanonicalStore) -> Self {
        Self {
            workflow,
            canonical,
        }
    }
}

#[trpg_agent_runtime::repository_async_trait]
impl AgentJobRepository for CanonicalAgentJobRepository {
    async fn load(&self, job_id: &str) -> AgentJobResult<Option<DurableAgentJob>> {
        self.workflow
            .load_agent_job(job_id)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn claim_due(
        &self,
        claim_owner: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<Option<DurableAgentJob>> {
        self.workflow
            .claim_due_agent_job(claim_owner, now_unix_ms, lease_duration_ms)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn transition(
        &self,
        draft: &AgentJobTransitionDraft,
    ) -> AgentJobResult<DurableAgentJob> {
        self.workflow
            .transition_agent_job(draft)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn heartbeat(
        &self,
        job_id: &str,
        claim_owner: &str,
        claim_token: &str,
        now_unix_ms: i64,
        lease_duration_ms: i64,
    ) -> AgentJobResult<bool> {
        self.workflow
            .heartbeat_agent_job(
                job_id,
                claim_owner,
                claim_token,
                now_unix_ms,
                lease_duration_ms,
            )
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn cancellation_requested(&self, job_id: &str) -> AgentJobResult<bool> {
        self.workflow
            .agent_job_cancellation_requested(job_id)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn load_authority(
        &self,
        campaign_id: &str,
    ) -> AgentJobResult<DurableAgentAuthoritySnapshot> {
        self.workflow
            .load_agent_authority_snapshot(campaign_id)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn load_context(&self, job_id: &str) -> AgentJobResult<DurableAgentContextSnapshot> {
        let job = self
            .workflow
            .load_agent_job(job_id)
            .await
            .map_err(map_agent_workflow_error)?
            .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_NOT_FOUND"))?;
        let mut context = self
            .workflow
            .load_agent_job_context(job_id)
            .await
            .map_err(map_agent_workflow_error)?;
        let after_sequence = job
            .input_event_sequence
            .checked_sub(1)
            .ok_or_else(|| AgentJobError::terminal("AGENT_CANONICAL_INPUT_INVALID"))?;
        let mut events = self
            .canonical
            .load_replay_page(&job.campaign_id, after_sequence, 1)
            .await
            .map_err(map_agent_canonical_context_error)?;
        let event = events
            .pop()
            .ok_or_else(|| AgentJobError::terminal("AGENT_CANONICAL_INPUT_MISSING"))?;
        let visibility_scope: serde_json::Value =
            serde_json::from_str(&job.visibility_scope_json)
                .map_err(|_| AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"))?;
        let visibility_allowed = visibility_scope
            .get("allowed_labels")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|labels| {
                labels
                    .iter()
                    .any(|label| label.as_str() == Some(&event.visibility_label))
            });
        let payload_matches = event
            .payload
            .get("job_id")
            .and_then(serde_json::Value::as_str)
            == Some(job.job_id.as_str())
            && event
                .payload
                .get("authority_contract_id")
                .and_then(serde_json::Value::as_str)
                == Some(job.authority_contract_id.as_str())
            && event
                .payload
                .get("route_authorization_event_id")
                .and_then(serde_json::Value::as_str)
                == Some(job.route_authorization_event_id.as_str())
            && event
                .payload
                .get("rag_snapshot_id")
                .and_then(serde_json::Value::as_str)
                == Some(job.rag_snapshot_id.as_str())
            && event
                .payload
                .get("provider_id")
                .and_then(serde_json::Value::as_str)
                == Some(job.provider_id.as_str())
            && event
                .payload
                .get("model_id")
                .and_then(serde_json::Value::as_str)
                == Some(job.model_id.as_str())
            && event
                .payload
                .get("agent_kind")
                .and_then(serde_json::Value::as_str)
                == Some(job.agent_kind.as_str())
            && event
                .payload
                .get("authority_mode")
                .and_then(serde_json::Value::as_str)
                == Some(job.authority_mode.as_str())
            && event
                .payload
                .get("authority_contract_version")
                .and_then(serde_json::Value::as_i64)
                == Some(job.authority_contract_version)
            && event.payload.get("visibility_scope") == Some(&visibility_scope)
            && event.payload.get("input").is_some();
        if event.sequence != job.input_event_sequence
            || event.stream_version != job.input_stream_version
            || event.expected_version.checked_add(1) != Some(event.stream_version)
            || event.stream_id != job.input_stream_id
            || event.event_type != "AgentJobRequested"
            || event.campaign_id != job.campaign_id
            || event.authority_mode != job.authority_mode.to_ascii_lowercase()
            || event.authority_contract_id != job.authority_contract_id
            || event.authority_contract_version != job.authority_contract_version
            || event.authority_owner
                != self
                    .workflow
                    .load_agent_authority_snapshot(&job.campaign_id)
                    .await
                    .map_err(map_agent_workflow_error)?
                    .authority_owner
            || event.resource_type != "agent_job"
            || event.resource_id != job.job_id
            || event.integrity_status != "verified_hmac"
            || event.request_hash_source != "formal_commit"
            || event.event_integrity_hash.is_none()
            || !visibility_allowed
            || !payload_matches
        {
            return Err(AgentJobError::terminal(
                "AGENT_CANONICAL_INPUT_BINDING_MISMATCH",
            ));
        }
        context.input_payload_json = serde_json::to_string(&event.payload)
            .map_err(|_| AgentJobError::terminal("AGENT_CANONICAL_INPUT_INVALID"))?;
        Ok(context)
    }

    async fn load_approval(
        &self,
        job_id: &str,
    ) -> AgentJobResult<Option<DurableAgentApproval>> {
        self.workflow
            .load_agent_job_approval(job_id)
            .await
            .map_err(map_agent_workflow_error)
    }

    async fn append_evidence(&self, draft: &AgentJobEvidenceDraft) -> AgentJobResult<()> {
        self.workflow
            .append_agent_job_evidence(draft)
            .await
            .map_err(map_agent_workflow_error)
    }
}

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

fn agent_policy_and_audit_from_environment(
    secret_manager: &SecretManager<MountedFileSecretResolver>,
) -> Result<(OpenFgaOpaPolicyAdapter, FileAuditLog), String> {
    let openfga_address = required_environment("TRPG_OPENFGA_ADDRESS")?
        .parse()
        .map_err(|_| "TRPG_OPENFGA_ADDRESS_INVALID".to_owned())?;
    let openfga_store_id = required_environment_or_file(
        "TRPG_OPENFGA_STORE_ID",
        "TRPG_OPENFGA_STORE_ID_FILE",
    )?;
    let openfga_model_id = required_environment_or_file(
        "TRPG_OPENFGA_MODEL_ID",
        "TRPG_OPENFGA_MODEL_ID_FILE",
    )?;
    let opa_address = required_environment("TRPG_OPA_ADDRESS")?
        .parse()
        .map_err(|_| "TRPG_OPA_ADDRESS_INVALID".to_owned())?;
    let opa_revision = required_environment("TRPG_OPA_POLICY_REVISION")?;
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga_address,
            format!("/stores/{openfga_store_id}/check"),
            PolicyBackend::OpenFga,
            openfga_model_id,
        )
        .map_err(|_| "OPENFGA_POLICY_CONFIGURATION_INVALID".to_owned())?,
        HttpPolicyEndpoint::new(
            opa_address,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            opa_revision,
        )
        .map_err(|_| "OPA_POLICY_CONFIGURATION_INVALID".to_owned())?,
    )
    .map_err(|_| "POLICY_CONFIGURATION_INVALID".to_owned())?;
    let audit_path = required_environment("TRPG_AUDIT_LOG_PATH")?;
    let audit_key_id = required_environment("TRPG_AUDIT_HMAC_KEY_ID")?;
    let audit_key = resolve_mounted_secret(secret_manager, "TRPG_AUDIT_HMAC_KEY")?
        .to_key32()
        .map_err(|_| "AUDIT_HMAC_KEY_INVALID".to_owned())?;
    let audit = audit_key
        .expose_to(|key| FileAuditLog::open(&audit_path, &audit_key_id, key))
        .map_err(|_| "AUDIT_LOG_CONFIGURATION_INVALID".to_owned())?;
    Ok((policy, audit))
}

fn local_model_certification_from_environment(
    provider: &HttpModelProvider<MountedFileSecretResolver>,
    secret_manager: &SecretManager<MountedFileSecretResolver>,
    witness_url: &SecretValue,
) -> Result<Option<CertifiedLocalModel>, String> {
    if provider.provider_type() == ProviderType::Cloud {
        return Ok(None);
    }
    let certificate_path =
        PathBuf::from(required_environment("TRPG_LOCAL_MODEL_CERTIFICATE_PATH")?);
    let registry_path =
        PathBuf::from(required_environment("TRPG_LOCAL_MODEL_CERTIFICATION_REGISTRY_PATH")?);
    validate_regular_absolute_file(&certificate_path)?;
    validate_regular_absolute_file(&registry_path)?;
    let certificate: LocalModelCertificate = serde_json::from_slice(
        &fs::read(certificate_path)
            .map_err(|_| "LOCAL_MODEL_CERTIFICATE_UNREADABLE".to_owned())?,
    )
    .map_err(|_| "LOCAL_MODEL_CERTIFICATE_INVALID".to_owned())?;
    let signing_key_id =
        required_environment("TRPG_LOCAL_MODEL_CERTIFICATION_HMAC_KEY_ID")?;
    let signing_key =
        resolve_mounted_secret(secret_manager, "TRPG_LOCAL_MODEL_CERTIFICATION_HMAC_KEY")?
            .to_key32()
            .map_err(|_| "LOCAL_MODEL_CERTIFICATION_HMAC_KEY_INVALID".to_owned())?;
    let checkpoint = witness_url
        .expose_utf8_to(PostgresLedgerCheckpointStore::connect)
        .map_err(|_| "WITNESS_DATABASE_URL_SECRET_INVALID".to_owned())?
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_WITNESS_INVALID".to_owned())?;
    let mut authority = None;
    signing_key.expose_to(|key| {
        authority = Some(LocalModelCertificationAuthority::new_with_checkpoint(
            signing_key_id,
            key,
            registry_path,
            Arc::new(checkpoint),
        ));
    });
    let authority = authority
        .ok_or_else(|| "LOCAL_MODEL_CERTIFICATION_AUTHORITY_NOT_ATTEMPTED".to_owned())?
        .map_err(|error| error.code().to_owned())?;
    Ok(Some(CertifiedLocalModel::new(
        Arc::new(authority),
        certificate,
    )))
}

fn required_environment_or_file(name: &str, file_name: &str) -> Result<String, String> {
    if let Some(value) = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(value);
    }
    let path = PathBuf::from(required_environment(file_name)?);
    validate_regular_absolute_file(&path)?;
    fs::read_to_string(path)
        .map(|value| value.trim().to_owned())
        .map_err(|_| format!("{file_name}_UNREADABLE"))
        .and_then(|value| {
            if value.is_empty() {
                Err(format!("{name}_REQUIRED"))
            } else {
                Ok(value)
            }
        })
}

fn bounded_environment_u64(
    name: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    let value = match std::env::var(name) {
        Err(std::env::VarError::NotPresent) => default,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(format!("{name}_INVALID"));
        }
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| format!("{name}_INVALID"))?,
    };
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{name}_INVALID"));
    }
    Ok(value)
}

fn current_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(i64::MAX)
}
