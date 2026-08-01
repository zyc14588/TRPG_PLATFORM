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
    let system = format!(
        "COC7 agent runtime. template={}:{}; authority={}; output_visibility={}; \
         Return only the requested structured decision. Never invent dice, \
         mutate state directly, reveal hidden facts, or bypass a tool.",
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
        "Canonical input event:\n{}\n\nVisible retrieval:\n{}",
        context.input_payload_json, retrieved
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
                                    "apply_san_loss", "change_scene"
                                ]
                            },
                            "arguments": {"type": "object"}
                        }
                    }
                ]
            }
        }
    });
    let tool_schema = json!({
        "type": "object",
        "additionalProperties": false
    });
    let tools = [
        (
            "request_skill_check",
            "Request a server-side COC7 skill check",
        ),
        (
            "reveal_clue",
            "Reveal an authorized clue through the rules workflow",
        ),
        ("apply_san_loss", "Apply rules-engine validated sanity loss"),
        ("change_scene", "Request a governed scene transition"),
    ]
    .into_iter()
    .map(|(name, description)| ModelToolDefinition {
        name: name.to_owned(),
        description: description.to_owned(),
        input_schema: tool_schema.clone(),
    })
    .collect::<Vec<_>>();
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
        structured_output: Some(StructuredOutputRequest {
            name: "agent_npc_turn".to_owned(),
            schema: structured_schema.clone(),
        }),
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
            tool_schema_hash: sha256_label(
                serde_json::to_string(&(structured_schema, tool_schema))
                    .map_err(|_| AgentJobError::terminal("AGENT_TOOL_SCHEMA_INVALID"))?
                    .as_bytes(),
            ),
            retrieval_hash: sha256_label(retrieval_binding.as_bytes()),
            input_hash: sha256_label(user.as_bytes()),
        },
    ))
}
