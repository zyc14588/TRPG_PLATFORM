use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Deserialize;
use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_data_eventing::event_bus_nats_impl::{JetStreamOutboxPublisher, PublishBatchResult};
use trpg_data_eventing::event_store_sqlx_outbox_projection::PostgresCanonicalStore;
use trpg_extension_sdk::plugin_host::{HostedPlugin, HostedPluginManifest, PluginHost};
use trpg_extension_sdk::{ExtensionCapability, ExtensionCapabilityGrantSet};
use trpg_runtime::durable_workflow::DurableWorkflowStore;
use trpg_security_governance::secret::{
    MountedFileSecretResolver, SecretManager, SecretReference, SecretValue,
};
use trpg_security_governance::security_privacy::{
    BackupKeyDeletionSurface, DeletionTarget, DeletionWorker, FilesystemDeletionSurface,
    NatsQueueDeletionSurface, PostgresDeletionRepository, PostgresLegalHoldResolver,
    PostgresRecordDeletionSurface, RedisCacheDeletionSurface, S3ObjectDeletionSurface,
};

const BACKGROUND_HEARTBEAT_STALE_AFTER: Duration = Duration::from_secs(30);

fn main() -> ExitCode {
    let worker = match AgentWorkerProcess::from_environment() {
        Ok(worker) => worker,
        Err(error) => {
            eprintln!("service=agent-worker error={error}");
            return ExitCode::FAILURE;
        }
    };
    let (runtime_probe, background_worker) = match worker.start() {
        Ok(started) => started,
        Err(error) => {
            eprintln!("service=agent-worker error={error}");
            return ExitCode::FAILURE;
        }
    };
    run(
        ServiceKind::AgentWorker,
        Ok(runtime_probe),
        background_worker,
    )
}

struct AgentWorkerProcess {
    runtime: tokio::runtime::Runtime,
    workflow: DurableWorkflowStore,
    outbox: JetStreamOutboxPublisher,
    deletion: DeletionWorker,
    plugins: PluginRuntime,
}

