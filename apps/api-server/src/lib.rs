pub mod core_domain;
pub mod middleware;
pub mod player_action;

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

impl ApiApplication {
    pub fn new(identity: IdentityService) -> Self {
        let identity_verifier = identity.verifier();
        Self {
            authentication: AuthenticationMiddleware::new(Arc::new(Mutex::new(identity))),
            identity_verifier,
            membership_governance: None,
            canonical_custody: None,
        }
    }

    pub fn new_governed(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
    ) -> Self {
        let identity_verifier = identity.verifier();
        let audit = FormalCommitAudit::from_file_log(audit);
        Self {
            authentication: AuthenticationMiddleware::new(Arc::new(Mutex::new(identity))),
            identity_verifier,
            membership_governance: Some(Arc::new(Mutex::new(MembershipGovernance {
                policy,
                audit,
            }))),
            canonical_custody: None,
        }
    }

    pub fn new_production_governed(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical_runtime: tokio::runtime::Runtime,
        canonical_store: PostgresCanonicalStore,
        privacy_runtime: tokio::runtime::Runtime,
        deletion_repository: PostgresDeletionRepository,
    ) -> Self {
        Self::new_production_governed_internal(
            identity,
            policy,
            audit,
            canonical_runtime,
            canonical_store,
            privacy_runtime,
            deletion_repository,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_production_governed_with_player_actions(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical_runtime: tokio::runtime::Runtime,
        canonical_store: PostgresCanonicalStore,
        privacy_runtime: tokio::runtime::Runtime,
        deletion_repository: PostgresDeletionRepository,
        player_action_repository: CoreDomainRepository,
    ) -> Self {
        Self::new_production_governed_internal(
            identity,
            policy,
            audit,
            canonical_runtime,
            canonical_store,
            privacy_runtime,
            deletion_repository,
            Some(player_action_repository),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_production_governed_internal(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical_runtime: tokio::runtime::Runtime,
        canonical_store: PostgresCanonicalStore,
        privacy_runtime: tokio::runtime::Runtime,
        deletion_repository: PostgresDeletionRepository,
        player_action_repository: Option<CoreDomainRepository>,
    ) -> Self {
        let identity_verifier = identity.verifier();
        let audit = FormalCommitAudit::from_file_log(audit);
        let runtime = Arc::new(Mutex::new(canonical_runtime));
        let canonical: Arc<dyn CanonicalCommitPort> = Arc::new(PostgresCanonicalCommitPort::new(
            Arc::clone(&runtime),
            canonical_store.clone(),
        ));
        let authorizer =
            FormalCommitAuthorizer::new(identity_verifier.clone(), policy.clone(), audit.clone());
        Self {
            authentication: AuthenticationMiddleware::new(Arc::new(Mutex::new(identity))),
            identity_verifier,
            membership_governance: Some(Arc::new(Mutex::new(MembershipGovernance {
                policy,
                audit,
            }))),
            canonical_custody: Some(Arc::new(CanonicalCustody {
                runtime,
                privacy_runtime: Mutex::new(privacy_runtime),
                store: canonical_store,
                canonical: Arc::clone(&canonical),
                authorizer: authorizer.clone(),
                deletion_repository,
                runtime_events: trpg_runtime::EventStore::with_formal_custody(
                    authorizer.clone(),
                    Arc::clone(&canonical),
                ),
                agent_events: trpg_agent_runtime::AgentEventStore::with_formal_custody(
                    authorizer, canonical,
                ),
                player_action_port: player_action_repository.map(RepositoryPlayerActionPort::new),
            })),
        }
    }

    pub fn readiness(&self) -> Result<String, String> {
        self.authentication
            .identity()
            .lock()
            .map_err(|_| "identity state lock poisoned".to_owned())?
            .check_readiness()
            .map_err(|error| error.code().to_owned())?;
        let governance = self
            .membership_governance
            .as_ref()
            .ok_or_else(|| "POLICY_UNAVAILABLE".to_owned())?;
        governance
            .lock()
            .map_err(|_| "policy state lock poisoned".to_owned())?
            .policy
            .check_readiness()
            .map_err(|error| error.code().to_owned())?;
        if let Some(custody) = &self.canonical_custody {
            custody.check_readiness()?;
        }
        Ok(
            "persistent identity, authorization, canonical event and witness state ready"
                .to_owned(),
        )
    }

    pub fn handle(&self, request: &HttpRequest) -> Option<HttpResponse> {
        match (request.method.as_str(), request.path.as_str()) {
            ("POST", "/auth/login") => Some(self.login(request)),
            ("POST", "/auth/refresh") => Some(self.refresh(request)),
            ("POST", "/auth/logout") => Some(self.logout(request)),
            _ => self.handle_campaign_route(request),
        }
    }

    fn login(&self, request: &HttpRequest) -> HttpResponse {
        let body: LoginRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        match self.authentication.identity().lock() {
            Ok(mut identity) => match identity.login(&body.login, &body.password, now) {
                Ok(session) => HttpResponse::json(
                    200,
                    json!({
                        "access_token": session.token.expose(),
                        "token_type": "Bearer",
                        "expires_at_unix_ms": session.expires_at_unix_ms,
                    }),
                ),
                Err(error) => identity_error(error),
            },
            Err(_) => internal_error(),
        }
    }

    fn refresh(&self, request: &HttpRequest) -> HttpResponse {
        let token = match bearer_token(request) {
            Ok(token) => token,
            Err(response) => return response,
        };
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        match self.authentication.identity().lock() {
            Ok(mut identity) => match identity.refresh_session(token, now) {
                Ok(session) => HttpResponse::json(
                    200,
                    json!({
                        "access_token": session.token.expose(),
                        "token_type": "Bearer",
                        "expires_at_unix_ms": session.expires_at_unix_ms,
                    }),
                ),
                Err(error) => identity_error(error),
            },
            Err(_) => internal_error(),
        }
    }

    fn logout(&self, request: &HttpRequest) -> HttpResponse {
        let token = match bearer_token(request) {
            Ok(token) => token,
            Err(response) => return response,
        };
        match self.authentication.identity().lock() {
            Ok(mut identity) => match identity.logout(token) {
                Ok(()) => HttpResponse::json(204, json!({})),
                Err(error) => identity_error(error),
            },
            Err(_) => internal_error(),
        }
    }

    fn handle_campaign_route(&self, request: &HttpRequest) -> Option<HttpResponse> {
        let (path, query) = request
            .path
            .split_once('?')
            .map_or((request.path.as_str(), ""), |(path, query)| (path, query));
        let segments = path.trim_matches('/').split('/').collect::<Vec<_>>();
        match (request.method.as_str(), segments.as_slice()) {
            ("GET", ["campaigns", campaign_id, "authority"]) => {
                Some(self.get_authority(request, campaign_id))
            }
            ("PUT", ["campaigns", campaign_id, "memberships", user_id]) => {
                Some(self.put_membership(request, campaign_id, user_id))
            }
            ("GET", ["campaigns", campaign_id, "events"]) => {
                Some(self.get_canonical_events(request, campaign_id, query))
            }
            ("POST", ["campaigns", campaign_id, "privacy", "deletions"]) => {
                Some(self.request_deletion(request, campaign_id))
            }
            ("POST", ["campaigns", campaign_id, "player-actions"]) => {
                Some(self.submit_player_action(request, campaign_id))
            }
            ("POST", ["campaigns", campaign_id, "player-actions", action_id, "confirm"]) => {
                Some(self.confirm_player_action(request, campaign_id, action_id))
            }
            ("GET", ["campaigns", campaign_id, "privacy", "deletions", job_id]) => {
                Some(self.get_deletion_status(request, campaign_id, job_id))
            }
            _ => None,
        }
    }

    fn submit_player_action(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        let body: SubmitPlayerActionApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id {
            return HttpResponse::json(400, json!({"error": "PLAYER_ACTION_PATH_BODY_MISMATCH"}));
        }
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let context = match self.authorized_player_action_context(
            request,
            campaign_id,
            &body.action_id,
            &body.command,
            now,
        ) {
            Ok(context) => context,
            Err(response) => return response,
        };
        let Some(custody) = &self.canonical_custody else {
            return HttpResponse::json(503, json!({"error": "PLAYER_ACTION_WORKFLOW_UNAVAILABLE"}));
        };
        let Some(port) = custody.player_action_port.as_ref() else {
            return HttpResponse::json(503, json!({"error": "PLAYER_ACTION_WORKFLOW_UNAVAILABLE"}));
        };
        let api = PlayerActionApi::new(Arc::new(port.clone()));
        let result = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(api.submit(&context, &body)),
            Err(_) => return internal_error(),
        };
        match result {
            Ok(receipt) => HttpResponse::json(
                202,
                json!({
                    "first_event_sequence": receipt.first_event_sequence,
                    "last_event_sequence": receipt.last_event_sequence,
                    "aggregate_version": receipt.aggregate_version,
                    "state": receipt.state,
                    "realtime_delta_id": receipt.realtime_delta_id,
                }),
            ),
            Err(error) => player_action_api_error(error),
        }
    }

    fn confirm_player_action(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        action_id: &str,
    ) -> HttpResponse {
        let body: ConfirmPlayerActionApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id || body.action_id != action_id {
            return HttpResponse::json(400, json!({"error": "PLAYER_ACTION_PATH_BODY_MISMATCH"}));
        }
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let context = match self.authorized_player_action_context(
            request,
            campaign_id,
            action_id,
            &body.command,
            now,
        ) {
            Ok(context) => context,
            Err(response) => return response,
        };
        let Some(custody) = &self.canonical_custody else {
            return HttpResponse::json(503, json!({"error": "PLAYER_ACTION_WORKFLOW_UNAVAILABLE"}));
        };
        let Some(port) = custody.player_action_port.as_ref() else {
            return HttpResponse::json(503, json!({"error": "PLAYER_ACTION_WORKFLOW_UNAVAILABLE"}));
        };
        let api = PlayerActionApi::new(Arc::new(port.clone()));
        let result = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(api.confirm(&context, &body)),
            Err(_) => return internal_error(),
        };
        match result {
            Ok(receipt) => HttpResponse::json(
                200,
                json!({
                    "first_event_sequence": receipt.first_event_sequence,
                    "last_event_sequence": receipt.last_event_sequence,
                    "aggregate_version": receipt.aggregate_version,
                    "state": receipt.state,
                    "realtime_delta_id": receipt.realtime_delta_id,
                }),
            ),
            Err(error) => player_action_api_error(error),
        }
    }

    fn authorized_player_action_context(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        action_id: &str,
        command: &ApiCommandFields,
        now_unix_ms: u64,
    ) -> Result<AuthorizedCoreApiContext, HttpResponse> {
        let requesting_authentication = self
            .authentication
            .authenticate_bearer(request.header("authorization"), now_unix_ms)
            .map_err(auth_error)?;
        let campaign = EntityId::new(campaign_id)
            .map_err(|_| HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})))?;
        let resource = ResourceRef::new(campaign_id, "player_action", action_id)
            .map_err(|_| HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})))?;
        let (requesting_actor, workflow_authentication, workflow_actor, authority_contract) =
            match self.authentication.identity().lock() {
                Ok(mut identity) => {
                    let requesting_actor = identity
                        .command_actor(&requesting_authentication, &campaign, now_unix_ms)
                        .map_err(identity_error)?;
                    let expires_at = now_unix_ms.checked_add(60_000).ok_or_else(internal_error)?;
                    let credential = identity
                        .issue_workload_credential(
                            "api_player_action_workflow",
                            WorkloadRole::WorkflowEngine,
                            now_unix_ms,
                            expires_at,
                        )
                        .map_err(identity_error)?;
                    let workflow_authentication = identity
                        .authenticate_workload(&credential, now_unix_ms)
                        .map_err(identity_error)?;
                    let workflow_actor = identity
                        .command_actor(&workflow_authentication, &campaign, now_unix_ms)
                        .map_err(identity_error)?;
                    let authority = identity
                        .authority_contract(&campaign)
                        .map_err(identity_error)?
                        .ok_or_else(|| {
                            HttpResponse::json(
                                404,
                                json!({"error": "AUTHORITY_CONTRACT_NOT_FOUND"}),
                            )
                        })?;
                    (
                        requesting_actor,
                        workflow_authentication,
                        workflow_actor,
                        authority,
                    )
                }
                Err(_) => return Err(internal_error()),
            };
        let authority_binding = authority_contract.binding().map_err(|error| {
            kernel_error_response(
                request,
                &error,
                "authorize_player_action",
                action_id,
                error.code(),
            )
        })?;
        let requesting_context = AuthenticatedCommandContext::new(
            requesting_actor.clone(),
            resource.clone(),
            authority_binding.clone(),
            command.trace_id.clone(),
            requesting_authentication.authenticated_at_unix_ms(),
            requesting_authentication.expires_at_unix_ms(),
        )
        .map_err(|error| {
            kernel_error_response(
                request,
                &error,
                "authorize_player_action",
                action_id,
                error.code(),
            )
        })?;
        let workflow_context = AuthenticatedCommandContext::new(
            workflow_actor,
            resource,
            authority_binding,
            command.trace_id.clone(),
            workflow_authentication.authenticated_at_unix_ms(),
            workflow_authentication.expires_at_unix_ms(),
        )
        .map_err(|error| {
            kernel_error_response(
                request,
                &error,
                "authorize_player_action",
                action_id,
                error.code(),
            )
        })?;
        let expected_version = u64::try_from(command.expected_version).map_err(|_| {
            HttpResponse::json(
                400,
                json!({"error": "PLAYER_ACTION_EXPECTED_VERSION_INVALID"}),
            )
        })?;
        let provenance_kind =
            if requesting_actor.role() == &trpg_shared_kernel::ActorRole::HumanKeeper {
                ProvenanceKind::HumanKeeperStatement
            } else {
                ProvenanceKind::UserStatement
            };
        let policy_command = CommandEnvelope::new(
            (),
            CommandMetadata {
                command_id: EntityId::new(&command.command_id).map_err(|_| {
                    HttpResponse::json(400, json!({"error": "PLAYER_ACTION_COMMAND_ID_INVALID"}))
                })?,
                idempotency_key: command.idempotency_key.clone(),
                expected_version,
                authority_mode: authority_contract.mode().clone(),
                visibility: Visibility::new(VisibilityLabel::PartyVisible),
                fact_provenance: FactProvenance::new(
                    provenance_kind,
                    &command.command_id,
                    requesting_actor.id().as_str(),
                )
                .map_err(|error| {
                    kernel_error_response(
                        request,
                        &error,
                        "authorize_player_action",
                        action_id,
                        error.code(),
                    )
                })?,
                correlation_id: EntityId::new(&command.correlation_id).map_err(|_| {
                    HttpResponse::json(
                        400,
                        json!({"error": "PLAYER_ACTION_CORRELATION_ID_INVALID"}),
                    )
                })?,
                causation_id: EntityId::new(&command.causation_id).map_err(|_| {
                    HttpResponse::json(400, json!({"error": "PLAYER_ACTION_CAUSATION_ID_INVALID"}))
                })?,
                write_path: FormalWritePath::WorkflowDecision,
                authenticated_context: workflow_context.clone(),
            },
        );
        let custody = self.canonical_custody.as_ref().ok_or_else(|| {
            HttpResponse::json(503, json!({"error": "PLAYER_ACTION_WORKFLOW_UNAVAILABLE"}))
        })?;
        let authorization = custody
            .authorizer
            .authorize(
                &workflow_authentication,
                None,
                &policy_command,
                "workflow",
                now_unix_ms,
            )
            .map_err(|error| {
                kernel_error_response(
                    request,
                    &error,
                    "authorize_player_action",
                    action_id,
                    error.code(),
                )
            })?;
        AuthorizedCoreApiContext::from_authenticated_contexts(
            requesting_context,
            workflow_context,
            &authority_contract,
            authorization.canonical_audit().clone(),
        )
        .map_err(player_action_api_error)
    }

    fn request_deletion(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let authorizing_authentication = match self
            .authentication
            .authenticate_bearer(request.header("authorization"), now)
        {
            Ok(authentication) => authentication,
            Err(error) => return auth_error(error),
        };
        let body: DataDeletionRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.reason.len() > 1_024 {
            return HttpResponse::json(400, json!({"error": "DELETION_REASON_TOO_LONG"}));
        }
        let idempotency_key = match required_safe_header(request, "idempotency-key", 160) {
            Ok(value) => value,
            Err(response) => return response,
        };
        let campaign_id = match EntityId::new(campaign_id) {
            Ok(campaign_id) => campaign_id,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        let subject_id = match EntityId::new(&body.subject_id) {
            Ok(subject_id) => subject_id,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        let command_id = match EntityId::new(&body.command_id) {
            Ok(value) => value,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        let correlation_id = match EntityId::new(&body.correlation_id) {
            Ok(value) => value,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        let causation_id = match EntityId::new(&body.causation_id) {
            Ok(value) => value,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        if EntityId::new(&body.job_id).is_err() || body.retention_policy.len() > 128 {
            return HttpResponse::json(400, json!({"error": "INVALID_DELETION_REQUEST"}));
        }
        let Some(custody) = &self.canonical_custody else {
            return HttpResponse::json(503, json!({"error": "PRIVACY_WORKFLOW_UNAVAILABLE"}));
        };
        let (workflow_authentication, actor, authority) =
            match self.authentication.identity().lock() {
                Ok(mut identity) => {
                    let expires_at = match now.checked_add(60_000) {
                        Some(value) => value,
                        None => return internal_error(),
                    };
                    let credential = match identity.issue_workload_credential(
                        "api_privacy_workflow",
                        WorkloadRole::WorkflowEngine,
                        now,
                        expires_at,
                    ) {
                        Ok(credential) => credential,
                        Err(error) => return identity_error(error),
                    };
                    let workflow = match identity.authenticate_workload(&credential, now) {
                        Ok(authentication) => authentication,
                        Err(error) => return identity_error(error),
                    };
                    let actor = match identity.command_actor(&workflow, &campaign_id, now) {
                        Ok(actor) => actor,
                        Err(error) => return identity_error(error),
                    };
                    let authority = match identity.authority_contract(&campaign_id) {
                        Ok(Some(authority)) => authority,
                        Ok(None) => {
                            return HttpResponse::json(
                                404,
                                json!({"error": "AUTHORITY_CONTRACT_NOT_FOUND"}),
                            )
                        }
                        Err(error) => return identity_error(error),
                    };
                    (workflow, actor, authority)
                }
                Err(_) => return internal_error(),
            };
        let context = match AuthenticatedCommandContext::new(
            actor,
            match ResourceRef::new(campaign_id.as_str(), "data_subject", subject_id.as_str()) {
                Ok(resource) => resource,
                Err(error) => {
                    return kernel_error_response(
                        request,
                        &error,
                        "request_data_deletion",
                        subject_id.as_str(),
                        "invalid deletion resource binding",
                    )
                }
            },
            match authority.binding() {
                Ok(binding) => binding,
                Err(error) => {
                    return kernel_error_response(
                        request,
                        &error,
                        "request_data_deletion",
                        subject_id.as_str(),
                        "invalid authority binding",
                    )
                }
            },
            safe_request_context(request.header("x-trace-id"), "trace"),
            workflow_authentication.authenticated_at_unix_ms(),
            workflow_authentication.expires_at_unix_ms(),
        ) {
            Ok(context) => context,
            Err(error) => {
                return kernel_error_response(
                    request,
                    &error,
                    "request_data_deletion",
                    subject_id.as_str(),
                    "invalid authenticated command context",
                )
            }
        };
        let command = CommandEnvelope::new(
            RequestDataDeletion {
                job_id: body.job_id.clone(),
                subject_id: body.subject_id.clone(),
                retention_policy: body.retention_policy,
                reason: body.reason,
            },
            CommandMetadata {
                command_id: command_id.clone(),
                idempotency_key,
                expected_version: body.expected_version,
                authority_mode: authority.mode().clone(),
                visibility: Visibility::private_to_player(subject_id.clone()),
                fact_provenance: match FactProvenance::new(
                    ProvenanceKind::UserStatement,
                    command_id.as_str(),
                    authorizing_authentication.subject_id().as_str(),
                ) {
                    Ok(provenance) => provenance,
                    Err(error) => {
                        return kernel_error_response(
                            request,
                            &error,
                            "request_data_deletion",
                            subject_id.as_str(),
                            "invalid deletion provenance",
                        )
                    }
                },
                correlation_id,
                causation_id,
                write_path: FormalWritePath::WorkflowDecision,
                authenticated_context: context,
            },
        );
        let result = match custody.privacy_runtime.lock() {
            Ok(runtime) => runtime.block_on(request_data_deletion_canonical(
                &custody.deletion_repository,
                &custody.authorizer,
                custody.canonical.as_ref(),
                &workflow_authentication,
                Some(&authorizing_authentication),
                &command,
                now,
            )),
            Err(_) => return internal_error(),
        };
        match result {
            Ok(event) => HttpResponse::json(
                202,
                json!({
                    "job_id": body.job_id,
                    "status": "requested",
                    "evidence_status": "confirmed",
                    "canonical_event_sequence": event.sequence,
                }),
            ),
            Err(error) => kernel_error_response(
                request,
                &error,
                "request_data_deletion",
                subject_id.as_str(),
                error.code(),
            ),
        }
    }

    fn get_deletion_status(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        job_id: &str,
    ) -> HttpResponse {
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let authentication = match self
            .authentication
            .authenticate_bearer(request.header("authorization"), now)
        {
            Ok(authentication) => authentication,
            Err(error) => return auth_error(error),
        };
        let campaign_id = match EntityId::new(campaign_id) {
            Ok(value) => value,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        let globally_privileged = matches!(
            authentication.kind(),
            PrincipalKind::UserSession {
                global_role: GlobalRole::Moderator | GlobalRole::ServerOwner,
                ..
            }
        );
        let campaign_authorized = if globally_privileged {
            self.identity_verifier.verify(&authentication, now).is_ok()
        } else {
            self.identity_verifier
                .authorize_replay(&authentication, &campaign_id, now)
                .is_ok()
        };
        if !campaign_authorized {
            // This resource is subject-private. An authenticated caller who
            // is outside the campaign must receive the same opaque response
            // as a same-campaign non-owner or an unknown job identifier.
            return HttpResponse::json(404, json!({"error": "DELETION_JOB_NOT_FOUND"}));
        }
        let Some(custody) = &self.canonical_custody else {
            return HttpResponse::json(503, json!({"error": "PRIVACY_WORKFLOW_UNAVAILABLE"}));
        };
        let job = match custody.privacy_runtime.lock() {
            Ok(runtime) => runtime.block_on(
                custody
                    .deletion_repository
                    .load_for_campaign(job_id, &campaign_id),
            ),
            Err(_) => return internal_error(),
        };
        let job = match job {
            Ok(job) => job,
            Err(trpg_security_governance::security_privacy::PrivacyError::JobNotFound) => {
                return HttpResponse::json(404, json!({"error": "DELETION_JOB_NOT_FOUND"}))
            }
            Err(_) => return internal_error(),
        };
        let may_view = authentication.subject_id().as_str() == job.requested_by
            || matches!(
                authentication.kind(),
                PrincipalKind::UserSession {
                    global_role: GlobalRole::Moderator | GlobalRole::ServerOwner,
                    ..
                }
            );
        if !may_view {
            // Deliberately indistinguishable from an unknown or
            // different-campaign identifier: the status endpoint must not be
            // a deletion-job existence oracle.
            return HttpResponse::json(404, json!({"error": "DELETION_JOB_NOT_FOUND"}));
        }
        deletion_status_response(job)
    }

    fn get_canonical_events(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        query: &str,
    ) -> HttpResponse {
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let authentication = match self
            .authentication
            .authenticate_bearer(request.header("authorization"), now)
        {
            Ok(authentication) => authentication,
            Err(error) => return auth_error(error),
        };
        let campaign_id = match EntityId::new(campaign_id) {
            Ok(campaign_id) => campaign_id,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        let authorization =
            match self
                .identity_verifier
                .authorize_replay(&authentication, &campaign_id, now)
            {
                Ok(authorization) => authorization,
                Err(error) => return identity_error(error),
            };
        let (after_sequence, limit) = match replay_page_parameters(query) {
            Ok(parameters) => parameters,
            Err(response) => return response,
        };
        let Some(custody) = &self.canonical_custody else {
            return HttpResponse::json(503, json!({"error": "CANONICAL_STORE_UNAVAILABLE"}));
        };
        match custody.replay_visible(&authorization, now, after_sequence, limit) {
            Ok(page) => HttpResponse::json(
                200,
                json!({
                    "campaign_id": campaign_id.as_str(),
                    "events": page.events,
                    "scanned_through_sequence": page.scanned_through_sequence,
                    "limit": limit,
                }),
            ),
            Err(CanonicalReplayError::Identity(error)) => identity_error(error),
            Err(CanonicalReplayError::Store(CanonicalStoreError::IntegrityViolation(_)))
            | Err(CanonicalReplayError::StoredEventInvalid) => kernel_error_response(
                request,
                &TrpgError::AuditIntegrityViolation,
                "canonical_replay",
                campaign_id.as_str(),
                "canonical replay integrity validation failed",
            ),
            Err(CanonicalReplayError::Store(_)) => kernel_error_response(
                request,
                &TrpgError::PolicyUnavailable,
                "canonical_replay",
                campaign_id.as_str(),
                "canonical replay store unavailable",
            ),
        }
    }

    fn get_authority(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        if let Err(error) = self.authentication.authorize_campaign(
            request.header("authorization"),
            campaign_id,
            &[
                CampaignRole::CampaignOwner,
                CampaignRole::HumanKeeper,
                CampaignRole::Player,
                CampaignRole::Spectator,
            ],
            match now_unix_ms() {
                Ok(now) => now,
                Err(response) => return response,
            },
        ) {
            return auth_error(error);
        }
        let campaign_id = match EntityId::new(campaign_id) {
            Ok(campaign_id) => campaign_id,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        let contract = match self.authentication.identity().lock() {
            Ok(mut identity) => match identity.authority_contract(&campaign_id) {
                Ok(contract) => contract,
                Err(error) => return identity_error(error),
            },
            Err(_) => return internal_error(),
        };
        let Some(contract) = contract else {
            return HttpResponse::json(404, json!({"error": "AUTHORITY_CONTRACT_NOT_FOUND"}));
        };
        HttpResponse::json(
            200,
            json!({
                "contract_id": contract.contract_id().as_str(),
                "campaign_id": contract.campaign_id().as_str(),
                "mode": match contract.mode() {
                    AuthorityMode::HumanKp => "HUMAN_KP",
                    AuthorityMode::AiKp => "AI_KP",
                },
                "authority_owner": contract.authority_owner().as_str(),
                "version": contract.version(),
                "locked": contract.is_locked(),
                "change_policy": "FORK_ONLY",
            }),
        )
    }

    fn put_membership(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        user_id: &str,
    ) -> HttpResponse {
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let authentication = match self
            .authentication
            .authenticate_bearer(request.header("authorization"), now)
        {
            Ok(authentication) => authentication,
            Err(error) => return auth_error(error),
        };
        let body: MembershipRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        let role = match parse_campaign_role(&body.role) {
            Ok(role) => role,
            Err(response) => return response,
        };
        let campaign_entity_id = match EntityId::new(campaign_id) {
            Ok(campaign_id) => campaign_id,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        let (acting_membership, authority) = match self.authentication.identity().lock() {
            Ok(mut identity) => {
                let membership = match identity.require_membership_manager(
                    &authentication,
                    &campaign_entity_id,
                    now,
                ) {
                    Ok(membership) => membership,
                    Err(error) => return identity_error(error),
                };
                let authority = match identity.authority_contract(&campaign_entity_id) {
                    Ok(Some(authority)) => authority,
                    Ok(None) => {
                        return HttpResponse::json(
                            404,
                            json!({"error": "AUTHORITY_CONTRACT_NOT_FOUND"}),
                        )
                    }
                    Err(error) => return identity_error(error),
                };
                (membership, authority)
            }
            Err(_) => return internal_error(),
        };
        let governance = match &self.membership_governance {
            Some(governance) => governance,
            None => {
                return HttpResponse::json(503, json!({"error": "POLICY_UNAVAILABLE"}));
            }
        };
        let mut governance = match governance.lock() {
            Ok(governance) => governance,
            Err(_) => return internal_error(),
        };
        let trace_id = format!(
            "membership_{}_{}",
            authentication.subject_id().as_str(),
            now
        );
        let policy = governance.policy.clone();
        if let Err(error) = authorize_campaign_membership_change(
            &policy,
            &mut governance.audit,
            &self.identity_verifier,
            &authentication,
            acting_membership.as_ref(),
            authority.mode(),
            &campaign_entity_id,
            user_id,
            role,
            &trace_id,
            now,
        ) {
            return kernel_error_response(
                request,
                &error,
                "authorize_membership_change",
                campaign_id,
                error.code(),
            );
        }
        match self.authentication.identity().lock() {
            Ok(mut identity) => {
                match identity.grant_membership(&authentication, campaign_id, user_id, role, now) {
                    Ok(membership) => HttpResponse::json(
                        200,
                        json!({
                            "campaign_id": membership.campaign_id().as_str(),
                            "user_id": membership.user_id().as_str(),
                            "role": campaign_role_name(membership.role()),
                        }),
                    ),
                    Err(error) => identity_error(error),
                }
            }
            Err(_) => internal_error(),
        }
    }
}

#[derive(Debug)]
enum CanonicalReplayError {
    Identity(IdentityError),
    Store(CanonicalStoreError),
    StoredEventInvalid,
}

impl CanonicalCustody {
    fn check_readiness(&self) -> Result<(), String> {
        if !self.runtime_events.has_canonical_custody()
            || !self.agent_events.has_canonical_custody()
        {
            return Err("formal runtime/agent canonical custody missing".to_owned());
        }
        self.runtime
            .lock()
            .map_err(|_| "canonical runtime lock poisoned".to_owned())?
            .block_on(self.store.verify_integrity())
            .map_err(|error| error.to_string())?;
        self.privacy_runtime
            .lock()
            .map_err(|_| "privacy runtime lock poisoned".to_owned())?
            .block_on(self.deletion_repository.check_readiness())
            .map_err(|error| error.code().to_owned())
    }

    fn replay_visible(
        &self,
        authorization: &ReplayAuthorization,
        now_unix_ms: u64,
        after_sequence: i64,
        limit: i64,
    ) -> Result<VisibleReplayPage, CanonicalReplayError> {
        let records = self
            .runtime
            .lock()
            .map_err(|_| {
                CanonicalReplayError::Store(CanonicalStoreError::Connection {
                    component: "primary",
                })
            })?
            .block_on(self.store.load_replay_page(
                authorization.campaign_id().as_str(),
                after_sequence,
                limit,
            ))
            .map_err(CanonicalReplayError::Store)?;
        let scanned_through_sequence = records
            .last()
            .map_or(after_sequence, |event| event.sequence);
        let mut visible = Vec::with_capacity(records.len());
        for event in records {
            if event.campaign_id != authorization.campaign_id().as_str() {
                return Err(CanonicalReplayError::StoredEventInvalid);
            }
            let visibility = stored_visibility(&event)?;
            if authorization
                .can_view(authorization.campaign_id(), &visibility, now_unix_ms)
                .map_err(CanonicalReplayError::Identity)?
            {
                visible.push(canonical_event_json(event));
            }
        }
        Ok(VisibleReplayPage {
            events: visible,
            scanned_through_sequence,
        })
    }
}

fn stored_visibility(event: &CanonicalReplayEvent) -> Result<Visibility, CanonicalReplayError> {
    let subject =
        (event.visibility_subject != "not_applicable").then_some(event.visibility_subject.as_str());
    Visibility::try_from_parts(&event.visibility_label, subject)
        .map_err(|_| CanonicalReplayError::StoredEventInvalid)
}

fn canonical_event_json(event: CanonicalReplayEvent) -> serde_json::Value {
    json!({
        "sequence": event.sequence,
        "stream_version": event.stream_version,
        "stream_id": event.stream_id,
        "event_type": event.event_type,
        "campaign_id": event.campaign_id,
        "authenticated_actor_id": event.authenticated_actor_id,
        "resource_type": event.resource_type,
        "resource_id": event.resource_id,
        "authority_contract_id": event.authority_contract_id,
        "authority_owner": event.authority_owner,
        "command_id": event.command_id,
        "idempotency_key": event.idempotency_key,
        "authority_contract_version": event.authority_contract_version,
        "visibility_label": event.visibility_label,
        "visibility_subject": event.visibility_subject,
        "provenance_kind": event.provenance_kind,
        "provenance_reference": event.provenance_reference,
        "provenance_recorded_by": event.provenance_recorded_by,
        "correlation_id": event.correlation_id,
        "causation_id": event.causation_id,
        "trace_id": event.trace_id,
        "payload": event.payload,
        "event_integrity_hash": event.event_integrity_hash,
        "request_hash_source": event.request_hash_source,
        "integrity_status": event.integrity_status,
    })
}

fn replay_page_parameters(query: &str) -> Result<(i64, i64), HttpResponse> {
    let mut after_sequence = 0_i64;
    let mut limit = 100_i64;
    let mut saw_after_sequence = false;
    let mut saw_limit = false;
    if query.is_empty() {
        return Ok((after_sequence, limit));
    }
    for parameter in query.split('&') {
        let Some((name, value)) = parameter.split_once('=') else {
            return Err(HttpResponse::json(
                400,
                json!({"error": "INVALID_REPLAY_CURSOR"}),
            ));
        };
        match name {
            "after_sequence" if !saw_after_sequence => {
                after_sequence = value
                    .parse::<i64>()
                    .ok()
                    .filter(|value| *value >= 0)
                    .ok_or_else(|| {
                        HttpResponse::json(400, json!({"error": "INVALID_REPLAY_CURSOR"}))
                    })?;
                saw_after_sequence = true;
            }
            "limit" if !saw_limit => {
                limit = value
                    .parse::<i64>()
                    .ok()
                    .filter(|value| (1..=500).contains(value))
                    .ok_or_else(|| {
                        HttpResponse::json(400, json!({"error": "INVALID_REPLAY_LIMIT"}))
                    })?;
                saw_limit = true;
            }
            _ => {
                return Err(HttpResponse::json(
                    400,
                    json!({"error": "INVALID_REPLAY_CURSOR"}),
                ));
            }
        }
    }
    Ok((after_sequence, limit))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    login: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MembershipRequest {
    role: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DataDeletionRequest {
    job_id: String,
    subject_id: String,
    retention_policy: String,
    reason: String,
    command_id: String,
    correlation_id: String,
    causation_id: String,
    expected_version: u64,
}

fn deletion_status_response(job: DeletionJob) -> HttpResponse {
    HttpResponse::json(
        200,
        json!({
            "job_id": job.job_id,
            "status": deletion_job_status_name(job.status),
            "evidence_status": match job.evidence_status {
                trpg_security_governance::security_privacy::DeletionEvidenceStatus::Pending => {
                    "pending"
                }
                trpg_security_governance::security_privacy::DeletionEvidenceStatus::Confirmed => {
                    "confirmed"
                }
            },
            "canonical_event_sequence": job.canonical_event_sequence,
            "failure_code": job.failure_code,
            "targets": job.targets.into_iter().map(|target| json!({
                "target": target.target.as_str(),
                "status": deletion_target_status_name(target.status),
                "error_code": target.error_code,
            })).collect::<Vec<_>>(),
        }),
    )
}

fn deletion_job_status_name(status: DeletionJobStatus) -> &'static str {
    match status {
        DeletionJobStatus::Requested => "requested",
        DeletionJobStatus::BlockedLegalHold => "blocked_legal_hold",
        DeletionJobStatus::Running => "running",
        DeletionJobStatus::Verifying => "verifying",
        DeletionJobStatus::Completed => "completed",
        DeletionJobStatus::Failed => "failed",
    }
}

fn deletion_target_status_name(status: DeletionTargetStatus) -> &'static str {
    match status {
        DeletionTargetStatus::Pending => "pending",
        DeletionTargetStatus::Deleted => "deleted",
        DeletionTargetStatus::Verified => "verified",
        DeletionTargetStatus::Failed => "failed",
    }
}

fn parse_json<T: for<'de> Deserialize<'de>>(request: &HttpRequest) -> Result<T, HttpResponse> {
    if request.header("content-type") != Some("application/json") {
        return Err(HttpResponse::json(
            400,
            json!({"error": "JSON_CONTENT_TYPE_REQUIRED"}),
        ));
    }
    serde_json::from_slice(&request.body)
        .map_err(|_| HttpResponse::json(400, json!({"error": "INVALID_JSON_BODY"})))
}

fn bearer_token(request: &HttpRequest) -> Result<&str, HttpResponse> {
    request
        .header("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or_else(|| HttpResponse::json(401, json!({"error": "AUTHENTICATION_REQUIRED"})))
}

fn required_safe_header(
    request: &HttpRequest,
    name: &str,
    max_len: usize,
) -> Result<String, HttpResponse> {
    request
        .header(name)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= max_len
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
        .map(str::to_owned)
        .ok_or_else(|| HttpResponse::json(400, json!({"error": "INVALID_IDEMPOTENCY_KEY"})))
}

fn parse_campaign_role(value: &str) -> Result<CampaignRole, HttpResponse> {
    match value {
        "CAMPAIGN_OWNER" => Ok(CampaignRole::CampaignOwner),
        "HUMAN_KEEPER" => Ok(CampaignRole::HumanKeeper),
        "PLAYER" => Ok(CampaignRole::Player),
        "SPECTATOR" => Ok(CampaignRole::Spectator),
        _ => Err(HttpResponse::json(
            400,
            json!({"error": "INVALID_CAMPAIGN_ROLE"}),
        )),
    }
}

fn campaign_role_name(role: CampaignRole) -> &'static str {
    match role {
        CampaignRole::CampaignOwner => "CAMPAIGN_OWNER",
        CampaignRole::HumanKeeper => "HUMAN_KEEPER",
        CampaignRole::Player => "PLAYER",
        CampaignRole::Spectator => "SPECTATOR",
    }
}

fn identity_error(error: IdentityError) -> HttpResponse {
    auth_error(ApiAuthError::from(error))
}

fn auth_error(error: ApiAuthError) -> HttpResponse {
    HttpResponse::json(error.status, json!({"error": error.code}))
}

fn player_action_api_error(error: CoreApiError) -> HttpResponse {
    HttpResponse::json(error.status_code(), json!({"error": error.to_string()}))
}

fn internal_error() -> HttpResponse {
    kernel_error_response_without_request(
        &TrpgError::AuditIntegrityViolation,
        "api_internal",
        "api_application",
        "internal API state unavailable",
    )
}

struct ProductionTrustedErrorLogSink;

impl TrustedErrorLogSink for ProductionTrustedErrorLogSink {
    fn record(&mut self, entry: TrustedErrorLogEntry<'_>) {
        let record = json!({
            "level": "error",
            "classification": "trusted_internal",
            "operation": entry.operation,
            "resource": entry.resource,
            "correlation_id": entry.correlation_id,
            "trace_id": entry.trace_id,
            "root_cause": entry.root_cause,
        });
        eprintln!("{record}");
    }
}

fn kernel_error_response(
    request: &HttpRequest,
    error: &TrpgError,
    operation: &str,
    resource: &str,
    root_cause: &str,
) -> HttpResponse {
    let correlation_id = safe_request_context(request.header("x-correlation-id"), "correlation");
    let trace_id = safe_request_context(request.header("x-trace-id"), "trace");
    build_kernel_error_response(
        error,
        operation,
        resource,
        &correlation_id,
        &trace_id,
        root_cause,
    )
}

fn kernel_error_response_without_request(
    error: &TrpgError,
    operation: &str,
    resource: &str,
    root_cause: &str,
) -> HttpResponse {
    let correlation_id = safe_request_context(None, "correlation");
    let trace_id = safe_request_context(None, "trace");
    build_kernel_error_response(
        error,
        operation,
        resource,
        &correlation_id,
        &trace_id,
        root_cause,
    )
}

fn build_kernel_error_response(
    error: &TrpgError,
    operation: &str,
    resource: &str,
    correlation_id: &str,
    trace_id: &str,
    root_cause: &str,
) -> HttpResponse {
    let descriptor = describe_error(error);
    let context =
        InternalErrorContext::new(operation, resource, correlation_id, trace_id, root_cause)
            .expect("fixed production error context must be valid");
    context.record(&mut ProductionTrustedErrorLogSink);
    let response = context.public_response(&descriptor);
    HttpResponse::json(
        response.http_status,
        serde_json::to_value(response).expect("public error response must serialize"),
    )
}

fn safe_request_context(candidate: Option<&str>, prefix: &str) -> String {
    candidate
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos());
            format!("{prefix}_{}_{}", std::process::id(), now)
        })
}

fn now_unix_ms() -> Result<u64, HttpResponse> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| internal_error())?
        .as_millis();
    u64::try_from(millis).map_err(|_| internal_error())
}

#[cfg(test)]
mod production_custody_tests;
