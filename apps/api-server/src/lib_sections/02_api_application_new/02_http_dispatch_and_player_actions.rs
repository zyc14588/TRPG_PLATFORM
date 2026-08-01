
impl ApiApplication {
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
                Ok(session) => {
                    let authentication = match identity
                        .authenticate_session(Some(session.token.expose()), now)
                    {
                        Ok(authentication) => authentication,
                        Err(error) => return identity_error(error),
                    };
                    let global_role = match authentication.kind() {
                        PrincipalKind::UserSession {
                            global_role: GlobalRole::ServerOwner,
                            ..
                        } => "SERVER_OWNER",
                        PrincipalKind::UserSession {
                            global_role: GlobalRole::Moderator,
                            ..
                        } => "MODERATOR",
                        PrincipalKind::UserSession {
                            global_role: GlobalRole::User,
                            ..
                        } => "USER",
                        _ => return internal_error(),
                    };
                    HttpResponse::json(
                        200,
                        json!({
                            "access_token": session.token.expose(),
                            "token_type": "Bearer",
                            "expires_at_unix_ms": session.expires_at_unix_ms,
                            "user_id": authentication.subject_id().as_str(),
                            "global_role": global_role,
                        }),
                    )
                }
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
            ("GET", ["campaigns", campaign_id, "membership"]) => {
                Some(self.get_current_membership(request, campaign_id))
            }
            ("PUT", ["campaigns", campaign_id, "memberships", user_id]) => {
                Some(self.put_membership(request, campaign_id, user_id))
            }
            ("POST", ["campaigns", campaign_id, "groups", group_id]) => {
                Some(self.create_group(request, campaign_id, group_id))
            }
            (
                "PUT",
                ["campaigns", campaign_id, "groups", group_id, "memberships", user_id],
            ) => Some(self.put_group_membership(request, campaign_id, group_id, user_id)),
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
