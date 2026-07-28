
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::json;
use trpg_api::api_contracts::{
    ApiCommandFields, AuthorizedCoreApiContext, ConfirmPlayerActionApiRequest, CoreApiError,
    PlayerActionApi, SubmitPlayerActionApiRequest,
};
use trpg_contracts::{HttpRequest, HttpResponse};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    CanonicalReplayEvent, CanonicalStoreError, PostgresCanonicalCommitPort, PostgresCanonicalStore,
};
use trpg_data_eventing::persistence_postgresql::CoreDomainRepository;
use trpg_identity::{
    CampaignRole, GlobalRole, IdentityError, IdentityService, PrincipalKind, ReplayAuthorization,
    WorkloadRole,
};
use trpg_platform::security_privacy_copyright::{
    request_data_deletion_canonical, RequestDataDeletion,
};
use trpg_security_governance::authorize_campaign_membership_change;
use trpg_security_governance::formal_commit_audit::{FormalCommitAudit, FormalCommitAuthorizer};
use trpg_security_governance::policy_adapter::OpenFgaOpaPolicyAdapter;
use trpg_security_governance::security_privacy::{
    DeletionJob, DeletionJobStatus, DeletionTargetStatus, PostgresDeletionRepository,
};
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
use trpg_shared_kernel::error_model::{
    describe_error, InternalErrorContext, TrustedErrorLogEntry, TrustedErrorLogSink,
};
use trpg_shared_kernel::{
    AuthenticatedCommandContext, AuthorityMode, CanonicalCommitPort, CommandEnvelope,
    CommandMetadata, EntityId, FactProvenance, FormalWritePath, ProvenanceKind, ResourceRef,
    TrpgError, Visibility, VisibilityLabel,
};

use middleware::{ApiAuthError, AuthenticationMiddleware};
use player_action::RepositoryPlayerActionPort;

#[derive(Clone)]
pub struct ApiApplication {
    authentication: AuthenticationMiddleware,
    identity_verifier: trpg_identity::IdentityVerifier,
    membership_governance: Option<Arc<Mutex<MembershipGovernance>>>,
    canonical_custody: Option<Arc<CanonicalCustody>>,
}

struct MembershipGovernance {
    policy: OpenFgaOpaPolicyAdapter,
    audit: FormalCommitAudit,
}

/// The production composition root moves the canonical store into this
/// private owner. HTTP handlers can request an authenticated replay page, but
/// neither a caller nor an agent receives the PostgreSQL write capability.
struct CanonicalCustody {
    runtime: Arc<Mutex<tokio::runtime::Runtime>>,
    privacy_runtime: Mutex<tokio::runtime::Runtime>,
    store: PostgresCanonicalStore,
    canonical: Arc<dyn CanonicalCommitPort>,
    authorizer: FormalCommitAuthorizer,
    deletion_repository: PostgresDeletionRepository,
    runtime_events: trpg_runtime::EventStore<trpg_runtime::RuntimeEventPayload>,
    agent_events: trpg_agent_runtime::AgentEventStore<trpg_agent_runtime::AgentEventPayload>,
    player_action_port: Option<RepositoryPlayerActionPort>,
}

struct VisibleReplayPage {
    events: Vec<serde_json::Value>,
    scanned_through_sequence: i64,
}
