#[allow(clippy::too_many_arguments)]
fn optional_agent_job_worker_from_environment(
    workflow: DurableWorkflowStore,
    provider: Option<HttpModelProvider<MountedFileSecretResolver>>,
    canonical: PostgresCanonicalStore,
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
    let canonical_runtime = Arc::new(Mutex::new(
        tokio::runtime::Runtime::new()
            .map_err(|_| "AGENT_CANONICAL_RUNTIME_INITIALIZATION_FAILED".to_owned())?,
    ));
    let canonical = Arc::new(PostgresCanonicalCommitPort::new(
        canonical_runtime,
        canonical,
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
    let repository: Arc<dyn trpg_agent_runtime::agent_job::AgentJobRepository> =
        Arc::new(workflow);
    let decisions: Arc<dyn trpg_agent_runtime::agent_job::AgentJobDecisionPort> =
        Arc::new(decision_port);
    let tools: Arc<dyn trpg_agent_runtime::agent_job::AgentJobToolPort> =
        Arc::new(RejectingAgentJobToolPort);
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
