impl ApiApplication {
    fn v1_submit_public_gameplay_action(
        &self,
        request: &HttpRequest,
        campaign_id: &str,
    ) -> HttpResponse {
        if let Err(response) = bearer_token(request) {
            return response;
        }
        let body: SubmitPublicGameplayActionApiRequest = match parse_json(request) {
            Ok(body) => body,
            Err(response) => return response,
        };
        if body.campaign_id != campaign_id
            || body.command.expected_version != 0
            || EntityId::new(&body.action_id).is_err()
            || EntityId::new(&body.session_id).is_err()
        {
            return public_gameplay_error(400, "PUBLIC_GAMEPLAY_REQUEST_INVALID");
        }
        let now = match now_unix_ms() {
            Ok(now) => now,
            Err(response) => return response,
        };
        let context = match self.authorized_core_context(
            request,
            campaign_id,
            "gameplay_action",
            &body.action_id,
            &body.command,
            Visibility::new(VisibilityLabel::PartyVisible),
            false,
            "PUBLIC_GAMEPLAY",
            "submit_public_gameplay_action",
            now,
        ) {
            Ok(context) => context,
            Err(response) => return response,
        };
        if context.authority_mode() != "human_kp"
            || context.actor_role() != "human_keeper"
            || context.actor_id() != context.authority_owner()
        {
            return public_gameplay_error(403, "PUBLIC_GAMEPLAY_AUTHORITY_FORBIDDEN");
        }
        let Some(custody) = &self.canonical_custody else {
            return public_gameplay_error(503, "PUBLIC_GAMEPLAY_WORKFLOW_UNAVAILABLE");
        };
        let Some(repository) = custody.gameplay_repository.as_ref() else {
            return public_gameplay_error(503, "PUBLIC_GAMEPLAY_WORKFLOW_UNAVAILABLE");
        };
        let (character_id, npc_id, kind) = public_gameplay_context_key(&body.action);
        let loaded = match custody.runtime.lock() {
            Ok(runtime) => runtime.block_on(repository.load_public_gameplay_profile_context(
                campaign_id,
                &body.session_id,
                character_id,
                npc_id,
                kind,
            )),
            Err(_) => return internal_error(),
        };
        let loaded = match loaded {
            Ok(loaded) => loaded,
            Err(error) => return public_gameplay_repository_error(error),
        };
        let rules_context = PublicGameplayContext {
            npc_public_identity: loaded.npc_public_identity,
            character_combat_profile: loaded.character_combat_profile,
            npc_combat_profile: loaded.npc_combat_profile,
            character_chase_profile: loaded.character_chase_profile,
            npc_chase_profile: loaded.npc_chase_profile,
        };
        let action = public_gameplay_rules_action(body.action);
        let resolution = match resolve_public_gameplay(&rules_context, &action) {
            Ok(resolution) => resolution,
            Err(_) => return public_gameplay_error(400, "PUBLIC_GAMEPLAY_RULES_REJECTED"),
        };
        let event_type = resolution.event_type();
        let payload = match serde_json::to_value(&resolution) {
            Ok(payload) => payload,
            Err(_) => return internal_error(),
        };
        let event = match commit_agent_gateway_event(
            custody,
            &context,
            &body.command,
            event_type,
            &payload,
            "party_visible",
            CanonicalAgentActor::Workflow,
        ) {
            Ok(event) => event,
            Err(_) => return public_gameplay_error(409, "PUBLIC_GAMEPLAY_CANONICAL_COMMIT_FAILED"),
        };
        HttpResponse::json(
            200,
            json!({
                "action_id": body.action_id,
                "aggregate_version": event.stream_version,
                "event_sequence": event.sequence,
                "event_type": event.event_type,
                "result": payload,
            }),
        )
    }
}

fn public_gameplay_context_key(
    action: &PublicGameplayActionApiRequest,
) -> (&str, &str, PublicGameplayContextKind) {
    match action {
        PublicGameplayActionApiRequest::NpcInteraction {
            character_id,
            npc_id,
            ..
        } => (
            character_id,
            npc_id,
            PublicGameplayContextKind::NpcInteraction,
        ),
        PublicGameplayActionApiRequest::CombatRound {
            character_id,
            npc_id,
            ..
        } => (character_id, npc_id, PublicGameplayContextKind::CombatRound),
        PublicGameplayActionApiRequest::ChaseSegment {
            character_id,
            npc_id,
            initial_range,
            ..
        } => (
            character_id,
            npc_id,
            PublicGameplayContextKind::ChaseSegment {
                initial_range: *initial_range,
            },
        ),
    }
}

fn public_gameplay_rules_action(action: PublicGameplayActionApiRequest) -> PublicGameplayAction {
    match action {
        PublicGameplayActionApiRequest::NpcInteraction {
            character_id,
            npc_id,
            approach,
            public_response,
        } => PublicGameplayAction::NpcInteraction {
            character_id,
            npc_id,
            approach,
            public_response,
        },
        PublicGameplayActionApiRequest::CombatRound {
            character_id,
            npc_id,
            action_kind,
            defense,
        } => PublicGameplayAction::CombatRound {
            character_id,
            npc_id,
            action_kind,
            defense,
        },
        PublicGameplayActionApiRequest::ChaseSegment {
            character_id,
            npc_id,
            initial_range,
            obstacle_id,
            obstacle_cost,
        } => PublicGameplayAction::ChaseSegment {
            character_id,
            npc_id,
            initial_range,
            obstacle_id,
            obstacle_cost,
        },
    }
}

fn public_gameplay_repository_error(error: CoreDomainRepositoryError) -> HttpResponse {
    match error {
        CoreDomainRepositoryError::NotFound(_) => {
            public_gameplay_error(404, "PUBLIC_GAMEPLAY_CONTEXT_NOT_FOUND")
        }
        CoreDomainRepositoryError::Forbidden
        | CoreDomainRepositoryError::PolicyEvidenceMismatch => {
            public_gameplay_error(403, "PUBLIC_GAMEPLAY_AUTHORITY_FORBIDDEN")
        }
        CoreDomainRepositoryError::InvalidInput(_) => {
            public_gameplay_error(409, "PUBLIC_GAMEPLAY_CONTEXT_REJECTED")
        }
        CoreDomainRepositoryError::Domain(_)
        | CoreDomainRepositoryError::Integrity(_)
        | CoreDomainRepositoryError::ConcurrentStart => {
            public_gameplay_error(409, "PUBLIC_GAMEPLAY_CONTEXT_CONFLICT")
        }
        CoreDomainRepositoryError::Canonical(_)
        | CoreDomainRepositoryError::Database(_)
        | CoreDomainRepositoryError::Serialization => {
            public_gameplay_error(503, "PUBLIC_GAMEPLAY_WORKFLOW_UNAVAILABLE")
        }
    }
}

fn public_gameplay_error(status: u16, code: &str) -> HttpResponse {
    HttpResponse::json(status, json!({"error": code}))
}
