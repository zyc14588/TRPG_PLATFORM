
impl AgentWorkerProcess {
    fn from_environment() -> Result<Self, String> {
        let secret_manager = Arc::new(production_secret_manager()?);
        let model_provider =
            optional_model_provider_from_environment(Arc::clone(&secret_manager))?;
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
                    FilesystemDeletionSurface::new(export_root, DeletionTarget::Export)
                        .map_err(|_| "DELETION_EXPORT_SURFACE_INVALID".to_owned())?,
                ),
                Box::new(BackupKeyDeletionSurface::new(
                    deletion_repository.pool().clone(),
                )),
                Box::new(queue_surface),
            ],
        )
        .map_err(|_| "DELETION_WORKER_CONFIGURATION_INVALID".to_owned())?;
        Ok(Self {
            runtime,
            workflow,
            outbox,
            deletion,
            plugins,
            model_provider,
        })
    }

    fn start(self) -> Result<(RoleRuntimeProbe, BackgroundWorker), String> {
        let background_health = Arc::new(Mutex::new(BackgroundWorkerHealth::default()));
        let (shutdown_sender, shutdown_receiver) = mpsc::channel();
        let background_outbox = self.outbox.clone();
        let background_workflow = self.workflow.clone();
        let background_deletion = self.deletion;
        let background_runtime = self.runtime;
        let background_health_writer = Arc::clone(&background_health);
        let worker = thread::Builder::new()
            .name("agent-outbox-publisher".to_owned())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| loop {
                    let (delivery, projection, deletion) = background_runtime.block_on(async {
                        let delivery = async {
                            background_workflow.check_readiness().await.map_err(|_| {
                                trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxError::Database(
                                    "workflow_readiness",
                                )
                            })?;
                            background_outbox.stream_message_count().await?;
                            background_outbox.publish_batch().await
                        }
                        .await;
                        // Projection recovery is an independent Event Store read-model
                        // responsibility. A NATS outage must not prevent it from
                        // converging, and its failure must retain a distinct health
                        // classification from event delivery.
                        let projection = background_outbox.rebuild_projections_to_tip().await;
                        // Deletion is a durable, evidence-gated workflow. Its failures
                        // are independent of delivery/projection failures and therefore
                        // receive a distinct fail-closed health classification.
                        let deletion = background_deletion.execute_next(25).await;
                        (delivery, projection, deletion)
                    });
                    if let Ok(result) = &delivery {
                        if result.requires_operator_attention() {
                            let alert = result.alert_code().unwrap_or("OUTBOX_DELIVERY_ALERT");
                            eprintln!(
                                "service=agent-worker alert={alert} dead_lettered={} dead_letter_total={} failed={} claimed={}",
                                result.dead_lettered,
                                result.dead_letter_total,
                                result.failed,
                                result.claimed
                            );
                        }
                    }
                    if let Ok(mut health) = background_health_writer.lock() {
                        health.record_cycle(
                            Instant::now(),
                            background_cycle_error(&delivery, &projection, &deletion),
                        );
                    }
                    match shutdown_receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }));
                if let Ok(mut health) = background_health_writer.lock() {
                    health.record_stopped(if outcome.is_ok() {
                        "AGENT_WORKER_BACKGROUND_STOPPED"
                    } else {
                        "AGENT_WORKER_BACKGROUND_PANICKED"
                    });
                }
            })
            .map_err(|_| "OUTBOX_WORKER_START_FAILED".to_owned())?;

        let plugins = self.plugins;
        let model_provider = self.model_provider;
        let probe_health = Arc::clone(&background_health);
        let probe = RoleRuntimeProbe::spawn("agent_worker_runtime", move || {
            let boundary = trpg_agent_runtime::provider_boundary_snapshot();
            if boundary.gateway != "Agent Gateway"
                || boundary.runtime != "Agent Orchestrator/Runtime"
                || boundary.provider_adapter != "Model Provider Adapter"
                || boundary.forbidden_direct_call_error != "DIRECT_LLM_CALL_FORBIDDEN"
            {
                return Err("provider boundary initialization is incomplete".to_owned());
            }
            let provider_status = if let Some(model_provider) = &model_provider {
                let model_route = model_provider.startup_route_snapshot();
                if model_route.fallback_policy != "none_no_automatic_fallback"
                    || model_route.privacy_boundary != "explicit_route_authorization_event"
                {
                    return Err("model provider route authorization is incomplete".to_owned());
                }
                model_provider.provider_type().route_name()
            } else {
                "transport_only_awaiting_ar09"
            };
            if let Some(error) = probe_health
                .lock()
                .map_err(|_| "outbox health lock poisoned".to_owned())?
                .readiness_error(Instant::now(), BACKGROUND_HEARTBEAT_STALE_AFTER)
            {
                return Err(error);
            }
            plugins.check_readiness()?;
            Ok(format!(
                "gateway/runtime/provider adapter ready; provider_status={}; durable workflow and sandboxed plugins ready; eventing_workers_status=enabled; plugins={}",
                provider_status,
                plugins.plugin_count(),
            ))
        })
        .map_err(|error| error.to_string())?;
        Ok((
            probe,
            BackgroundWorker {
                shutdown_sender,
                worker: Some(worker),
            },
        ))
    }
}

#[derive(Debug, Default)]
struct BackgroundWorkerHealth {
    last_cycle_completed_at: Option<Instant>,
    cycle_error: Option<String>,
    stopped_error: Option<&'static str>,
}

impl BackgroundWorkerHealth {
    fn record_cycle(&mut self, completed_at: Instant, cycle_error: Option<String>) {
        self.last_cycle_completed_at = Some(completed_at);
        self.cycle_error = cycle_error;
        self.stopped_error = None;
    }

    fn record_stopped(&mut self, error: &'static str) {
        self.stopped_error = Some(error);
    }

    fn readiness_error(&self, now: Instant, stale_after: Duration) -> Option<String> {
        if let Some(error) = self.stopped_error {
            return Some(error.to_owned());
        }
        let Some(completed_at) = self.last_cycle_completed_at else {
            return Some("AGENT_WORKER_DEPENDENCY_CHECK_PENDING".to_owned());
        };
        if now.saturating_duration_since(completed_at) > stale_after {
            return Some("AGENT_WORKER_BACKGROUND_HEARTBEAT_STALE".to_owned());
        }
        self.cycle_error.clone()
    }
}
