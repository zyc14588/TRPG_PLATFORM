
impl HttpPlayerActionApplication {
    fn new(
        identity: IdentityService,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FileAuditLog,
        port: RepositoryPlayerActionPort,
    ) -> Self {
        let authorizer = FormalCommitAuthorizer::new(
            identity.verifier(),
            policy,
            FormalCommitAudit::from_file_log(audit),
        );
        Self {
            identity: Arc::new(Mutex::new(identity)),
            authorizer,
            runtime: Arc::new(Mutex::new(tokio::runtime::Runtime::new().unwrap())),
            port,
        }
    }

    fn handle(&self, request: &HttpRequest) -> Option<HttpResponse> {
        let segments = request
            .path
            .trim_matches('/')
            .split('/')
            .collect::<Vec<_>>();
        match (request.method.as_str(), segments.as_slice()) {
            ("POST", ["campaigns", campaign_id, "player-actions"]) => {
                Some(self.submit(request, campaign_id))
            }
            ("POST", ["campaigns", campaign_id, "player-actions", action_id, "confirm"]) => {
                Some(self.confirm(request, campaign_id, action_id))
            }
            _ => None,
        }
    }

    fn submit(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        let body: SubmitPlayerActionApiRequest = match serde_json::from_slice(&request.body) {
            Ok(body) => body,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_JSON_BODY"})),
        };
        if body.campaign_id != campaign_id {
            return HttpResponse::json(400, json!({"error": "PLAYER_ACTION_PATH_BODY_MISMATCH"}));
        }
        let context =
            match self.authorized_context(request, campaign_id, &body.action_id, &body.command) {
                Ok(context) => context,
                Err(response) => return response,
            };
        let api = PlayerActionApi::new(Arc::new(self.port.clone()));
        let result = self
            .runtime
            .lock()
            .expect("P07 HTTP runtime lock")
            .block_on(api.submit(&context, &body));
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
            Err(error) => core_api_error(error),
        }
    }

    fn confirm(&self, request: &HttpRequest, campaign_id: &str, action_id: &str) -> HttpResponse {
        let body: ConfirmPlayerActionApiRequest = match serde_json::from_slice(&request.body) {
            Ok(body) => body,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_JSON_BODY"})),
        };
        if body.campaign_id != campaign_id || body.action_id != action_id {
            return HttpResponse::json(400, json!({"error": "PLAYER_ACTION_PATH_BODY_MISMATCH"}));
        }
        let context = match self.authorized_context(request, campaign_id, action_id, &body.command)
        {
            Ok(context) => context,
            Err(response) => return response,
        };
        let api = PlayerActionApi::new(Arc::new(self.port.clone()));
        let result = self
            .runtime
            .lock()
            .expect("P07 HTTP runtime lock")
            .block_on(api.confirm(&context, &body));
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
            Err(error) => core_api_error(error),
        }
    }

    fn authorized_context(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        action_id: &str,
        command: &ApiCommandFields,
    ) -> Result<AuthorizedCoreApiContext, HttpResponse> {
        let now = now_unix_ms();
        let token = request
            .header("authorization")
            .and_then(|value| value.strip_prefix("Bearer "))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| HttpResponse::json(401, json!({"error": "AUTHENTICATION_REQUIRED"})))?;
        let campaign = EntityId::new(campaign_id)
            .map_err(|_| HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})))?;
        let resource = ResourceRef::new(campaign_id, "player_action", action_id)
            .map_err(|_| HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})))?;
        let (
            requesting_authentication,
            requesting_actor,
            workflow_authentication,
            workflow_actor,
            authority,
        ) = {
            let mut identity = self
                .identity
                .lock()
                .map_err(|_| HttpResponse::json(500, json!({"error": "IDENTITY_LOCK_FAILED"})))?;
            let requesting_authentication = identity
                .authenticate_session(Some(token), now)
                .map_err(|_| {
                    HttpResponse::json(401, json!({"error": "AUTHENTICATION_REQUIRED"}))
                })?;
            let requesting_actor = identity
                .command_actor(&requesting_authentication, &campaign, now)
                .map_err(|_| HttpResponse::json(403, json!({"error": "CAMPAIGN_FORBIDDEN"})))?;
            let expires_at = now
                .checked_add(60_000)
                .ok_or_else(|| HttpResponse::json(500, json!({"error": "CLOCK_OVERFLOW"})))?;
            let credential = identity
                .issue_workload_credential(
                    "api_player_action_workflow",
                    WorkloadRole::WorkflowEngine,
                    now,
                    expires_at,
                )
                .map_err(|_| {
                    HttpResponse::json(503, json!({"error": "WORKFLOW_IDENTITY_UNAVAILABLE"}))
                })?;
            let workflow_authentication = identity
                .authenticate_workload(&credential, now)
                .map_err(|_| {
                    HttpResponse::json(503, json!({"error": "WORKFLOW_IDENTITY_UNAVAILABLE"}))
                })?;
            let workflow_actor = identity
                .command_actor(&workflow_authentication, &campaign, now)
                .map_err(|_| {
                    HttpResponse::json(503, json!({"error": "WORKFLOW_IDENTITY_UNAVAILABLE"}))
                })?;
            let authority = identity
                .authority_contract(&campaign)
                .map_err(|_| HttpResponse::json(503, json!({"error": "AUTHORITY_UNAVAILABLE"})))?
                .ok_or_else(|| {
                    HttpResponse::json(404, json!({"error": "AUTHORITY_CONTRACT_NOT_FOUND"}))
                })?;
            (
                requesting_authentication,
                requesting_actor,
                workflow_authentication,
                workflow_actor,
                authority,
            )
        };
        let authority_binding = authority
            .binding()
            .map_err(|_| HttpResponse::json(403, json!({"error": "AUTHORITY_CONTRACT_INVALID"})))?;
        let requesting_context = AuthenticatedCommandContext::new(
            requesting_actor.clone(),
            resource.clone(),
            authority_binding.clone(),
            command.trace_id.clone(),
            requesting_authentication.authenticated_at_unix_ms(),
            requesting_authentication.expires_at_unix_ms(),
        )
        .map_err(|_| HttpResponse::json(403, json!({"error": "REQUEST_CONTEXT_INVALID"})))?;
        let workflow_context = AuthenticatedCommandContext::new(
            workflow_actor,
            resource,
            authority_binding,
            command.trace_id.clone(),
            workflow_authentication.authenticated_at_unix_ms(),
            workflow_authentication.expires_at_unix_ms(),
        )
        .map_err(|_| HttpResponse::json(403, json!({"error": "WORKFLOW_CONTEXT_INVALID"})))?;
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
                authority_mode: authority.mode().clone(),
                visibility: Visibility::new(VisibilityLabel::PartyVisible),
                fact_provenance: FactProvenance::new(
                    provenance_kind,
                    &command.command_id,
                    requesting_actor.id().as_str(),
                )
                .map_err(|_| {
                    HttpResponse::json(400, json!({"error": "PLAYER_ACTION_PROVENANCE_INVALID"}))
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
        let authorization = self
            .authorizer
            .authorize(
                &workflow_authentication,
                None,
                &policy_command,
                "workflow",
                now,
            )
            .map_err(|_| HttpResponse::json(403, json!({"error": "POLICY_DENIED"})))?;
        AuthorizedCoreApiContext::from_authenticated_contexts(
            requesting_context,
            workflow_context,
            &authority,
            authorization.canonical_audit().clone(),
        )
        .map_err(core_api_error)
    }
}

