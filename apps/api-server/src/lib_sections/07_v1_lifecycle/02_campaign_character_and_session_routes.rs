impl ApiApplication {
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

}
