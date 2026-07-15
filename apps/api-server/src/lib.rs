pub mod middleware;

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::json;
use trpg_contracts::{HttpRequest, HttpResponse};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    CanonicalReplayEvent, CanonicalStoreError, PostgresCanonicalCommitPort, PostgresCanonicalStore,
};
use trpg_identity::{CampaignRole, IdentityError, IdentityService, ReplayAuthorization};
use trpg_security_governance::authorize_campaign_membership_change;
use trpg_security_governance::formal_commit_audit::{FormalCommitAudit, FormalCommitAuthorizer};
use trpg_security_governance::policy_adapter::OpenFgaOpaPolicyAdapter;
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
use trpg_shared_kernel::{
    AuthorityMode, CanonicalCommitPort, EntityId, Visibility, VisibilityLabel,
};

use middleware::{ApiAuthError, AuthenticationMiddleware};

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
    store: PostgresCanonicalStore,
    runtime_events: trpg_runtime::EventStore<trpg_runtime::RuntimeEventPayload>,
    agent_events: trpg_agent_runtime::AgentEventStore<trpg_agent_runtime::AgentEventPayload>,
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
                store: canonical_store,
                runtime_events: trpg_runtime::EventStore::with_formal_custody(
                    authorizer.clone(),
                    Arc::clone(&canonical),
                ),
                agent_events: trpg_agent_runtime::AgentEventStore::with_formal_custody(
                    authorizer, canonical,
                ),
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
            _ => None,
        }
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
            | Err(CanonicalReplayError::StoredEventInvalid) => {
                HttpResponse::json(500, json!({"error": "CANONICAL_EVENT_INTEGRITY_VIOLATION"}))
            }
            Err(CanonicalReplayError::Store(_)) => {
                HttpResponse::json(503, json!({"error": "CANONICAL_STORE_UNAVAILABLE"}))
            }
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
            return HttpResponse::json(error.http_status(), json!({"error": error.code()}));
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
            .map_err(|error| error.to_string())
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
    let label = VisibilityLabel::try_from(event.visibility_label.as_str())
        .map_err(|_| CanonicalReplayError::StoredEventInvalid)?;
    match label {
        VisibilityLabel::PrivateToPlayer => EntityId::new(&event.visibility_subject)
            .map(Visibility::private_to_player)
            .map_err(|_| CanonicalReplayError::StoredEventInvalid),
        VisibilityLabel::InvestigatorPrivate => EntityId::new(&event.visibility_subject)
            .map(Visibility::investigator_private)
            .map_err(|_| CanonicalReplayError::StoredEventInvalid),
        label if event.visibility_subject == "not_applicable" => Ok(Visibility::new(label)),
        _ => Err(CanonicalReplayError::StoredEventInvalid),
    }
}

fn canonical_event_json(event: CanonicalReplayEvent) -> serde_json::Value {
    json!({
        "sequence": event.sequence,
        "stream_version": event.stream_version,
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

fn internal_error() -> HttpResponse {
    HttpResponse::json(500, json!({"error": "IDENTITY_DATA_INVALID"}))
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
