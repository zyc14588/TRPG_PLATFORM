trait AgentJobTransitionDraftExt {
    fn with_next_attempt(self, next_attempt_at_unix_ms: Option<i64>) -> Self;
}

impl AgentJobTransitionDraftExt for AgentJobTransitionDraft {
    fn with_next_attempt(mut self, next_attempt_at_unix_ms: Option<i64>) -> Self {
        self.next_attempt_at_unix_ms = next_attempt_at_unix_ms;
        self
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VisibilityScope {
    allowed_labels: Vec<String>,
    subject_id: Option<String>,
    output_label: String,
}

struct ModelRequestHashes {
    prompt_template_hash: String,
    tool_schema_hash: String,
    retrieval_hash: String,
    input_hash: String,
}

fn validate_authority_snapshot(
    job: &DurableAgentJob,
    authority: &DurableAgentAuthoritySnapshot,
) -> AgentJobResult<()> {
    if authority.contract_id != job.authority_contract_id
        || authority.campaign_id != job.campaign_id
        || authority.authority_mode != job.authority_mode
        || authority.contract_version != job.authority_contract_version
        || authority.prompt_version != job.prompt_template_version
        || authority.tool_schema_version != job.tool_schema_version
        || authority.model_route_snapshot.trim().is_empty()
        || authority.agent_pack_version.trim().is_empty()
        || (job.authority_mode == "AI_KP"
            && (job.agent_kind != "ai_keeper_orchestrator"
                || job.actor_id != authority.authority_owner))
        || (job.authority_mode == "HUMAN_KP" && job.agent_kind != "keeper_copilot")
    {
        return Err(AgentJobError::terminal(
            "AGENT_JOB_AUTHORITY_SNAPSHOT_MISMATCH",
        ));
    }
    Ok(())
}

fn validate_context_scope(
    job: &DurableAgentJob,
    context: &DurableAgentContextSnapshot,
) -> AgentJobResult<VisibilityScope> {
    let scope: VisibilityScope = serde_json::from_str(&job.visibility_scope_json)
        .map_err(|_| AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"))?;
    let known = [
        "public",
        "party_visible",
        "private_to_player",
        "private_to_group",
        "keeper_only",
        "ai_internal",
        "system_only",
        "spectator_visible",
        "spectator_hidden",
        "investigator_private",
        "system_private",
    ];
    let labels = scope.allowed_labels.iter().collect::<HashSet<_>>();
    if scope.allowed_labels.is_empty()
        || labels.len() != scope.allowed_labels.len()
        || scope.allowed_labels.len() > known.len()
        || scope
            .allowed_labels
            .iter()
            .any(|label| !known.contains(&label.as_str()))
        || !scope.allowed_labels.contains(&scope.output_label)
        || Visibility::try_from_parts(&scope.output_label, scope.subject_id.as_deref()).is_err()
    {
        return Err(AgentJobError::terminal("RAG_VISIBILITY_SCOPE_INVALID"));
    }
    if context.chunks.iter().any(|chunk| {
        !labels.contains(&chunk.visibility_label)
            || (chunk.visibility_subject.is_some() && chunk.visibility_subject != scope.subject_id)
            || !valid_plain_hash(&chunk.chunk_hash)
            || !valid_provenance(&chunk.fact_provenance_json)
    }) {
        return Err(AgentJobError::terminal("RAG_VISIBILITY_SCOPE_VIOLATION"));
    }
    serde_json::from_str::<Value>(&context.input_payload_json)
        .map_err(|_| AgentJobError::terminal("AGENT_JOB_INPUT_EVENT_INVALID"))?;
    Ok(scope)
}

fn build_model_request(
    job: &DurableAgentJob,
    context: &DurableAgentContextSnapshot,
    scope: &VisibilityScope,
    max_context_bytes: usize,
) -> AgentJobResult<(ModelChatRequest, ModelRequestHashes)> {
    let canonical_gameplay_tool = canonical_public_gameplay_tool(&context.input_payload_json)?;
    let ollama_structured_gameplay =
        job.provider_type == "ollama" && canonical_gameplay_tool.is_some();
    let system = format!(
        "COC7 agent runtime. template={}:{}; authority={}; output_visibility={}; \
         Return only the requested structured decision. Never invent dice, \
         mutate state directly, reveal hidden facts, or bypass a tool. For \
         NPC_INTERACTION, COMBAT_ROUND, or CHASE_SEGMENT player actions, call \
         the matching resolve_* tool once and copy IDs and bounded choices \
         from the canonical input; the server loads all rules statistics.",
        job.prompt_template_id, job.prompt_template_version, job.authority_mode, scope.output_label,
    );
    let retrieved = context
        .chunks
        .iter()
        .map(|chunk| {
            format!(
                "[chunk:{} source:{} visibility:{} hash:{}]\n{}",
                chunk.chunk_id,
                chunk.source_event_sequence,
                chunk.visibility_label,
                chunk.chunk_hash,
                chunk.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let user = format!(
        "{}Canonical input event:\n{}\n\nVisible retrieval:\n{}",
        gameplay_model_instruction(&job.provider_type, canonical_gameplay_tool.as_ref()),
        context.input_payload_json,
        retrieved
    );
    let total_bytes = system
        .len()
        .checked_add(user.len())
        .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_CONTEXT_BUDGET_EXCEEDED"))?;
    if total_bytes > max_context_bytes {
        return Err(AgentJobError::terminal("AGENT_JOB_CONTEXT_BUDGET_EXCEEDED"));
    }
    let structured_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["kind", "player_visible_text", "tool"],
        "properties": {
            "kind": {"const": "npc_turn"},
            "player_visible_text": {"type": "string", "minLength": 1, "maxLength": 16384},
            "tool": {
                "anyOf": [
                    {"type": "null"},
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["name", "arguments"],
                        "properties": {
                            "name": {
                                "enum": [
                                    "request_skill_check", "reveal_clue",
                                    "apply_san_loss", "change_scene",
                                    "resolve_npc_interaction", "resolve_combat_round",
                                    "resolve_chase_segment"
                                ]
                            },
                            "arguments": {"type": "object"}
                        }
                    }
                ]
            }
        }
    });
    let permissive_draft_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    });
    let skill_check_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["character_id", "skill_name", "adjustment"],
        "properties": {
            "character_id": {"type": "string", "minLength": 1, "maxLength": 128},
            "skill_name": {"type": "string", "minLength": 1, "maxLength": 128},
            "adjustment": {"const": "NONE"}
        }
    });
    let npc_interaction_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["session_id", "character_id", "npc_id", "approach", "public_response"],
        "properties": {
            "session_id": {"type": "string", "minLength": 1, "maxLength": 128},
            "character_id": {"type": "string", "minLength": 1, "maxLength": 128},
            "npc_id": {"type": "string", "minLength": 1, "maxLength": 128},
            "approach": {"type": "string", "minLength": 1, "maxLength": 500},
            "public_response": {"type": "string", "minLength": 1, "maxLength": 2000}
        }
    });
    let combat_round_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["session_id", "character_id", "npc_id", "action_kind", "defense"],
        "properties": {
            "session_id": {"type": "string", "minLength": 1, "maxLength": 128},
            "character_id": {"type": "string", "minLength": 1, "maxLength": 128},
            "npc_id": {"type": "string", "minLength": 1, "maxLength": 128},
            "action_kind": {"enum": ["MELEE", "FIREARM"]},
            "defense": {"enum": ["NONE", "DODGE"]}
        }
    });
    let chase_segment_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["session_id", "character_id", "npc_id", "initial_range", "obstacle_id", "obstacle_cost"],
        "properties": {
            "session_id": {"type": "string", "minLength": 1, "maxLength": 128},
            "character_id": {"type": "string", "minLength": 1, "maxLength": 128},
            "npc_id": {"type": "string", "minLength": 1, "maxLength": 128},
            "initial_range": {"type": "integer", "minimum": 1, "maximum": 4},
            "obstacle_id": {"type": ["string", "null"], "maxLength": 128},
            "obstacle_cost": {"type": "integer", "minimum": 0, "maximum": 2}
        }
    });
    let structured_schema = match canonical_gameplay_tool.as_ref() {
        Some(tool) if ollama_structured_gameplay => ollama_gameplay_decision_schema(tool)?,
        _ => structured_schema,
    };
    let tools = [
        (
            "request_skill_check",
            "Request a server-side COC7 skill check",
            skill_check_schema,
        ),
        (
            "reveal_clue",
            "Reveal an authorized clue through the rules workflow",
            permissive_draft_schema.clone(),
        ),
        (
            "apply_san_loss",
            "Apply rules-engine validated sanity loss",
            permissive_draft_schema.clone(),
        ),
        (
            "change_scene",
            "Request a governed scene transition",
            permissive_draft_schema,
        ),
        (
            "resolve_npc_interaction",
            "Record a visible NPC interaction using server-loaded scenario context",
            npc_interaction_schema,
        ),
        (
            "resolve_combat_round",
            "Resolve one basic COC7 combat round with server profiles and dice",
            combat_round_schema,
        ),
        (
            "resolve_chase_segment",
            "Resolve one basic COC7 chase segment with server profiles and dice",
            chase_segment_schema,
        ),
    ]
    .into_iter()
    .map(|(name, description, input_schema)| ModelToolDefinition {
        name: name.to_owned(),
        description: description.to_owned(),
        input_schema,
    })
    .collect::<Vec<_>>();
    let tools = if ollama_structured_gameplay {
        Vec::new()
    } else {
        match canonical_gameplay_tool {
            Some(tool) => vec![tool],
            None => tools,
        }
    };
    let structured_output = if ollama_structured_gameplay {
        Some(StructuredOutputRequest {
            name: "agent_public_gameplay".to_owned(),
            schema: structured_schema.clone(),
        })
    } else {
        agent_structured_output(&job.provider_type, &structured_schema)
    };
    let request = ModelChatRequest {
        messages: vec![
            ModelMessage {
                role: ModelMessageRole::System,
                content: system.clone(),
            },
            ModelMessage {
                role: ModelMessageRole::User,
                content: user.clone(),
            },
        ],
        structured_output,
        tools,
    };
    let retrieval_binding = context
        .chunks
        .iter()
        .map(|chunk| {
            format!(
                "{}:{}:{}",
                chunk.chunk_id, chunk.source_event_sequence, chunk.chunk_hash
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    let tool_schema_hash = sha256_label(
        serde_json::to_string(&(&request.structured_output, &request.tools))
            .map_err(|_| AgentJobError::terminal("AGENT_TOOL_SCHEMA_INVALID"))?
            .as_bytes(),
    );
    Ok((
        request,
        ModelRequestHashes {
            prompt_template_hash: sha256_label(
                format!(
                    "{}:{}:{}",
                    job.prompt_template_id, job.prompt_template_version, system
                )
                .as_bytes(),
            ),
            tool_schema_hash,
            retrieval_hash: sha256_label(retrieval_binding.as_bytes()),
            input_hash: sha256_label(user.as_bytes()),
        },
    ))
}

include!("08_context_and_model_request/01_gameplay_request_helpers.rs");
