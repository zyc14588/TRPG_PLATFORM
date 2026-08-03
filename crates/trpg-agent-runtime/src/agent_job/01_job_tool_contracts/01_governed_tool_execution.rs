#[async_trait]
impl AgentJobToolPort for GovernedAgentJobToolPort {
    async fn execute(
        &self,
        job: &DurableAgentJob,
        call: &AgentJobToolCall,
        idempotency_key: &str,
        now_unix_ms: i64,
    ) -> AgentJobResult<AgentJobToolResult> {
        if job.authority_mode != "AI_KP"
            || job.agent_kind != "ai_keeper_orchestrator"
            || idempotency_key != format!("{}:tool", job.idempotency_key)
        {
            return Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED"));
        }
        if matches!(
            call.name.as_str(),
            "resolve_npc_interaction" | "resolve_combat_round" | "resolve_chase_segment"
        ) {
            let context = self
                .context
                .as_ref()
                .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED"))?
                .load_context(&job.job_id)
                .await?;
            validate_gameplay_tool_binding(&context.input_payload_json, call)?;
            let result = self.gameplay.resolve(job, call).await?;
            if !result.is_object() {
                return Err(AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"));
            }
            let encoded = serde_json::to_vec(&result)
                .map_err(|_| AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"))?;
            let digest = format!(
                "{:x}",
                Sha256::digest([job.job_id.as_bytes(), &encoded].concat())
            );
            return Ok(AgentJobToolResult {
                execution_id: format!("gameplay_{}", &digest[..32]),
                result,
                result_hash: format!("sha256:{:x}", Sha256::digest(&encoded)),
            });
        }
        if call.name != "request_skill_check" {
            return Err(AgentJobError::terminal("AGENT_TOOL_PERMISSION_DENIED"));
        }
        let arguments: SkillCheckToolArguments = serde_json::from_value(call.arguments.clone())
            .map_err(|_| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
        if arguments.adjustment != "NONE" {
            return Err(AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"));
        }
        let rules = Arc::clone(&self.rules);
        let receipt = self
            .workflow
            .execute_agent_job_skill_check(
                &AgentJobSkillCheckDraft {
                    job_id: job.job_id.clone(),
                    claim_owner: job
                        .claim_owner
                        .clone()
                        .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?,
                    claim_token: job
                        .claim_token
                        .clone()
                        .ok_or_else(|| AgentJobError::retryable("AGENT_JOB_LEASE_LOST"))?,
                    expected_attempt: job.attempt,
                    idempotency_key: idempotency_key.to_owned(),
                    character_id: arguments.character_id,
                    skill_name: arguments.skill_name,
                    adjustment: arguments.adjustment,
                    now_unix_ms,
                },
                move |target| {
                    let roll = rules.roll_skill_check(target).map_err(|_| {
                        WorkflowStoreError::IntegrityViolation("agent_skill_check_rule_failure")
                    })?;
                    Ok(AgentJobSkillCheckRollDraft {
                        execution_id: roll.execution_id,
                        roll: roll.roll,
                        selected_tens_digit: roll.selected_tens_digit,
                        ones_digit: roll.ones_digit,
                        success_level: roll.success_level,
                    })
                },
            )
            .await
            .map_err(|error| match error {
                WorkflowStoreError::NotFound => {
                    AgentJobError::terminal("AGENT_SKILL_TARGET_NOT_FOUND")
                }
                other => map_store_error(other),
            })?;
        let result = serde_json::from_str(&receipt.result_json)
            .map_err(|_| AgentJobError::terminal("AGENT_TOOL_RESULT_INVALID"))?;
        Ok(AgentJobToolResult {
            execution_id: receipt.execution_id,
            result,
            result_hash: receipt.result_hash,
        })
    }
}

fn validate_gameplay_tool_binding(
    input_payload_json: &str,
    call: &AgentJobToolCall,
) -> AgentJobResult<()> {
    let payload: Value = serde_json::from_str(input_payload_json)
        .map_err(|_| AgentJobError::terminal("AGENT_JOB_INPUT_EVENT_INVALID"))?;
    let input = payload
        .get("input")
        .and_then(Value::as_object)
        .filter(|input| input.get("kind").and_then(Value::as_str) == Some("player_action"))
        .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
    let intent = input
        .get("intent")
        .and_then(Value::as_object)
        .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
    let arguments = call
        .arguments
        .as_object()
        .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
    let intent_kind = intent
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"))?;
    let (expected_tool, required_keys): (&str, &[&str]) = match intent_kind {
        "NPC_INTERACTION" => (
            "resolve_npc_interaction",
            &[
                "session_id",
                "character_id",
                "npc_id",
                "approach",
                "public_response",
            ],
        ),
        "COMBAT_ROUND" => (
            "resolve_combat_round",
            &[
                "session_id",
                "character_id",
                "npc_id",
                "action_kind",
                "defense",
            ],
        ),
        "CHASE_SEGMENT" => (
            "resolve_chase_segment",
            &[
                "session_id",
                "character_id",
                "npc_id",
                "initial_range",
                "obstacle_id",
                "obstacle_cost",
            ],
        ),
        _ => return Err(AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID")),
    };
    if call.name != expected_tool
        || arguments.len() != required_keys.len()
        || required_keys
            .iter()
            .any(|key| !arguments.contains_key(*key))
        || input.get("session_id") != arguments.get("session_id")
        || input.get("character_id") != arguments.get("character_id")
        || intent.get("npc_id") != arguments.get("npc_id")
    {
        return Err(AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"));
    }
    let bounded_choices_match = match intent_kind {
        "NPC_INTERACTION" => {
            intent.get("approach") == arguments.get("approach")
                && arguments
                    .get("public_response")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty() && value.len() <= 2_000)
        }
        "COMBAT_ROUND" => {
            intent.get("action_kind") == arguments.get("action_kind")
                && intent.get("defense") == arguments.get("defense")
        }
        "CHASE_SEGMENT" => {
            intent.get("initial_range") == arguments.get("initial_range")
                && intent.get("obstacle_id") == arguments.get("obstacle_id")
                && intent.get("obstacle_cost") == arguments.get("obstacle_cost")
        }
        _ => false,
    };
    if !bounded_choices_match {
        return Err(AgentJobError::terminal("AGENT_TOOL_ARGUMENTS_INVALID"));
    }
    Ok(())
}

#[cfg(test)]
mod public_gameplay_tool_binding_tests {
    use super::*;

    fn payload(intent: Value) -> String {
        json!({
            "input": {
                "kind": "player_action",
                "session_id": "session_rf04",
                "character_id": "character_rf04",
                "intent": intent
            }
        })
        .to_string()
    }

    #[test]
    fn each_public_gameplay_tool_is_bound_to_the_canonical_input() {
        let cases = [
            (
                payload(json!({
                    "kind": "NPC_INTERACTION",
                    "npc_id": "npc_rf04",
                    "approach": "Ask about the archive"
                })),
                AgentJobToolCall {
                    name: "resolve_npc_interaction".to_owned(),
                    arguments: json!({
                        "session_id": "session_rf04",
                        "character_id": "character_rf04",
                        "npc_id": "npc_rf04",
                        "approach": "Ask about the archive",
                        "public_response": "Marta points toward the locked stacks."
                    }),
                },
            ),
            (
                payload(json!({
                    "kind": "COMBAT_ROUND",
                    "npc_id": "npc_rf04",
                    "action_kind": "MELEE",
                    "defense": "DODGE"
                })),
                AgentJobToolCall {
                    name: "resolve_combat_round".to_owned(),
                    arguments: json!({
                        "session_id": "session_rf04",
                        "character_id": "character_rf04",
                        "npc_id": "npc_rf04",
                        "action_kind": "MELEE",
                        "defense": "DODGE"
                    }),
                },
            ),
            (
                payload(json!({
                    "kind": "CHASE_SEGMENT",
                    "npc_id": "npc_rf04",
                    "initial_range": 2,
                    "obstacle_id": "stairs",
                    "obstacle_cost": 1
                })),
                AgentJobToolCall {
                    name: "resolve_chase_segment".to_owned(),
                    arguments: json!({
                        "session_id": "session_rf04",
                        "character_id": "character_rf04",
                        "npc_id": "npc_rf04",
                        "initial_range": 2,
                        "obstacle_id": "stairs",
                        "obstacle_cost": 1
                    }),
                },
            ),
        ];
        for (input, call) in cases {
            validate_gameplay_tool_binding(&input, &call).unwrap();
        }
    }

    #[test]
    fn model_cannot_change_canonical_gameplay_ids_or_choices() {
        let input = payload(json!({
            "kind": "COMBAT_ROUND",
            "npc_id": "npc_rf04",
            "action_kind": "MELEE",
            "defense": "DODGE"
        }));
        for arguments in [
            json!({
                "session_id": "session_other",
                "character_id": "character_rf04",
                "npc_id": "npc_rf04",
                "action_kind": "MELEE",
                "defense": "DODGE"
            }),
            json!({
                "session_id": "session_rf04",
                "character_id": "character_rf04",
                "npc_id": "npc_rf04",
                "action_kind": "FIREARM",
                "defense": "DODGE"
            }),
        ] {
            let error = validate_gameplay_tool_binding(
                &input,
                &AgentJobToolCall {
                    name: "resolve_combat_round".to_owned(),
                    arguments,
                },
            )
            .unwrap_err();
            assert_eq!(error.code(), "AGENT_TOOL_ARGUMENTS_INVALID");
        }
    }
}
