fn gameplay_model_instruction(provider_type: &str, tool: Option<&ModelToolDefinition>) -> String {
    match (provider_type, tool) {
        ("ollama", Some(tool)) => format!(
            "/no_think\nReturn exactly one structured decision whose tool.name is {}; copy every \
             canonical ID and bounded choice unchanged, satisfy every required field, and return no prose.\n",
            tool.name
        ),
        _ => String::new(),
    }
}

fn ollama_gameplay_decision_schema(tool: &ModelToolDefinition) -> AgentJobResult<Value> {
    if !matches!(
        tool.name.as_str(),
        "resolve_npc_interaction" | "resolve_combat_round" | "resolve_chase_segment"
    ) {
        return Err(AgentJobError::terminal("AGENT_TOOL_SCHEMA_INVALID"));
    }
    let source = tool
        .input_schema
        .as_object()
        .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_SCHEMA_INVALID"))?;
    let required = source
        .get("required")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_SCHEMA_INVALID"))?;
    let source_properties = source
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_SCHEMA_INVALID"))?;
    let mut properties = serde_json::Map::new();
    for (name, schema) in source_properties {
        let schema = schema
            .as_object()
            .ok_or_else(|| AgentJobError::terminal("AGENT_TOOL_SCHEMA_INVALID"))?;
        let compatible = if let Some(value) = schema.get("const") {
            json!({"enum": [value.clone()]})
        } else if name == "public_response"
            && schema.get("type").and_then(Value::as_str) == Some("string")
        {
            // JSON Schema counts characters while the execution gate caps UTF-8
            // bytes. Five hundred Unicode scalar values cannot exceed 2,000
            // bytes, so generation and the fail-closed runtime bound agree.
            json!({"type": "string", "minLength": 1, "maxLength": 500})
        } else if let Some(value_type) = schema.get("type") {
            json!({"type": value_type.clone()})
        } else {
            return Err(AgentJobError::terminal("AGENT_TOOL_SCHEMA_INVALID"));
        };
        properties.insert(name.clone(), compatible);
    }
    let arguments = json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    });
    Ok(json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["kind", "player_visible_text", "tool"],
        "properties": {
            "kind": {"const": "npc_turn"},
            // Ollama's grammar rejects string-length keywords. Keep this
            // decision envelope non-empty and bounded with a fixed public
            // value; NPC dialogue remains model-authored in the governed
            // public_response tool argument.
            "player_visible_text": {
                "const": "The AI Keeper requested server-side rules resolution."
            },
            "tool": {
                "type": "object",
                "additionalProperties": false,
                "required": ["name", "arguments"],
                "properties": {
                    "name": {"const": tool.name},
                    "arguments": arguments
                }
            }
        }
    }))
}

fn canonical_public_gameplay_tool(
    input_payload_json: &str,
) -> AgentJobResult<Option<ModelToolDefinition>> {
    let payload: Value = serde_json::from_str(input_payload_json)
        .map_err(|_| AgentJobError::terminal("AGENT_JOB_INPUT_EVENT_INVALID"))?;
    let Some(input) = payload.get("input").and_then(Value::as_object) else {
        return Ok(None);
    };
    if input.get("kind").and_then(Value::as_str) != Some("player_action") {
        return Ok(None);
    }
    let Some(intent) = input.get("intent").and_then(Value::as_object) else {
        return Ok(None);
    };
    let Some(intent_kind) = intent.get("kind").and_then(Value::as_str) else {
        return Ok(None);
    };
    if !matches!(
        intent_kind,
        "NPC_INTERACTION" | "COMBAT_ROUND" | "CHASE_SEGMENT"
    ) {
        return Ok(None);
    }
    let required = |object: &serde_json::Map<String, Value>, key: &str| {
        object
            .get(key)
            .cloned()
            .ok_or_else(|| AgentJobError::terminal("AGENT_JOB_INPUT_EVENT_INVALID"))
    };
    let session_id = required(input, "session_id")?;
    let character_id = required(input, "character_id")?;
    let npc_id = required(intent, "npc_id")?;
    let (name, description, input_schema) = match intent_kind {
        "NPC_INTERACTION" => {
            let approach = required(intent, "approach")?;
            (
                "resolve_npc_interaction",
                "Resolve this canonical NPC interaction once; every const argument must be copied unchanged",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["session_id", "character_id", "npc_id", "approach", "public_response"],
                    "properties": {
                        "session_id": {"const": session_id},
                        "character_id": {"const": character_id},
                        "npc_id": {"const": npc_id},
                        "approach": {"const": approach},
                        "public_response": {"type": "string", "minLength": 1, "maxLength": 2000}
                    }
                }),
            )
        }
        "COMBAT_ROUND" => {
            let action_kind = required(intent, "action_kind")?;
            let defense = required(intent, "defense")?;
            (
                "resolve_combat_round",
                "Resolve this canonical combat round once; every argument must be copied unchanged",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["session_id", "character_id", "npc_id", "action_kind", "defense"],
                    "properties": {
                        "session_id": {"const": session_id},
                        "character_id": {"const": character_id},
                        "npc_id": {"const": npc_id},
                        "action_kind": {"const": action_kind},
                        "defense": {"const": defense}
                    }
                }),
            )
        }
        "CHASE_SEGMENT" => {
            let initial_range = required(intent, "initial_range")?;
            let obstacle_id = required(intent, "obstacle_id")?;
            let obstacle_cost = required(intent, "obstacle_cost")?;
            (
                "resolve_chase_segment",
                "Resolve this canonical chase segment once; every argument must be copied unchanged",
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["session_id", "character_id", "npc_id", "initial_range", "obstacle_id", "obstacle_cost"],
                    "properties": {
                        "session_id": {"const": session_id},
                        "character_id": {"const": character_id},
                        "npc_id": {"const": npc_id},
                        "initial_range": {"const": initial_range},
                        "obstacle_id": {"const": obstacle_id},
                        "obstacle_cost": {"const": obstacle_cost}
                    }
                }),
            )
        }
        _ => unreachable!("public gameplay intent was matched above"),
    };
    Ok(Some(ModelToolDefinition {
        name: name.to_owned(),
        description: description.to_owned(),
        input_schema,
    }))
}

