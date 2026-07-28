
impl ApiApplication {

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
}
