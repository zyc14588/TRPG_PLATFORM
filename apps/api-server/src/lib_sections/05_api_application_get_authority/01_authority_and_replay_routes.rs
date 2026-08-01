
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
                "created_at_unix_ms": contract.created_at_unix_ms(),
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