impl AgentWorkerProcess {
    fn from_environment() -> Result<Self, String> {
        let secret_manager = production_secret_manager()?;
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
            if let Some(error) = probe_health
                .lock()
                .map_err(|_| "outbox health lock poisoned".to_owned())?
                .readiness_error(Instant::now(), BACKGROUND_HEARTBEAT_STALE_AFTER)
            {
                return Err(error);
            }
            plugins.check_readiness()?;
            Ok(format!(
                "gateway/runtime/provider adapter, durable workflow and sandboxed plugins ready; eventing_workers_status=enabled; plugins={}",
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

fn background_cycle_error<T>(
    delivery: &Result<
        PublishBatchResult,
        trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxError,
    >,
    projection: &Result<T, trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxError>,
    deletion: &Result<
        Vec<trpg_security_governance::security_privacy::DeletionJob>,
        trpg_security_governance::security_privacy::PrivacyError,
    >,
) -> Option<String> {
    let delivery_error = match delivery {
        Ok(result) if result.requires_operator_attention() => {
            let alert = result.alert_code().unwrap_or("OUTBOX_DELIVERY_ALERT");
            Some(format!(
                "{alert}:dead_lettered={}:dead_letter_total={}:failed={}:claimed={}",
                result.dead_lettered, result.dead_letter_total, result.failed, result.claimed
            ))
        }
        Ok(_) => None,
        Err(error) => Some(format!("EVENTING_DELIVERY_CYCLE_FAILED:{error}")),
    };
    let projection_error = projection
        .as_ref()
        .err()
        .map(|error| format!("PROJECTION_REBUILD_FAILED:{error}"));
    let deletion_error = deletion
        .as_ref()
        .err()
        .map(|error| format!("PRIVACY_DELETION_CYCLE_FAILED:{}", error.code()));
    let errors = [delivery_error, projection_error, deletion_error]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if errors.is_empty() {
        None
    } else {
        Some(errors.join(";"))
    }
}

struct BackgroundWorker {
    shutdown_sender: Sender<()>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for BackgroundWorker {
    fn drop(&mut self) {
        let _ = self.shutdown_sender.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct PluginRuntime {
    _host: PluginHost,
    plugins: Vec<HostedPlugin>,
}

impl PluginRuntime {
    fn load(registry_path: &Path) -> Result<Self, String> {
        validate_regular_absolute_file(registry_path)?;
        let document: PluginRegistryDocument = serde_json::from_slice(
            &fs::read(registry_path).map_err(|_| "PLUGIN_REGISTRY_UNREADABLE".to_owned())?,
        )
        .map_err(|_| "PLUGIN_REGISTRY_INVALID".to_owned())?;
        if document.plugins.len() > 128 {
            return Err("PLUGIN_REGISTRY_LIMIT_EXCEEDED".to_owned());
        }
        let host = PluginHost::new(document.fuel_limit, document.memory_limit_bytes)
            .map_err(|_| "PLUGIN_HOST_CONFIGURATION_INVALID".to_owned())?;
        let mut plugins = Vec::with_capacity(document.plugins.len());
        for registration in document.plugins {
            let module_path = PathBuf::from(&registration.module_path);
            validate_regular_absolute_file(&module_path)?;
            let requested_capabilities = registration
                .requested_capabilities
                .iter()
                .map(|value| parse_capability(value))
                .collect::<Result<Vec<_>, _>>()?;
            let granted_capabilities = registration
                .granted_capabilities
                .iter()
                .map(|value| parse_capability(value))
                .collect::<Result<Vec<_>, _>>()?;
            let grants = ExtensionCapabilityGrantSet::with_grants(&granted_capabilities)
                .map_err(|_| "PLUGIN_CAPABILITY_GRANT_INVALID".to_owned())?;
            let module =
                fs::read(module_path).map_err(|_| "PLUGIN_MODULE_UNREADABLE".to_owned())?;
            plugins.push(
                host.register(
                    HostedPluginManifest {
                        plugin_id: registration.plugin_id,
                        module_sha256: registration.module_sha256,
                        requested_capabilities,
                    },
                    &module,
                    &grants,
                )
                .map_err(|_| "PLUGIN_REGISTRATION_REJECTED".to_owned())?,
            );
        }
        Ok(Self {
            _host: host,
            plugins,
        })
    }

    fn check_readiness(&self) -> Result<(), String> {
        if self
            .plugins
            .iter()
            .any(|plugin| plugin.manifest().plugin_id.trim().is_empty())
        {
            Err("plugin registry integrity failure".to_owned())
        } else {
            Ok(())
        }
    }

    fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginRegistryDocument {
    fuel_limit: u64,
    memory_limit_bytes: usize,
    plugins: Vec<PluginRegistration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginRegistration {
    plugin_id: String,
    module_path: String,
    module_sha256: String,
    requested_capabilities: Vec<String>,
    granted_capabilities: Vec<String>,
}

fn parse_capability(value: &str) -> Result<ExtensionCapability, String> {
    match value {
        "invoke_granted_tool" => Ok(ExtensionCapability::InvokeGrantedTool),
        "read_projection" => Ok(ExtensionCapability::ReadProjection),
        "emit_proposed_decision" => Ok(ExtensionCapability::EmitProposedDecision),
        _ => Err("PLUGIN_CAPABILITY_FORBIDDEN".to_owned()),
    }
}

fn validate_regular_absolute_file(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("ABSOLUTE_CONFIGURATION_PATH_REQUIRED".to_owned());
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "REQUIRED_CONFIGURATION_FILE_MISSING".to_owned())?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err("REGULAR_CONFIGURATION_FILE_REQUIRED".to_owned());
    }
    Ok(())
}

fn optional_path(name: &str) -> Result<Option<PathBuf>, String> {
    let Some(value) = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    validate_regular_absolute_file(&path)?;
    Ok(Some(path))
}

fn optional_file_bytes(name: &str) -> Result<Option<Vec<u8>>, String> {
    optional_path(name)?
        .map(|path| fs::read(path).map_err(|_| format!("{name}_UNREADABLE")))
        .transpose()
}

fn required_environment(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name}_REQUIRED"))
}

fn production_secret_manager() -> Result<SecretManager<MountedFileSecretResolver>, String> {
    let resolver = MountedFileSecretResolver::new(required_environment("TRPG_SECRET_MOUNT")?)
        .map_err(|_| "SECRET_MOUNT_INVALID".to_owned())?;
    SecretManager::new_durable(resolver, required_environment("TRPG_SECRET_CATALOG_PATH")?)
        .map_err(|_| "SECRET_CATALOG_INVALID".to_owned())
}

fn resolve_mounted_secret(
    manager: &SecretManager<MountedFileSecretResolver>,
    prefix: &str,
) -> Result<SecretValue, String> {
    let id = required_environment(&format!("{prefix}_SECRET_ID"))?;
    let version = required_environment(&format!("{prefix}_SECRET_VERSION"))?
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{prefix}_SECRET_VERSION_INVALID"))?;
    let reference = SecretReference::mounted(id, version)
        .map_err(|_| format!("{prefix}_SECRET_REFERENCE_INVALID"))?;
    manager
        .register(&reference)
        .map_err(|_| format!("{prefix}_SECRET_REGISTRATION_FAILED"))?;
    manager
        .resolve(&reference)
        .map_err(|_| format!("{prefix}_SECRET_RESOLUTION_FAILED"))
}

fn boolean_environment(name: &str, default: bool) -> Result<bool, String> {
    match std::env::var(name) {
        Ok(value) => parse_boolean_environment_value(name, &value),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{name}_MUST_BE_BOOLEAN")),
    }
}

fn require_eventing_workers_enabled(enabled: bool) -> Result<(), String> {
    if enabled {
        Ok(())
    } else {
        Err("TRPG_P04_EVENTING_WORKERS_DISABLED_FAIL_CLOSED".to_owned())
    }
}

fn parse_boolean_environment_value(name: &str, value: &str) -> Result<bool, String> {
    if value.eq_ignore_ascii_case("true") || value == "1" {
        Ok(true)
    } else if value.eq_ignore_ascii_case("false") || value == "0" {
        Ok(false)
    } else {
        Err(format!("{name}_MUST_BE_BOOLEAN"))
    }
}

fn run(
    kind: ServiceKind,
    runtime: Result<RoleRuntimeProbe, trpg_contracts::ServiceError>,
    _background_worker: BackgroundWorker,
) -> ExitCode {
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            return ExitCode::FAILURE;
        }
    };
    let spec = match ServiceSpec::from_environment(kind, env!("CARGO_PKG_VERSION")) {
        Ok(spec) => spec,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            return ExitCode::FAILURE;
        }
    };
    match run_service(spec, vec![runtime]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        background_cycle_error, parse_boolean_environment_value, require_eventing_workers_enabled,
        BackgroundWorkerHealth,
    };
    use std::time::{Duration, Instant};
    use trpg_data_eventing::event_bus_nats_impl::{JetStreamOutboxError, PublishBatchResult};

    #[test]
    fn eventing_worker_rollout_flag_is_explicit_and_fail_closed() {
        assert_eq!(
            parse_boolean_environment_value("TRPG_P04_EVENTING_WORKERS_ENABLED", "true"),
            Ok(true)
        );
        assert_eq!(
            parse_boolean_environment_value("TRPG_P04_EVENTING_WORKERS_ENABLED", "0"),
            Ok(false)
        );
        assert_eq!(require_eventing_workers_enabled(true), Ok(()));
        assert_eq!(
            require_eventing_workers_enabled(false),
            Err("TRPG_P04_EVENTING_WORKERS_DISABLED_FAIL_CLOSED".to_owned())
        );
        assert_eq!(
            parse_boolean_environment_value("TRPG_P04_EVENTING_WORKERS_ENABLED", "enabled"),
            Err("TRPG_P04_EVENTING_WORKERS_ENABLED_MUST_BE_BOOLEAN".to_owned())
        );
    }

    #[test]
    fn delivery_projection_and_deletion_health_fail_independently() {
        let healthy_delivery = Ok(PublishBatchResult::default());
        let healthy_deletion = Ok(Vec::new());
        assert_eq!(
            background_cycle_error(&healthy_delivery, &Ok(1), &healthy_deletion),
            None
        );

        let delivery_failed = Err(JetStreamOutboxError::NatsUnavailable);
        assert_eq!(
            background_cycle_error(&delivery_failed, &Ok(1), &healthy_deletion),
            Some("EVENTING_DELIVERY_CYCLE_FAILED:NATS unavailable".to_owned())
        );

        let projection_failed: Result<(), JetStreamOutboxError> =
            Err(JetStreamOutboxError::Database("projection_rebuild"));
        assert_eq!(
            background_cycle_error(&healthy_delivery, &projection_failed, &healthy_deletion),
            Some("PROJECTION_REBUILD_FAILED:outbox database failed: projection_rebuild".to_owned())
        );
        let deletion_failed =
            Err(trpg_security_governance::security_privacy::PrivacyError::Database);
        assert_eq!(
            background_cycle_error(&healthy_delivery, &Ok(1), &deletion_failed),
            Some("PRIVACY_DELETION_CYCLE_FAILED:PRIVACY_DATABASE_ERROR".to_owned())
        );
        let combined =
            background_cycle_error(&delivery_failed, &projection_failed, &deletion_failed).unwrap();
        assert!(combined.contains("EVENTING_DELIVERY_CYCLE_FAILED"));
        assert!(combined.contains("PROJECTION_REBUILD_FAILED"));
        assert!(combined.contains("PRIVACY_DELETION_CYCLE_FAILED"));
    }

    #[test]
    fn background_health_rejects_pending_stale_and_stopped_workers() {
        let started = Instant::now();
        let mut health = BackgroundWorkerHealth::default();
        assert_eq!(
            health.readiness_error(started, Duration::from_secs(30)),
            Some("AGENT_WORKER_DEPENDENCY_CHECK_PENDING".to_owned())
        );

        health.record_cycle(started, None);
        assert_eq!(
            health.readiness_error(started + Duration::from_secs(30), Duration::from_secs(30)),
            None
        );
        assert_eq!(
            health.readiness_error(started + Duration::from_secs(31), Duration::from_secs(30),),
            Some("AGENT_WORKER_BACKGROUND_HEARTBEAT_STALE".to_owned())
        );

        health.record_stopped("AGENT_WORKER_BACKGROUND_PANICKED");
        assert_eq!(
            health.readiness_error(started, Duration::from_secs(30)),
            Some("AGENT_WORKER_BACKGROUND_PANICKED".to_owned())
        );
    }

    #[test]
    fn background_health_preserves_current_cycle_failure() {
        let completed_at = Instant::now();
        let mut health = BackgroundWorkerHealth::default();
        health.record_cycle(
            completed_at,
            Some("EVENTING_DELIVERY_CYCLE_FAILED:NATS unavailable".to_owned()),
        );

        assert_eq!(
            health.readiness_error(completed_at, Duration::from_secs(30)),
            Some("EVENTING_DELIVERY_CYCLE_FAILED:NATS unavailable".to_owned())
        );
    }
}
