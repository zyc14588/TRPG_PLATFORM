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
