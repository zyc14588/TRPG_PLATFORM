
impl ApiApplication {

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
}
