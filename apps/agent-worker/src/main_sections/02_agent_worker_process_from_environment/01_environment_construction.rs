
impl AgentWorkerProcess {
    fn from_environment() -> Result<Self, String> {
        let secret_manager = Arc::new(production_secret_manager()?);
        let model_provider =
            optional_model_provider_from_environment(Arc::clone(&secret_manager))?;
        let model_route = model_provider
            .as_ref()
            .map(ExecutableModelProvider::startup_route_snapshot);
        let database_url = resolve_mounted_secret(&secret_manager, "TRPG_DATABASE_URL")?;
        let eventing_workers_enabled =
            boolean_environment("TRPG_P04_EVENTING_WORKERS_ENABLED", true)?;
        require_eventing_workers_enabled(eventing_workers_enabled)?;
        let worker_id = std::env::var("TRPG_AGENT_WORKER_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "agent-worker-primary".to_owned());
        let plugin_registry_path = required_environment("TRPG_PLUGIN_REGISTRY_PATH")?;
        let plugins = PluginRuntime::load(Path::new(&plugin_registry_path))?;

        let runtime = tokio::runtime::Runtime::new()
            .map_err(|_| "AGENT_WORKER_RUNTIME_INITIALIZATION_FAILED".to_owned())?;
        let mut workflow_connection = None;
        database_url
            .expose_utf8_to(|database| {
                workflow_connection =
                    Some(runtime.block_on(DurableWorkflowStore::connect(database)));
            })
            .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?;
        let workflow = workflow_connection
            .ok_or_else(|| "DURABLE_WORKFLOW_CONNECTION_NOT_ATTEMPTED".to_owned())?
            .map_err(|_| "DURABLE_WORKFLOW_CONNECTION_FAILED".to_owned())?;
        runtime
            .block_on(workflow.check_readiness())
            .map_err(|error| format!("DURABLE_WORKFLOW_NOT_READY:{error}"))?;
        let nats_url = resolve_mounted_secret(&secret_manager, "TRPG_NATS_URL")?;
        let witness_url = resolve_mounted_secret(&secret_manager, "TRPG_WITNESS_DATABASE_URL")?;
        let integrity_key_id = required_environment("TRPG_CANONICAL_HMAC_KEY_ID")?;
        let integrity_key = resolve_mounted_secret(&secret_manager, "TRPG_CANONICAL_HMAC_KEY")?
            .to_key32()
            .map_err(|_| "CANONICAL_HMAC_KEY_INVALID".to_owned())?;
        let payload_key_id = required_environment("TRPG_PAYLOAD_ENCRYPTION_KEY_ID")?;
        let payload_key = resolve_mounted_secret(&secret_manager, "TRPG_PAYLOAD_ENCRYPTION_KEY")?
            .to_key32()
            .map_err(|_| "PAYLOAD_ENCRYPTION_KEY_INVALID".to_owned())?;
        let canonical = database_url
            .expose_utf8_to(|database| {
                witness_url.expose_utf8_to(|witness| {
                    integrity_key.expose_to(|integrity| {
                        payload_key.expose_to(|payload| {
                            runtime.block_on(PostgresCanonicalStore::connect(
                                database,
                                witness,
                                &integrity_key_id,
                                integrity,
                                &payload_key_id,
                                payload,
                            ))
                        })
                    })
                })
            })
            .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?
            .map_err(|_| "WITNESS_DATABASE_URL_SECRET_INVALID".to_owned())?
            .map_err(|_| "CANONICAL_STORE_CONNECTION_FAILED".to_owned())?;
        runtime
            .block_on(canonical.verify_integrity())
            .map_err(|_| "CANONICAL_STORE_NOT_READY".to_owned())?;
        let agent_commit_canonical = if model_provider.is_some() {
            let canonical_database_url =
                resolve_mounted_secret(&secret_manager, "TRPG_CANONICAL_DATABASE_URL")?;
            let store = canonical_database_url
                .expose_utf8_to(|database| {
                    witness_url.expose_utf8_to(|witness| {
                        integrity_key.expose_to(|integrity| {
                            payload_key.expose_to(|payload| {
                                runtime.block_on(PostgresCanonicalStore::connect(
                                    database,
                                    witness,
                                    &integrity_key_id,
                                    integrity,
                                    &payload_key_id,
                                    payload,
                                ))
                            })
                        })
                    })
                })
                .map_err(|_| "CANONICAL_DATABASE_URL_SECRET_INVALID".to_owned())?
                .map_err(|_| "WITNESS_DATABASE_URL_SECRET_INVALID".to_owned())?
                .map_err(|_| "AGENT_CANONICAL_STORE_CONNECTION_FAILED".to_owned())?;
            runtime
                .block_on(store.verify_integrity())
                .map_err(|_| "AGENT_CANONICAL_STORE_NOT_READY".to_owned())?;
            Some(store)
        } else {
            None
        };
        let nats_ca = optional_path("TRPG_NATS_CA_CERT_PATH")?;
        let nats_client_certificate = optional_path("TRPG_NATS_CLIENT_CERT_PATH")?;
        let nats_client_private_key = optional_path("TRPG_NATS_CLIENT_KEY_PATH")?;
        let nats_credentials = optional_path("TRPG_NATS_CREDENTIALS_PATH")?;
        let mut outbox_result = None;
        nats_url
            .expose_utf8_to(|nats| {
                outbox_result = Some(runtime.block_on(
                    JetStreamOutboxPublisher::connect_with_credentials(
                        canonical.clone(),
                        nats,
                        &worker_id,
                        nats_ca.as_deref(),
                        nats_client_certificate.as_deref(),
                        nats_client_private_key.as_deref(),
                        nats_credentials.as_deref(),
                    ),
                ));
            })
            .map_err(|_| "NATS_URL_SECRET_INVALID".to_owned())?;
        let outbox_result =
            outbox_result.ok_or_else(|| "JETSTREAM_OUTBOX_CONNECTION_NOT_ATTEMPTED".to_owned())?;
        let outbox = outbox_result.map_err(|_| "JETSTREAM_OUTBOX_CONNECTION_FAILED".to_owned())?;
        runtime
            .block_on(outbox.check_readiness())
            .map_err(|_| "JETSTREAM_OUTBOX_NOT_READY".to_owned())?;
        let redis_url = resolve_mounted_secret(&secret_manager, "TRPG_REDIS_URL")?;
        let redis_ca = optional_file_bytes("TRPG_REDIS_CA_CERT_PATH")?;
        let redis_client_certificate = optional_file_bytes("TRPG_REDIS_CLIENT_CERT_PATH")?;
        let redis_client_private_key = optional_file_bytes("TRPG_REDIS_CLIENT_KEY_PATH")?;
        let mut deletion_repository = None;
        database_url
            .expose_utf8_to(|database| {
                deletion_repository =
                    Some(runtime.block_on(PostgresDeletionRepository::connect(database)));
            })
            .map_err(|_| "DATABASE_URL_SECRET_INVALID".to_owned())?;
        let deletion_repository = deletion_repository
            .ok_or_else(|| "DELETION_DATABASE_CONNECTION_NOT_ATTEMPTED".to_owned())?
            .map_err(|_| "DELETION_DATABASE_CONNECTION_FAILED".to_owned())?;
        runtime
            .block_on(deletion_repository.check_readiness())
            .map_err(|_| "DELETION_SCHEMA_NOT_READY".to_owned())?;
        let mut cache_surface = None;
        redis_url
            .expose_utf8_to(|redis| {
                cache_surface = Some(runtime.block_on(
                    RedisCacheDeletionSurface::connect_with_tls(
                        redis,
                        "trpg:realtime:projection",
                        redis_ca.as_deref(),
                        redis_client_certificate.as_deref(),
                        redis_client_private_key.as_deref(),
                    ),
                ));
            })
            .map_err(|_| "REDIS_URL_SECRET_INVALID".to_owned())?;
        let cache_surface = cache_surface
            .ok_or_else(|| "DELETION_CACHE_CONNECTION_NOT_ATTEMPTED".to_owned())?
            .map_err(|_| "DELETION_CACHE_CONNECTION_FAILED".to_owned())?;
        let mut queue_surface = None;
        nats_url
            .expose_utf8_to(|nats| {
                queue_surface = Some(runtime.block_on(
                    NatsQueueDeletionSurface::connect_crypto_erasure_with_credentials(
                        nats,
                        "TRPG_CANONICAL_EVENTS",
                        deletion_repository.pool().clone(),
                        nats_ca.as_deref(),
                        nats_client_certificate.as_deref(),
                        nats_client_private_key.as_deref(),
                        nats_credentials.as_deref(),
                    ),
                ));
            })
            .map_err(|_| "NATS_URL_SECRET_INVALID".to_owned())?;
        let queue_surface = queue_surface
            .ok_or_else(|| "DELETION_QUEUE_CONNECTION_NOT_ATTEMPTED".to_owned())?
            .map_err(|_| "DELETION_QUEUE_CONNECTION_FAILED".to_owned())?;
        let object_endpoint = required_environment("TRPG_OBJECT_STORAGE_ENDPOINT")?;
        let object_region = required_environment("TRPG_OBJECT_STORAGE_REGION")?;
        let object_bucket = required_environment("TRPG_OBJECT_STORAGE_BUCKET")?;
        let object_ca_bundle = optional_path("TRPG_OBJECT_STORAGE_CA_CERT_PATH")?
            .ok_or_else(|| "TRPG_OBJECT_STORAGE_CA_CERT_PATH_REQUIRED".to_owned())?;
        let object_access_key =
            resolve_mounted_secret(&secret_manager, "TRPG_OBJECT_STORAGE_ACCESS_KEY")?;
        let object_secret_key =
            resolve_mounted_secret(&secret_manager, "TRPG_OBJECT_STORAGE_SECRET_KEY")?;
        let object_surface = object_access_key
            .expose_utf8_to(|access_key| {
                object_secret_key.expose_utf8_to(|secret_key| {
                    runtime.block_on(S3ObjectDeletionSurface::connect(
                        &object_endpoint,
                        &object_region,
                        &object_bucket,
                        access_key,
                        secret_key,
                        &object_ca_bundle,
                    ))
                })
            })
            .map_err(|_| "OBJECT_STORAGE_ACCESS_KEY_INVALID".to_owned())?
            .map_err(|_| "OBJECT_STORAGE_SECRET_KEY_INVALID".to_owned())?
            .map_err(|_| "DELETION_OBJECT_SURFACE_CONNECTION_FAILED".to_owned())?;
        let export_root = PathBuf::from(required_environment("TRPG_EXPORT_STORAGE_ROOT")?);
        let export_retention_seconds = std::env::var("TRPG_EXPORT_RETENTION_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| (60..=366 * 24 * 60 * 60).contains(value))
            .unwrap_or(7 * 24 * 60 * 60);
        let campaign_exports = CampaignExportWorker::new(
            deletion_repository.pool().clone(),
            canonical.clone(),
            export_root.clone(),
            format!("{worker_id}-export"),
            Duration::from_secs(export_retention_seconds),
        )
        .map_err(|error| format!("CAMPAIGN_EXPORT_WORKER_INVALID:{}", error.code()))?;
        runtime
            .block_on(campaign_exports.check_readiness())
            .map_err(|error| format!("CAMPAIGN_EXPORT_WORKER_NOT_READY:{}", error.code()))?;
        let legal_holds = Arc::new(PostgresLegalHoldResolver::new(
            deletion_repository.pool().clone(),
        ));
        let deletion = DeletionWorker::new(
            deletion_repository.clone(),
            legal_holds,
            vec![
                Box::new(
                    PostgresRecordDeletionSurface::new(
                        deletion_repository.pool().clone(),
                        DeletionTarget::Database,
                    )
                    .map_err(|_| "DELETION_DATABASE_SURFACE_INVALID".to_owned())?,
                ),
                Box::new(
                    PostgresRecordDeletionSurface::new(
                        deletion_repository.pool().clone(),
                        DeletionTarget::RagIndex,
                    )
                    .map_err(|_| "DELETION_RAG_SURFACE_INVALID".to_owned())?,
                ),
                Box::new(object_surface),
                Box::new(cache_surface),
                Box::new(
                    CampaignExportDeletionSurface::new(
                        deletion_repository.pool().clone(),
                        export_root,
                    )
                    .map_err(|_| "DELETION_EXPORT_SURFACE_INVALID".to_owned())?,
                ),
                Box::new(BackupKeyDeletionSurface::new(
                    deletion_repository.pool().clone(),
                )),
                Box::new(queue_surface),
            ],
        )
        .map_err(|_| "DELETION_WORKER_CONFIGURATION_INVALID".to_owned())?;
        if model_provider.is_some() {
            runtime
                .block_on(workflow.check_agent_job_readiness())
                .map_err(|error| format!("AGENT_JOB_SCHEMA_NOT_READY:{error}"))?;
        }
        let agent_jobs = optional_agent_job_worker_from_environment(
            workflow.clone(),
            model_provider,
            canonical,
            agent_commit_canonical,
            &secret_manager,
            &database_url,
            &redis_url,
            &witness_url,
            &worker_id,
            redis_ca.as_deref(),
            redis_client_certificate.as_deref(),
            redis_client_private_key.as_deref(),
        )?;
        Ok(Self {
            runtime,
            workflow,
            outbox,
            deletion,
            campaign_exports,
            plugins,
            model_route,
            agent_jobs,
        })
    }

}
