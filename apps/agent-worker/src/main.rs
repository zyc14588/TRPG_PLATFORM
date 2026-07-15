use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Deserialize;
use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;
use trpg_extension_sdk::plugin_host::{HostedPlugin, HostedPluginManifest, PluginHost};
use trpg_extension_sdk::{ExtensionCapability, ExtensionCapabilityGrantSet};
use trpg_runtime::durable_workflow::DurableWorkflowStore;

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
    plugins: PluginRuntime,
}

impl AgentWorkerProcess {
    fn from_environment() -> Result<Self, String> {
        let database_url = required_environment("TRPG_DATABASE_URL")?;
        let nats_url = required_environment("TRPG_NATS_URL")?;
        let worker_id = std::env::var("TRPG_AGENT_WORKER_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "agent-worker-primary".to_owned());
        let nats_ca = optional_path("TRPG_NATS_CA_CERT_PATH")?;
        let nats_credentials = optional_path("TRPG_NATS_CREDENTIALS_PATH")?;
        let plugin_registry_path = required_environment("TRPG_PLUGIN_REGISTRY_PATH")?;
        let plugins = PluginRuntime::load(Path::new(&plugin_registry_path))?;

        let runtime = tokio::runtime::Runtime::new()
            .map_err(|_| "AGENT_WORKER_RUNTIME_INITIALIZATION_FAILED".to_owned())?;
        let workflow = runtime
            .block_on(DurableWorkflowStore::connect(&database_url))
            .map_err(|_| "DURABLE_WORKFLOW_CONNECTION_FAILED".to_owned())?;
        runtime
            .block_on(workflow.apply_migration())
            .map_err(|_| "DURABLE_WORKFLOW_MIGRATION_FAILED".to_owned())?;
        runtime
            .block_on(workflow.check_readiness())
            .map_err(|error| format!("DURABLE_WORKFLOW_NOT_READY:{error}"))?;
        let outbox = runtime
            .block_on(JetStreamOutboxPublisher::connect_with_credentials(
                &database_url,
                &nats_url,
                &worker_id,
                nats_ca.as_deref(),
                nats_credentials.as_deref(),
            ))
            .map_err(|_| "JETSTREAM_OUTBOX_CONNECTION_FAILED".to_owned())?;
        runtime
            .block_on(outbox.ensure_stream())
            .map_err(|_| "JETSTREAM_OUTBOX_NOT_READY".to_owned())?;
        Ok(Self {
            runtime,
            workflow,
            outbox,
            plugins,
        })
    }

    fn start(self) -> Result<(RoleRuntimeProbe, BackgroundWorker), String> {
        let background_error = Arc::new(Mutex::new(Some(
            "AGENT_WORKER_DEPENDENCY_CHECK_PENDING".to_owned(),
        )));
        let (shutdown_sender, shutdown_receiver) = mpsc::channel();
        let background_outbox = self.outbox.clone();
        let background_workflow = self.workflow.clone();
        let background_runtime = self.runtime;
        let background_error_writer = Arc::clone(&background_error);
        let worker = thread::Builder::new()
            .name("agent-outbox-publisher".to_owned())
            .spawn(move || loop {
                let cycle = background_runtime.block_on(async {
                    background_workflow.check_readiness().await.map_err(|_| {
                        trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxError::Database(
                            "workflow_readiness",
                        )
                    })?;
                    background_outbox.stream_message_count().await?;
                    background_outbox.publish_batch().await
                });
                match cycle {
                    Ok(_) => {
                        if let Ok(mut error) = background_error_writer.lock() {
                            *error = None;
                        }
                    }
                    Err(error_detail) => {
                        if let Ok(mut error) = background_error_writer.lock() {
                            *error =
                                Some(format!("JETSTREAM_OUTBOX_PUBLISH_FAILED:{error_detail}"));
                        }
                    }
                }
                match shutdown_receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            })
            .map_err(|_| "OUTBOX_WORKER_START_FAILED".to_owned())?;

        let plugins = self.plugins;
        let probe_error = Arc::clone(&background_error);
        let probe = RoleRuntimeProbe::spawn("agent_worker_runtime", move || {
            let boundary = trpg_agent_runtime::provider_boundary_snapshot();
            if boundary.gateway != "Agent Gateway"
                || boundary.runtime != "Agent Orchestrator/Runtime"
                || boundary.provider_adapter != "Model Provider Adapter"
                || boundary.forbidden_direct_call_error != "DIRECT_LLM_CALL_FORBIDDEN"
            {
                return Err("provider boundary initialization is incomplete".to_owned());
            }
            if let Some(error) = probe_error
                .lock()
                .map_err(|_| "outbox health lock poisoned".to_owned())?
                .clone()
            {
                return Err(error);
            }
            plugins.check_readiness()?;
            Ok(format!(
                "gateway/runtime/provider adapter, durable workflow, JetStream outbox and sandboxed plugins ready; plugins={}",
                plugins.plugin_count()
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

fn required_environment(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name}_REQUIRED"))
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
