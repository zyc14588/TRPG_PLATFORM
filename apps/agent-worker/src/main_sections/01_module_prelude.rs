use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use trpg_agent_runtime::agent_job::{
    AgentJobExecutionConfig, AgentJobOutcome, AgentJobWorker, CertifiedLocalModel,
    GovernedAgentDecisionPort, ProductionAgentIdentityConfiguration,
    RejectingAgentJobToolPort,
};
use trpg_agent_runtime::local_model_certification::{
    LocalModelCertificate, LocalModelCertificationAuthority,
};
use trpg_agent_runtime::model_provider::{
    Environment as ModelEnvironment, ExecutableModelProvider, ModelProviderRuntimeConfig,
    ExecutedModelRouteSnapshot, ProviderCapabilities, ProviderConfig, ProviderType,
};
use trpg_agent_runtime::model_provider_local_cloud_impl::HttpModelProvider;
use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_data_eventing::event_bus_nats_impl::{JetStreamOutboxPublisher, PublishBatchResult};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    PostgresCanonicalCommitPort, PostgresCanonicalStore,
};
use trpg_extension_sdk::plugin_host::{HostedPlugin, HostedPluginManifest, PluginHost};
use trpg_extension_sdk::{ExtensionCapability, ExtensionCapabilityGrantSet};
use trpg_runtime::durable_workflow::DurableWorkflowStore;
use trpg_security_governance::secret::{
    MountedFileSecretResolver, PostgresLedgerCheckpointStore, SecretManager, SecretReference,
    SecretValue,
};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
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
    model_route: Option<ExecutedModelRouteSnapshot>,
    agent_jobs: Option<AgentJobWorker>,
}
