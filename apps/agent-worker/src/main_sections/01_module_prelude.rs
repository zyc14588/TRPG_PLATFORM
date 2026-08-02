use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use trpg_agent_runtime::agent_job::{
    AgentJobError, AgentJobExecutionConfig, AgentJobOutcome, AgentJobRepository, AgentJobResult,
    AgentJobWorker, AgentSkillCheckRoll, AgentSkillCheckRulePort, CertifiedLocalModel,
    GovernedAgentDecisionPort, GovernedAgentJobToolPort, ProductionAgentIdentityConfiguration,
};
use trpg_agent_runtime::local_model_certification::{
    CertificationRequest, CertificationRunStatus, LocalModelCertificate,
    LocalModelCertificationAuthority, LocalModelCertificationRunner,
    LocalModelCertificationSuite,
};
use trpg_agent_runtime::model_provider::{
    Environment as ModelEnvironment, ExecutableModelProvider, ModelProviderRuntimeConfig,
    ExecutedModelRouteSnapshot, ProviderCancellation, ProviderCapabilities, ProviderConfig,
    ProviderType,
};
use trpg_agent_runtime::model_provider_local_cloud_impl::HttpModelProvider;
use trpg_contracts::{run_service, RoleRuntimeProbe, ServiceKind, ServiceSpec};
use trpg_data_eventing::event_bus_nats_impl::{JetStreamOutboxPublisher, PublishBatchResult};
use trpg_data_eventing::campaign_export_worker::{
    CampaignExportOutcome, CampaignExportWorker,
};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    CanonicalStoreError, PostgresCanonicalCommitPort, PostgresCanonicalStore,
};
use trpg_extension_sdk::plugin_host::{HostedPlugin, HostedPluginManifest, PluginHost};
use trpg_extension_sdk::{ExtensionCapability, ExtensionCapabilityGrantSet};
use trpg_ruleset_coc7::dice_roll_contract::{
    server_roll_skill_check, DiceAdjustment, SuccessLevel,
};
use trpg_runtime::durable_workflow::{
    AgentJobEvidenceDraft, AgentJobTransitionDraft, DurableAgentApproval,
    DurableAgentAuthoritySnapshot, DurableAgentContextSnapshot, DurableAgentJob,
    DurableWorkflowStore,
};
use trpg_security_governance::secret::{
    MountedFileSecretResolver, PostgresLedgerCheckpointStore, SecretManager, SecretReference,
    SecretValue,
};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
use trpg_security_governance::security_privacy::{
    BackupKeyDeletionSurface, CampaignExportDeletionSurface, DeletionTarget, DeletionWorker,
    NatsQueueDeletionSurface, PostgresDeletionRepository, PostgresLegalHoldResolver,
    PostgresRecordDeletionSurface, RedisCacheDeletionSurface, S3ObjectDeletionSurface,
};

include!("07_local_model_certification_process/01_process.rs");
include!("07_local_model_certification_process/02_artifact_io.rs");

const BACKGROUND_HEARTBEAT_STALE_AFTER: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct Coc7AgentSkillCheckRules;

impl AgentSkillCheckRulePort for Coc7AgentSkillCheckRules {
    fn roll_skill_check(&self, target: u8) -> AgentJobResult<AgentSkillCheckRoll> {
        let roll = server_roll_skill_check(target, DiceAdjustment::None)
            .map_err(|_| AgentJobError::terminal("AGENT_SKILL_CHECK_RULE_FAILURE"))?;
        let outcome = roll.outcome();
        let success_level = match outcome.success_level {
            SuccessLevel::Critical => "CRITICAL",
            SuccessLevel::Extreme => "EXTREME",
            SuccessLevel::Hard => "HARD",
            SuccessLevel::Regular => "REGULAR",
            SuccessLevel::Failure => "FAILURE",
            SuccessLevel::Fumble => "FUMBLE",
        };
        Ok(AgentSkillCheckRoll {
            execution_id: roll.roll_id().to_owned(),
            roll: outcome.roll,
            selected_tens_digit: outcome.selected_tens_digit,
            ones_digit: outcome.ones_digit,
            success_level: success_level.to_owned(),
        })
    }
}

fn main() -> ExitCode {
    match AgentWorkerStartupMode::from_environment() {
        Ok(AgentWorkerStartupMode::CertificationOnly) => {
            return match run_local_model_certification_from_environment() {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("service=agent-worker mode=certification-only error={error}");
                    ExitCode::FAILURE
                }
            };
        }
        Ok(AgentWorkerStartupMode::CertificationService) => {
            return match run_local_model_certification_service_from_environment() {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("service=agent-worker mode=certification-service error={error}");
                    ExitCode::FAILURE
                }
            };
        }
        Ok(AgentWorkerStartupMode::Ready) => {}
        Err(error) => {
            eprintln!("service=agent-worker error={error}");
            return ExitCode::FAILURE;
        }
    }
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
    campaign_exports: CampaignExportWorker,
    plugins: PluginRuntime,
    model_route: Option<ExecutedModelRouteSnapshot>,
    agent_jobs: Option<AgentJobWorker>,
}
