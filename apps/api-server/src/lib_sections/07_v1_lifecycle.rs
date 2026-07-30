impl ApiApplication {
    #[allow(clippy::too_many_arguments)]
    fn authorized_core_context(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        resource_type: &str,
        resource_id: &str,
        command: &ApiCommandFields,
        visibility: Visibility,
        allow_invite_acceptance: bool,
        error_namespace: &str,
        operation: &str,
        now_unix_ms: u64,
    ) -> Result<AuthorizedCoreApiContext, HttpResponse> {
        let requesting_authentication = self
            .authentication
            .authenticate_bearer(request.header("authorization"), now_unix_ms)
            .map_err(auth_error)?;
        let campaign = EntityId::new(campaign_id)
            .map_err(|_| core_request_error(error_namespace, "ENTITY_ID_INVALID"))?;
        let resource = ResourceRef::new(campaign_id, resource_type, resource_id)
            .map_err(|_| core_request_error(error_namespace, "ENTITY_ID_INVALID"))?;
        let (requesting_actor, workflow_authentication, workflow_actor, authority_contract) =
            match self.authentication.identity().lock() {
                Ok(mut identity) => {
                    let requesting_actor = if allow_invite_acceptance {
                        match requesting_authentication.kind() {
                            PrincipalKind::UserSession { session_id, .. } => {
                                Actor::authenticated_user(
                                    requesting_authentication.subject_id().as_str(),
                                    ActorRole::Investigator,
                                    session_id.as_str(),
                                )
                                .map_err(|_| IdentityError::InvalidIdentityData)
                            }
                            _ => Err(IdentityError::InvalidIdentityData),
                        }
                    } else {
                        identity.command_actor(
                            &requesting_authentication,
                            &campaign,
                            now_unix_ms,
                        )
                    }
                    .map_err(identity_error)?;
                    let expires_at = now_unix_ms.checked_add(60_000).ok_or_else(internal_error)?;
                    let credential = identity
                        .issue_workload_credential(
                            "api_core_workflow",
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
                operation,
                resource_id,
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
                operation,
                resource_id,
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
                operation,
                resource_id,
                error.code(),
            )
        })?;
        let expected_version = u64::try_from(command.expected_version)
            .map_err(|_| core_request_error(error_namespace, "EXPECTED_VERSION_INVALID"))?;
        let provenance_kind =
            if requesting_actor.role() == &trpg_shared_kernel::ActorRole::HumanKeeper {
                ProvenanceKind::HumanKeeperStatement
            } else {
                ProvenanceKind::UserStatement
            };
        let policy_command = CommandEnvelope::new(
            (),
            CommandMetadata {
                command_id: EntityId::new(&command.command_id)
                    .map_err(|_| core_request_error(error_namespace, "COMMAND_ID_INVALID"))?,
                idempotency_key: command.idempotency_key.clone(),
                expected_version,
                authority_mode: authority_contract.mode().clone(),
                visibility,
                fact_provenance: FactProvenance::new(
                    provenance_kind,
                    &command.command_id,
                    requesting_actor.id().as_str(),
                )
                .map_err(|error| {
                    kernel_error_response(
                        request,
                        &error,
                        operation,
                        resource_id,
                        error.code(),
                    )
                })?,
                correlation_id: EntityId::new(&command.correlation_id)
                    .map_err(|_| core_request_error(error_namespace, "CORRELATION_ID_INVALID"))?,
                causation_id: EntityId::new(&command.causation_id)
                    .map_err(|_| core_request_error(error_namespace, "CAUSATION_ID_INVALID"))?,
                write_path: FormalWritePath::WorkflowDecision,
                authenticated_context: workflow_context.clone(),
            },
        );
        let custody = self
            .canonical_custody
            .as_ref()
            .ok_or_else(|| core_request_error(error_namespace, "WORKFLOW_UNAVAILABLE"))?;
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
                    operation,
                    resource_id,
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

    fn handle_v1_route(
        &self,
        request: &HttpRequest,
        segments: &[&str],
    ) -> Option<HttpResponse> {
        match (request.method.as_str(), segments) {
            ("GET", ["campaigns"]) => Some(self.v1_list_campaigns(request)),
            ("POST", ["campaigns"]) => Some(self.v1_create_campaign(request)),
            ("GET", ["campaigns", campaign_id]) => {
                Some(self.v1_get_campaign(request, campaign_id))
            }
            ("POST", ["campaigns", campaign_id, "invites"]) => {
                Some(self.v1_issue_invite(request, campaign_id))
            }
            ("POST", ["campaigns", campaign_id, "invites", invite_id, "accept"]) => {
                Some(self.v1_accept_invite(request, campaign_id, invite_id))
            }
            ("POST", ["campaigns", campaign_id, "characters"]) => {
                Some(self.v1_create_character(request, campaign_id))
            }
            ("PUT", ["campaigns", campaign_id, "characters", character_id]) => {
                Some(self.v1_update_character(request, campaign_id, character_id))
            }
            (
                "POST",
                ["campaigns", campaign_id, "characters", character_id, "submit"],
            ) => Some(self.v1_submit_character(request, campaign_id, character_id)),
            (
                "POST",
                ["campaigns", campaign_id, "characters", character_id, "review"],
            ) => Some(self.v1_review_character(request, campaign_id, character_id)),
            ("POST", ["campaigns", campaign_id, "scenarios", "import"]) => {
                Some(self.v1_import_scenario(request, campaign_id))
            }
            ("POST", ["campaigns", campaign_id, "sessions"]) => {
                Some(self.v1_start_session(request, campaign_id))
            }
            ("PATCH", ["campaigns", campaign_id, "sessions", session_id]) => {
                Some(self.v1_change_session_state(request, campaign_id, session_id))
            }
            (
                "POST",
                ["campaigns", campaign_id, "sessions", session_id, "scenes"],
            ) => Some(self.v1_switch_scene(request, campaign_id, session_id)),
            (
                "POST",
                [
                    "campaigns",
                    campaign_id,
                    "sessions",
                    session_id,
                    "characters",
                    character_id,
                    "join",
                ],
            ) => Some(self.v1_join_character(
                request,
                campaign_id,
                session_id,
                character_id,
            )),
            ("POST", ["campaigns", campaign_id, "player-actions"]) => {
                Some(self.submit_player_action(request, campaign_id))
            }
            (
                "POST",
                [
                    "campaigns",
                    campaign_id,
                    "player-actions",
                    action_id,
                    "confirm",
                ],
            ) => Some(self.confirm_player_action(request, campaign_id, action_id)),
            ("POST", ["campaigns", campaign_id, "reconsiderations"]) => {
                Some(self.v1_request_reconsideration(request, campaign_id))
            }
            (
                "POST",
                [
                    "campaigns",
                    campaign_id,
                    "reconsiderations",
                    reconsideration_id,
                    "review",
                ],
            ) => Some(self.v1_review_reconsideration(
                request,
                campaign_id,
                reconsideration_id,
            )),
            (
                "POST",
                [
                    "campaigns",
                    campaign_id,
                    "reconsiderations",
                    reconsideration_id,
                    "resolve",
                ],
            ) => Some(self.v1_resolve_reconsideration(
                request,
                campaign_id,
                reconsideration_id,
            )),
            ("POST", ["campaigns", campaign_id, "forks"]) => {
                Some(self.v1_fork_campaign(request, campaign_id))
            }
            ("POST", ["campaigns", campaign_id, "exports"]) => {
                Some(self.v1_request_export(request, campaign_id))
            }
            ("GET", ["campaigns", campaign_id, "exports", export_id]) => {
                Some(self.v1_get_export(request, campaign_id, export_id))
            }
            _ => None,
        }
    }

    fn v1_binding(
        &self,
    ) -> Result<
        (
            Arc<CanonicalCustody>,
            V1LifecycleApi<RepositoryCampaignCharacterPort>,
        ),
        HttpResponse,
    > {
        let custody = Arc::clone(self.canonical_custody.as_ref().ok_or_else(|| {
            HttpResponse::json(503, json!({"error": "CORE_API_WORKFLOW_UNAVAILABLE"}))
        })?);
        let port = custody.lifecycle_port.clone().ok_or_else(|| {
            HttpResponse::json(503, json!({"error": "CORE_API_WORKFLOW_UNAVAILABLE"}))
        })?;
        Ok((custody, V1LifecycleApi::new(Arc::new(port))))
    }

    fn v1_query_actor(&self, request: &HttpRequest) -> Result<(String, bool), HttpResponse> {
        let now = now_unix_ms()?;
        let authentication = self
            .authentication
            .authenticate_bearer(request.header("authorization"), now)
            .map_err(auth_error)?;
        let include_all = matches!(
            authentication.kind(),
            PrincipalKind::UserSession {
                global_role: GlobalRole::ServerOwner,
                ..
            }
        );
        Ok((authentication.subject_id().as_str().to_owned(), include_all))
    }

    fn v1_list_campaigns(&self, request: &HttpRequest) -> HttpResponse {
        let (actor_id, include_all) = match self.v1_query_actor(request) {
            Ok(actor) => actor,
            Err(response) => return response,
        };
        let (custody, api) = match self.v1_binding() {
            Ok(binding) => binding,
            Err(response) => return response,
        };
        let result = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(api.list_campaigns(&actor_id, include_all)),
            Err(_) => return internal_error(),
        };
        match result {
            Ok(campaigns) => HttpResponse::json(200, json!({"campaigns": campaigns})),
            Err(error) => player_action_api_error(error),
        }
    }

    fn v1_get_campaign(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        let (actor_id, include_all) = match self.v1_query_actor(request) {
            Ok(actor) => actor,
            Err(response) => return response,
        };
        let (custody, api) = match self.v1_binding() {
            Ok(binding) => binding,
            Err(response) => return response,
        };
        let result = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(api.get_campaign(
                &actor_id,
                include_all,
                campaign_id,
            )),
            Err(_) => return internal_error(),
        };
        match result {
            Ok(campaign) => HttpResponse::json(200, json!(campaign)),
            Err(error) => player_action_api_error(error),
        }
    }

    fn v1_create_campaign(&self, request: &HttpRequest) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: CreateCampaignApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        self.v1_run_command(
            request,
            &body.campaign_id,
            "campaign",
            &body.campaign_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            201,
            &body,
            |api, context, body| Box::pin(api.create_campaign(context, body)),
        )
    }

    fn v1_issue_invite(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: IssueInviteApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id {
            return v1_path_body_mismatch();
        }
        let invited_user_id = match EntityId::new(&body.invited_user_id) {
            Ok(id) => id,
            Err(_) => return core_request_error("CORE_API", "ENTITY_ID_INVALID"),
        };
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let context = match self.authorized_core_context(
            request,
            campaign_id,
            "campaign_invite",
            &body.invite_id,
            &body.command,
            Visibility::private_to_player(invited_user_id),
            false,
            "CORE_API",
            "authorize_campaign_invite",
            now,
        ) {
            Ok(context) => context,
            Err(response) => return response,
        };
        let (custody, api) = match self.v1_binding() {
            Ok(binding) => binding,
            Err(response) => return response,
        };
        let result = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(api.issue_invite(&context, &body)),
            Err(_) => return internal_error(),
        };
        match result {
            Ok(invite) => HttpResponse::json(
                201,
                json!({
                    "invite_id": invite.invite_id,
                    "raw_token": invite.raw_token,
                    "expires_at_unix_ms": invite.expires_at_unix_ms,
                    "last_event_sequence": invite.receipt.last_event_sequence,
                    "aggregate_version": invite.receipt.aggregate_version,
                }),
            ),
            Err(error) => player_action_api_error(error),
        }
    }

    fn v1_accept_invite(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        invite_id: &str,
    ) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: AcceptInviteApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id || body.invite_id != invite_id {
            return v1_path_body_mismatch();
        }
        let accepting_user_id = match EntityId::new(&body.accepting_user_id) {
            Ok(id) => id,
            Err(_) => return core_request_error("CORE_API", "ENTITY_ID_INVALID"),
        };
        self.v1_run_command(
            request,
            campaign_id,
            "campaign_invite",
            invite_id,
            &body.command,
            Visibility::private_to_player(accepting_user_id),
            true,
            200,
            &body,
            |api, context, body| Box::pin(api.accept_invite(context, body)),
        )
    }

    fn v1_create_character(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: CreateCharacterApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "character",
            &body.character_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            201,
            &body,
            |api, context, body| Box::pin(api.create_character(context, body)),
        )
    }

    fn v1_update_character(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        character_id: &str,
    ) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: UpdateCharacterApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id || body.character_id != character_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "character",
            character_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            200,
            &body,
            |api, context, body| Box::pin(api.update_character(context, body)),
        )
    }

    fn v1_submit_character(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        character_id: &str,
    ) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: CharacterTransitionApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id || body.character_id != character_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "character",
            character_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            202,
            &body,
            |api, context, body| Box::pin(api.submit_character(context, body)),
        )
    }

    fn v1_review_character(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        character_id: &str,
    ) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: CharacterTransitionApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id || body.character_id != character_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "character",
            character_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            200,
            &body,
            |api, context, body| Box::pin(api.review_character(context, body)),
        )
    }

    fn v1_import_scenario(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: ImportScenarioApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "scenario",
            &body.scenario_id,
            &body.command,
            Visibility::new(VisibilityLabel::KeeperOnly),
            false,
            201,
            &body,
            |api, context, body| Box::pin(api.import_scenario(context, body)),
        )
    }

    fn v1_start_session(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: StartSessionApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "session",
            &body.session_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            201,
            &body,
            |api, context, body| Box::pin(api.start_session(context, body)),
        )
    }

    fn v1_change_session_state(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        session_id: &str,
    ) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: ChangeSessionStateApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id || body.session_id != session_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "session",
            session_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            200,
            &body,
            |api, context, body| Box::pin(api.change_session_state(context, body)),
        )
    }

    fn v1_switch_scene(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        session_id: &str,
    ) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: SwitchSceneApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id || body.session_id != session_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "session",
            session_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            201,
            &body,
            |api, context, body| Box::pin(api.switch_scene(context, body)),
        )
    }

    fn v1_join_character(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        session_id: &str,
        character_id: &str,
    ) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: JoinCharacterSessionApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id
            || body.session_id != session_id
            || body.character_id != character_id
        {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "session_character",
            &body.join_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            201,
            &body,
            |api, context, body| Box::pin(api.join_character_session(context, body)),
        )
    }

    fn v1_request_reconsideration(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
    ) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: RequestReconsiderationApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "reconsideration",
            &body.reconsideration_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            202,
            &body,
            |api, context, body| Box::pin(api.request_reconsideration(context, body)),
        )
    }

    fn v1_review_reconsideration(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        reconsideration_id: &str,
    ) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: ReviewReconsiderationApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id || body.reconsideration_id != reconsideration_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "reconsideration",
            reconsideration_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            200,
            &body,
            |api, context, body| Box::pin(api.review_reconsideration(context, body)),
        )
    }

    fn v1_resolve_reconsideration(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        reconsideration_id: &str,
    ) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: ResolveReconsiderationApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id || body.reconsideration_id != reconsideration_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "reconsideration",
            reconsideration_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            200,
            &body,
            |api, context, body| Box::pin(api.resolve_reconsideration(context, body)),
        )
    }

    fn v1_fork_campaign(&self, request: &HttpRequest, parent_campaign_id: &str) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: ForkCampaignApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.parent_campaign_id != parent_campaign_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            &body.child_campaign_id,
            "campaign_fork",
            &body.fork_id,
            &body.command,
            Visibility::new(VisibilityLabel::KeeperOnly),
            false,
            201,
            &body,
            |api, context, body| Box::pin(api.fork_campaign(context, body)),
        )
    }

    fn v1_request_export(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: RequestCampaignExportApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id {
            return v1_path_body_mismatch();
        }
        self.v1_run_command(
            request,
            campaign_id,
            "campaign_export",
            &body.export_id,
            &body.command,
            Visibility::new(VisibilityLabel::KeeperOnly),
            false,
            202,
            &body,
            |api, context, body| Box::pin(api.request_campaign_export(context, body)),
        )
    }

    fn v1_get_export(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        export_id: &str,
    ) -> HttpResponse {
        let (actor_id, include_all) = match self.v1_query_actor(request) {
            Ok(actor) => actor,
            Err(response) => return response,
        };
        let (custody, api) = match self.v1_binding() {
            Ok(binding) => binding,
            Err(response) => return response,
        };
        let result = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(api.get_campaign_export(
                &actor_id,
                include_all,
                campaign_id,
                export_id,
            )),
            Err(_) => return internal_error(),
        };
        match result {
            Ok(export) => HttpResponse::json(200, json!(export)),
            Err(error) => player_action_api_error(error),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn v1_run_command<R, F>(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        resource_type: &str,
        resource_id: &str,
        command: &ApiCommandFields,
        visibility: Visibility,
        allow_invite_acceptance: bool,
        success_status: u16,
        body: &R,
        run: F,
    ) -> HttpResponse
    where
        F: for<'a> FnOnce(
            &'a V1LifecycleApi<RepositoryCampaignCharacterPort>,
            &'a AuthorizedCoreApiContext,
            &'a R,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<trpg_api::api_contracts::CoreApiCommitReceipt, CoreApiError>,
                    > + 'a,
            >,
        >,
    {
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let context = match self.authorized_core_context(
            request,
            campaign_id,
            resource_type,
            resource_id,
            command,
            visibility,
            allow_invite_acceptance,
            "CORE_API",
            "authorize_v1_lifecycle_command",
            now,
        ) {
            Ok(context) => context,
            Err(response) => return response,
        };
        let (custody, api) = match self.v1_binding() {
            Ok(binding) => binding,
            Err(response) => return response,
        };
        let result = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(run(&api, &context, body)),
            Err(_) => return internal_error(),
        };
        match result {
            Ok(receipt) => HttpResponse::json(
                success_status,
                json!({
                    "last_event_sequence": receipt.last_event_sequence,
                    "aggregate_version": receipt.aggregate_version,
                }),
            ),
            Err(error) => player_action_api_error(error),
        }
    }
}

fn v1_path_body_mismatch() -> HttpResponse {
    HttpResponse::json(400, json!({"error": "CORE_API_PATH_BODY_MISMATCH"}))
}

fn core_request_error(namespace: &str, suffix: &str) -> HttpResponse {
    let status = if suffix == "WORKFLOW_UNAVAILABLE" {
        503
    } else {
        400
    };
    HttpResponse::json(status, json!({"error": format!("{namespace}_{suffix}")}))
}
