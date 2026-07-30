
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
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_production_governed_with_v1_lifecycle(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical_runtime: tokio::runtime::Runtime,
        canonical_store: PostgresCanonicalStore,
        privacy_runtime: tokio::runtime::Runtime,
        deletion_repository: PostgresDeletionRepository,
        core_domain_database_url: &str,
    ) -> Result<Self, String> {
        let repository = canonical_runtime
            .block_on(CoreDomainRepository::connect(
                core_domain_database_url,
                canonical_store.clone(),
            ))
            .map_err(|_| "CORE_DOMAIN_DATABASE_CONNECTION_FAILED".to_owned())?;
        Ok(Self::new_production_governed_internal(
            identity,
            policy,
            audit,
            canonical_runtime,
            canonical_store,
            privacy_runtime,
            deletion_repository,
            Some(repository),
            None,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_production_governed_with_agent_jobs(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        canonical_runtime: tokio::runtime::Runtime,
        canonical_store: PostgresCanonicalStore,
        privacy_runtime: tokio::runtime::Runtime,
        deletion_repository: PostgresDeletionRepository,
        player_action_repository: Option<CoreDomainRepository>,
        workflow: DurableWorkflowStore,
        route: AgentJobRouteConfiguration,
    ) -> Result<Self, String> {
        route.validate()?;
        Ok(Self::new_production_governed_internal(
            identity,
            policy,
            audit,
            canonical_runtime,
            canonical_store,
            privacy_runtime,
            deletion_repository,
            player_action_repository,
            Some(AgentJobGateway { workflow, route }),
        ))
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
        agent_jobs: Option<AgentJobGateway>,
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
                lifecycle_port: player_action_repository
                    .clone()
                    .map(RepositoryCampaignCharacterPort::new),
                player_action_port: player_action_repository.map(RepositoryPlayerActionPort::new),
                agent_jobs,
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
            ("GET", "/api/v1/openapi.json") => {
                Some(HttpResponse::json(
                    200,
                    trpg_api::api_contracts::v1_openapi_document(),
                ))
            }
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
        if segments.starts_with(&["api", "v1"]) {
            return self.handle_v1_route(request, &segments[2..]);
        }
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
            ("POST", ["campaigns", campaign_id, "agent-jobs"]) => {
                Some(self.request_agent_job(request, campaign_id))
            }
            ("POST", ["campaigns", campaign_id, "agent-jobs", job_id, "approve"]) => {
                Some(self.approve_agent_job(request, campaign_id, job_id))
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
}

include!("07_v1_lifecycle.rs");
