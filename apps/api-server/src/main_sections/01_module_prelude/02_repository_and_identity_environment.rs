fn core_domain_repository_from_environment(
    database_url: &SecretValue,
    runtime: &tokio::runtime::Runtime,
    canonical_store: PostgresCanonicalStore,
) -> Result<CoreDomainRepository, String> {
    let mut connection = None;
    database_url
        .expose_utf8_to(|database| {
            connection =
                Some(runtime.block_on(CoreDomainRepository::connect(database, canonical_store)));
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?;
    connection
        .ok_or_else(|| "CORE_DOMAIN_DATABASE_CONNECTION_NOT_ATTEMPTED".to_owned())?
        .map_err(|_| "CORE_DOMAIN_DATABASE_CONNECTION_FAILED".to_owned())
}

fn optional_agent_job_route_from_environment(
) -> Result<Option<AgentJobRouteConfiguration>, String> {
    let provider_type = match std::env::var("TRPG_MODEL_PROVIDER_TYPE") {
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err("TRPG_MODEL_PROVIDER_TYPE_INVALID".to_owned())
        }
        Ok(value) if value.trim().is_empty() => {
            return Err("TRPG_MODEL_PROVIDER_TYPE_INVALID".to_owned())
        }
        Ok(value) => value,
    };
    Ok(Some(AgentJobRouteConfiguration {
        provider_id: required_environment("TRPG_MODEL_PROVIDER_ID")?,
        provider_type,
        model_id: required_environment("TRPG_MODEL_ID")?,
        model_artifact_sha256: required_environment("TRPG_MODEL_ARTIFACT_SHA256")?,
        route_authorization_event_id: required_environment(
            "TRPG_MODEL_ROUTE_AUTHORIZATION_EVENT_ID",
        )?,
    }))
}

fn agent_workflow_from_environment(
    database_url: &SecretValue,
    runtime: &tokio::runtime::Runtime,
) -> Result<DurableWorkflowStore, String> {
    let mut connection = None;
    database_url
        .expose_utf8_to(|database| {
            connection = Some(runtime.block_on(DurableWorkflowStore::connect(database)));
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?;
    let workflow = connection
        .ok_or_else(|| "AGENT_JOB_DATABASE_CONNECTION_NOT_ATTEMPTED".to_owned())?
        .map_err(|_| "AGENT_JOB_DATABASE_CONNECTION_FAILED".to_owned())?;
    runtime
        .block_on(workflow.check_agent_job_readiness())
        .map_err(|_| "AGENT_JOB_SCHEMA_NOT_READY".to_owned())?;
    Ok(workflow)
}

fn deletion_repository_from_environment(
    database_url: &SecretValue,
) -> Result<(tokio::runtime::Runtime, PostgresDeletionRepository), String> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|_| "PRIVACY_RUNTIME_INITIALIZATION_FAILED".to_owned())?;
    let mut connection = None;
    database_url
        .expose_utf8_to(|database| {
            connection = Some(runtime.block_on(PostgresDeletionRepository::connect(database)));
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?;
    let repository = connection
        .ok_or_else(|| "DELETION_DATABASE_CONNECTION_NOT_ATTEMPTED".to_owned())?
        .map_err(|_| "DELETION_DATABASE_CONNECTION_FAILED".to_owned())?;
    runtime
        .block_on(repository.check_readiness())
        .map_err(|_| "DELETION_SCHEMA_NOT_READY".to_owned())?;
    Ok((runtime, repository))
}

fn canonical_store_from_environment(
    secret_manager: &SecretManager<MountedFileSecretResolver>,
) -> Result<(tokio::runtime::Runtime, PostgresCanonicalStore), String> {
    let database_url = resolve_mounted_secret(secret_manager, "TRPG_CANONICAL_DATABASE_URL")?;
    let witness_database_url = resolve_mounted_secret(secret_manager, "TRPG_WITNESS_DATABASE_URL")?;
    let integrity_key_id = required_environment("TRPG_CANONICAL_HMAC_KEY_ID")?;
    let integrity_key = resolve_mounted_secret(secret_manager, "TRPG_CANONICAL_HMAC_KEY")?
        .to_key32()
        .map_err(|_| "CANONICAL_HMAC_KEY_INVALID".to_owned())?;
    let payload_key_id = required_environment("TRPG_PAYLOAD_ENCRYPTION_KEY_ID")?;
    let payload_key = resolve_mounted_secret(secret_manager, "TRPG_PAYLOAD_ENCRYPTION_KEY")?
        .to_key32()
        .map_err(|_| "PAYLOAD_ENCRYPTION_KEY_INVALID".to_owned())?;
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|_| "CANONICAL_RUNTIME_INITIALIZATION_FAILED".to_owned())?;
    let mut connection = None;
    database_url
        .expose_utf8_to(|primary| {
            witness_database_url
                .expose_utf8_to(|witness| {
                    integrity_key.expose_to(|integrity| {
                        payload_key.expose_to(|payload| {
                            connection = Some(runtime.block_on(PostgresCanonicalStore::connect(
                                primary,
                                witness,
                                integrity_key_id,
                                integrity,
                                payload_key_id,
                                payload,
                            )));
                        });
                    });
                })
                .map_err(|_| "WITNESS_DATABASE_URL_SECRET_INVALID".to_owned())
        })
        .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())??;
    let store = connection
        .ok_or_else(|| "CANONICAL_STORE_CONNECTION_NOT_ATTEMPTED".to_owned())?
        .map_err(|error| format!("CANONICAL_STORE_CONNECTION_FAILED:{error}"))?;
    runtime
        .block_on(store.verify_integrity())
        .map_err(|error| format!("CANONICAL_STORE_NOT_READY:{error}"))?;
    Ok((runtime, store))
}

struct IdentitySecretConfiguration<'a> {
    database_url: &'a SecretValue,
    redis_url: &'a SecretValue,
    signing_key: &'a SecretKey32,
    postgres_ca: Option<&'a [u8]>,
    redis_ca: Option<&'a [u8]>,
    redis_client_certificate: Option<&'a [u8]>,
    redis_client_private_key: Option<&'a [u8]>,
    redis_namespace: &'a str,
    session_ttl_ms: u64,
    argon2_concurrency: usize,
}

fn identity_from_secrets(
    configuration: IdentitySecretConfiguration<'_>,
) -> Result<IdentityService, trpg_identity::IdentityError> {
    let mut identity = None;
    let database_result = configuration.database_url.expose_utf8_to(|database| {
        configuration.redis_url.expose_utf8_to(|redis| {
            configuration.signing_key.expose_to(|key| {
                identity = Some(
                    IdentityService::from_prepared_postgres_with_security_and_redis_tls(
                        database,
                        configuration.postgres_ca,
                        redis,
                        configuration.redis_namespace,
                        key,
                        configuration.session_ttl_ms,
                        configuration.argon2_concurrency,
                        configuration.redis_ca,
                        configuration.redis_client_certificate,
                        configuration.redis_client_private_key,
                    ),
                );
            });
        })
    });
    database_result
        .map_err(|_| trpg_identity::IdentityError::InvalidIdentityData)?
        .map_err(|_| trpg_identity::IdentityError::InvalidIdentityData)?;
    identity.unwrap_or(Err(trpg_identity::IdentityError::InvalidIdentityData))
}

fn optional_file_from_environment(name: &str) -> Result<Option<Vec<u8>>, String> {
    let Some(path) = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    std::fs::read(path)
        .map(Some)
        .map_err(|_| format!("{name}_UNREADABLE"))
}
