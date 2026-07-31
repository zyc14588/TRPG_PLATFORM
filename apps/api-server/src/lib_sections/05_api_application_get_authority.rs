
impl ApiApplication {

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
        let snapshot = contract.snapshot();
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
                "snapshot": {
                    "ruleset_version": snapshot.ruleset_version().as_str(),
                    "house_rules_version": snapshot.house_rules_version().as_str(),
                    "scenario_version": snapshot.scenario_version().as_str(),
                    "prompt_version": snapshot.prompt_version().as_str(),
                    "agent_pack_version": snapshot.agent_pack_version().as_str(),
                    "tool_schema_version": snapshot.tool_schema_version().as_str(),
                    "safety_profile_version": snapshot.safety_profile_version().as_str(),
                    "ai_provider_snapshot": snapshot.ai_provider_snapshot().as_str(),
                    "model_route_snapshot": snapshot.model_route_snapshot().as_str(),
                    "character_sheet_template_version": snapshot
                        .character_sheet_template_version()
                        .as_str(),
                },
            }),
        )
    }

    fn get_current_membership(&self, request: &HttpRequest, campaign_id: &str) -> HttpResponse {
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
        let campaign = match EntityId::new(campaign_id) {
            Ok(campaign) => campaign,
            Err(_) => return HttpResponse::json(400, json!({"error": "INVALID_ENTITY_ID"})),
        };
        match self.authentication.identity().lock() {
            Ok(mut identity) => match identity.membership_for(&authentication, &campaign, now) {
                Ok(Some(membership)) => HttpResponse::json(
                    200,
                    json!({
                        "campaign_id": campaign_id,
                        "user_id": membership.user_id().as_str(),
                        "role": campaign_role_name(membership.role()),
                    }),
                ),
                Ok(None) => HttpResponse::json(404, json!({"error": "MEMBERSHIP_REQUIRED"})),
                Err(error) => identity_error(error),
            },
            Err(_) => internal_error(),
        }
    }

    fn create_group(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        group_id: &str,
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
        match self.authentication.identity().lock() {
            Ok(mut identity) => match identity.create_campaign_group(
                &authentication,
                campaign_id,
                group_id,
                now,
            ) {
                Ok(group) => HttpResponse::json(
                    201,
                    json!({
                        "campaign_id": group.campaign_id().as_str(),
                        "group_id": group.group_id().as_str(),
                    }),
                ),
                Err(error) => identity_error(error),
            },
            Err(_) => internal_error(),
        }
    }

    fn put_group_membership(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        group_id: &str,
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
        match self.authentication.identity().lock() {
            Ok(mut identity) => match identity.grant_group_membership(
                &authentication,
                campaign_id,
                group_id,
                user_id,
                now,
            ) {
                Ok(membership) => HttpResponse::json(
                    200,
                    json!({
                        "campaign_id": membership.campaign_id().as_str(),
                        "group_id": membership.group_id().as_str(),
                        "user_id": membership.user_id().as_str(),
                    }),
                ),
                Err(error) => identity_error(error),
            },
            Err(_) => internal_error(),
        }
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
        if let Some(agent_jobs) = &self.agent_jobs {
            self.runtime
                .lock()
                .map_err(|_| "agent job runtime lock poisoned".to_owned())?
                .block_on(agent_jobs.workflow.check_agent_job_readiness())
                .map_err(|error| format!("AGENT_JOB_SCHEMA_NOT_READY:{error}"))?;
        }
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
