impl ApiApplication {
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
        let visibility = if body.audience == "PLAYER" {
            match EntityId::new(&body.requested_by) {
                Ok(player_id) => Visibility::private_to_player(player_id),
                Err(_) => return core_request_error("CORE_API", "ENTITY_ID_INVALID"),
            }
        } else {
            Visibility::new(VisibilityLabel::KeeperOnly)
        };
        self.v1_run_command(
            request,
            campaign_id,
            "campaign_export",
            &body.export_id,
            &body.command,
            visibility,
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

    fn v1_issue_export_download(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
        export_id: &str,
    ) -> HttpResponse {
        let (actor_id, include_all) = match self.v1_query_actor(request) {
            Ok(actor) => actor,
            Err(response) => return response,
        };
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let (custody, api) = match self.v1_binding() {
            Ok(binding) => binding,
            Err(response) => return response,
        };
        let result = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(api.issue_campaign_export_download(
                &actor_id,
                include_all,
                campaign_id,
                export_id,
                now,
            )),
            Err(_) => return internal_error(),
        };
        match result {
            Ok(authorization) => HttpResponse::json(201, json!(authorization)),
            Err(error) => player_action_api_error(error),
        }
    }

}