fn core_api_error(error: CoreApiError) -> HttpResponse {
    HttpResponse::json(error.status_code(), json!({"error": error.to_string()}))
}

fn exchange(application: HttpPlayerActionApplication, request: String) -> (u16, Value) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let request = read_request(&mut stream);
        let response = application.handle(&request).unwrap_or_else(|| {
            trpg_contracts::HttpResponse::json(404, json!({"error": "NOT_FOUND"}))
        });
        let body = response.body.to_string();
        write!(
            stream,
            "HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.status,
            body.len(),
            body
        )
        .unwrap();
    });
    let mut stream = TcpStream::connect(address).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    server.join().unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    let status = headers
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    (status, serde_json::from_str(body).unwrap())
}

fn read_request(stream: &mut TcpStream) -> HttpRequest {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let (boundary, content_length) = loop {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0, "client closed before sending a complete request");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(boundary) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..boundary]);
            let content_length = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                .unwrap_or(0);
            if bytes.len() >= boundary + 4 + content_length {
                break (boundary, content_length);
            }
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..boundary]);
    let mut lines = headers.lines();
    let request_line = lines.next().unwrap().split_whitespace().collect::<Vec<_>>();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect::<HashMap<_, _>>();
    HttpRequest {
        method: request_line[0].to_owned(),
        path: request_line[1].to_owned(),
        headers,
        body: bytes[boundary + 4..boundary + 4 + content_length].to_vec(),
    }
}

fn json_request(method: &str, path: &str, token: Option<&str>, body: Value) -> String {
    let body = body.to_string();
    let authorization = token.map_or_else(String::new, |token| {
        format!("Authorization: Bearer {token}\r\n")
    });
    format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{authorization}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
