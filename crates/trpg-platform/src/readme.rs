use trpg_shared_kernel::{
    CommandEnvelope, EventEnvelope, EventStore, KernelResult, Visibility, VisibilityLabel,
};

pub const PLATFORM_README_RECORDED_EVENT: &str = "platform.infrastructure.readme.recorded";

pub const PLATFORM_INFRASTRUCTURE_INVARIANTS: &[&str] = &[
    "business_layer_must_not_call_llm_directly",
    "agent_must_not_write_database_directly",
    "authority_contract_is_immutable_after_creation",
    "visibility_and_fact_provenance_cross_all_platform_outputs",
    "formal_decisions_go_through_tool_rules_state_event_log",
];

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum PlatformEvent {
    BackgroundWorkerStarted {
        worker_id: String,
        worker_kind: String,
    },
    DeploymentConfigured {
        environment: String,
        provider: String,
    },
    LocalDevEnvironmentValidated {
        profile: String,
        service_count: usize,
    },
    ObjectStored {
        object_id: String,
        display_name: String,
    },
    MetricRecorded {
        metric_name: String,
        value: u64,
        detail: String,
    },
    PerformanceBudgetEvaluated {
        budget_name: String,
        actual_ms: u64,
        limit_ms: u64,
    },
    DeploymentHealthObserved {
        service: String,
        healthy: bool,
        detail: String,
    },
    ReliabilityPolicyEvaluated {
        operation: String,
        retry_after_ms: u64,
    },
    AuditTraceRecorded {
        action: String,
        detail: String,
    },
    ReadmeContractRecorded {
        invariant_count: usize,
    },
}

pub type PlatformEventEnvelope = EventEnvelope<PlatformEvent>;
pub type PlatformEventStore = EventStore<PlatformEvent>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordReadmeContract {
    pub reviewer: String,
}

pub fn append_platform_event<T>(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<T>,
    event_type: &'static str,
    payload: PlatformEvent,
) -> KernelResult<PlatformEventEnvelope> {
    store.append(command, event_type, payload)
}

pub fn restricted_visibility(label: &VisibilityLabel) -> bool {
    matches!(
        label,
        VisibilityLabel::KeeperOnly
            | VisibilityLabel::PrivateToPlayer
            | VisibilityLabel::InvestigatorPrivate
            | VisibilityLabel::AiInternal
            | VisibilityLabel::SystemOnly
            | VisibilityLabel::SystemPrivate
    )
}

pub fn redact_for_observability(visibility: &Visibility, text: &str) -> String {
    if restricted_visibility(visibility.label()) {
        "[redacted]".to_owned()
    } else {
        text.to_owned()
    }
}

pub fn record_readme_contract(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<RecordReadmeContract>,
) -> KernelResult<PlatformEventEnvelope> {
    append_platform_event(
        store,
        command,
        PLATFORM_README_RECORDED_EVENT,
        PlatformEvent::ReadmeContractRecorded {
            invariant_count: PLATFORM_INFRASTRUCTURE_INVARIANTS.len(),
        },
    )
}