fn agent_structured_output(
    provider_type: &str,
    structured_schema: &Value,
) -> Option<StructuredOutputRequest> {
    (provider_type != "ollama").then(|| StructuredOutputRequest {
        name: "agent_npc_turn".to_owned(),
        schema: structured_schema.clone(),
    })
}

#[cfg(test)]
mod agent_model_request_tests {
    use super::*;

    #[test]
    fn ollama_uses_native_tool_calls_without_an_incompatible_format_field() {
        let schema = json!({"type": "object"});
        assert!(agent_structured_output("ollama", &schema).is_none());
        assert!(agent_structured_output("cloud", &schema).is_some());
        assert!(agent_structured_output("llama_cpp", &schema).is_some());
    }

    #[test]
    fn public_gameplay_exposes_only_the_canonical_bound_tool() {
        let payload = json!({
            "input": {
                "kind": "player_action",
                "session_id": "session_rf04",
                "character_id": "character_rf04",
                "intent": {
                    "kind": "COMBAT_ROUND",
                    "npc_id": "npc_rf04",
                    "action_kind": "MELEE",
                    "defense": "DODGE"
                }
            }
        })
        .to_string();
        let tool = canonical_public_gameplay_tool(&payload).unwrap().unwrap();
        assert_eq!(tool.name, "resolve_combat_round");
        assert_eq!(
            tool.input_schema.pointer("/properties/session_id/const"),
            Some(&json!("session_rf04"))
        );
        assert_eq!(
            tool.input_schema.pointer("/properties/action_kind/const"),
            Some(&json!("MELEE"))
        );
    }

    #[test]
    fn non_gameplay_input_keeps_the_general_tool_contract() {
        let payload = json!({
            "input": {"kind": "player_action", "intent": {"kind": "INVESTIGATION"}}
        })
        .to_string();
        assert!(canonical_public_gameplay_tool(&payload).unwrap().is_none());
    }

    #[test]
    fn ollama_gameplay_instruction_requests_one_structured_tool_decision_without_thinking() {
        let tool = ModelToolDefinition {
            name: "resolve_chase_segment".to_owned(),
            description: "test".to_owned(),
            input_schema: json!({"type": "object"}),
        };
        let instruction = gameplay_model_instruction("ollama", Some(&tool));
        assert!(instruction.starts_with("/no_think\n"));
        assert!(instruction.contains("tool.name is resolve_chase_segment"));
        assert!(gameplay_model_instruction("cloud", Some(&tool)).is_empty());
        assert!(gameplay_model_instruction("ollama", None).is_empty());
    }

    #[test]
    fn ollama_gameplay_schema_uses_grammar_compatible_canonical_singleton_enums() {
        let payload = json!({
            "input": {
                "kind": "player_action",
                "session_id": "session_rf04",
                "character_id": "character_rf04",
                "intent": {
                    "kind": "NPC_INTERACTION",
                    "npc_id": "npc_rf04",
                    "approach": "Ask about the archive"
                }
            }
        })
        .to_string();
        let tool = canonical_public_gameplay_tool(&payload).unwrap().unwrap();
        let schema = ollama_gameplay_decision_schema(&tool).unwrap();
        assert_eq!(
            schema.pointer("/properties/tool/properties/name/const"),
            Some(&json!("resolve_npc_interaction"))
        );
        assert_eq!(
            schema.pointer("/properties/tool/properties/arguments/properties/session_id/enum/0"),
            Some(&json!("session_rf04"))
        );
        assert!(schema
            .pointer("/properties/tool/properties/arguments/properties/session_id/const")
            .is_none());
        assert_eq!(
            schema.pointer(
                "/properties/tool/properties/arguments/properties/public_response/maxLength"
            ),
            Some(&json!(500))
        );
        assert_eq!(
            schema.pointer("/properties/player_visible_text/const"),
            Some(&json!(
                "The AI Keeper requested server-side rules resolution."
            ))
        );
    }
}
