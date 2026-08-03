include!("06_structured_output_validation.rs");

fn parse_probe_response(
    provider_type: ProviderType,
    model_id: &str,
    declared: ProviderCapabilities,
    value: &Value,
) -> ModelProviderResult<ProviderCapabilities> {
    let remote = if provider_type == ProviderType::Ollama {
        value
            .get("capabilities")
            .map(parse_capabilities)
            .transpose()?
            .unwrap_or(declared)
    } else {
        let models = value
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(capability_error)?;
        let model = models
            .iter()
            .find(|model| model.get("id").and_then(Value::as_str) == Some(model_id))
            .ok_or_else(capability_error)?;
        model
            .get("capabilities")
            .map(parse_capabilities)
            .transpose()?
            .unwrap_or(declared)
    };
    Ok(declared.intersection(remote))
}

fn parse_capabilities(value: &Value) -> ModelProviderResult<ProviderCapabilities> {
    if let Some(object) = value.as_object() {
        return Ok(ProviderCapabilities {
            chat: required_bool(object, "chat")?,
            streaming: required_bool(object, "streaming")?,
            structured_output: required_bool(object, "structured_output")?,
            tool_requests: required_bool(object, "tool_requests")?,
            embeddings: required_bool(object, "embeddings")?,
        });
    }
    let values = value
        .as_array()
        .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_PROBE_SCHEMA_INVALID"))?;
    let values = values
        .iter()
        .map(Value::as_str)
        .collect::<Option<HashSet<_>>>()
        .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_PROBE_SCHEMA_INVALID"))?;
    let completion = values.contains("completion") || values.contains("chat");
    Ok(ProviderCapabilities {
        chat: completion,
        streaming: completion,
        structured_output: completion,
        tool_requests: values.contains("tools") || values.contains("tool_requests"),
        embeddings: values.contains("embedding") || values.contains("embeddings"),
    })
}

fn required_bool(object: &Map<String, Value>, key: &str) -> ModelProviderResult<bool> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_PROBE_SCHEMA_INVALID"))
}

fn parse_chat_response(
    provider_type: ProviderType,
    structured_schema: Option<&Value>,
    body: &[u8],
) -> ModelProviderResult<ModelChatResponse> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| invalid_schema_error("MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID"))?;
    let (message, usage) = if provider_type == ProviderType::Ollama {
        (
            value
                .get("message")
                .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID"))?,
            ModelTokenUsage {
                input_tokens: optional_u64(&value, "prompt_eval_count")?,
                output_tokens: optional_u64(&value, "eval_count")?,
            },
        )
    } else {
        let choice = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID"))?;
        (
            choice
                .get("message")
                .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID"))?,
            parse_openai_usage(value.get("usage"))?,
        )
    };
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let tool_calls = parse_tool_calls(message.get("tool_calls"), provider_type)?;
    if content.is_empty() && tool_calls.is_empty() {
        return Err(invalid_schema_error(
            "MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID",
        ));
    }
    let structured_output = match structured_schema {
        Some(schema) => {
            let output = serde_json::from_str(&content)
                .map_err(|_| invalid_schema_error("MODEL_PROVIDER_STRUCTURED_OUTPUT_INVALID"))?;
            if !validate_structured_output(schema, &output) {
                return Err(invalid_schema_error(
                    "MODEL_PROVIDER_STRUCTURED_OUTPUT_INVALID",
                ));
            }
            Some(output)
        }
        None => None,
    };
    Ok(ModelChatResponse {
        content,
        structured_output,
        tool_calls,
        usage,
    })
}

fn parse_openai_usage(value: Option<&Value>) -> ModelProviderResult<ModelTokenUsage> {
    let Some(value) = value else {
        return Ok(ModelTokenUsage::default());
    };
    Ok(ModelTokenUsage {
        input_tokens: optional_u64(value, "prompt_tokens")?,
        output_tokens: optional_u64(value, "completion_tokens")?,
    })
}

fn optional_u64(value: &Value, key: &str) -> ModelProviderResult<u64> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(0),
        Some(value) => value
            .as_u64()
            .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID")),
    }
}

fn parse_tool_calls(
    value: Option<&Value>,
    provider_type: ProviderType,
) -> ModelProviderResult<Vec<ModelToolCall>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let calls = value
        .as_array()
        .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_TOOL_CALL_INVALID"))?;
    let mut ids = HashSet::new();
    let mut parsed = Vec::with_capacity(calls.len());
    for (index, call) in calls.iter().enumerate() {
        let function = call
            .get("function")
            .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_TOOL_CALL_INVALID"))?;
        let name = function
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| valid_wire_name(name))
            .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_TOOL_CALL_INVALID"))?;
        let id = call
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("{}-tool-{index}", provider_type.route_name()));
        if !valid_wire_name(&id) || !ids.insert(id.clone()) {
            return Err(invalid_schema_error("MODEL_PROVIDER_DUPLICATE_TOOL_CALL"));
        }
        let arguments = function
            .get("arguments")
            .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_TOOL_CALL_INVALID"))?;
        let arguments = if let Some(arguments) = arguments.as_str() {
            serde_json::from_str(arguments)
                .map_err(|_| invalid_schema_error("MODEL_PROVIDER_TOOL_CALL_INVALID"))?
        } else {
            arguments.clone()
        };
        if !arguments.is_object() {
            return Err(invalid_schema_error("MODEL_PROVIDER_TOOL_CALL_INVALID"));
        }
        parsed.push(ModelToolCall {
            id,
            name: name.to_owned(),
            arguments,
        });
    }
    Ok(parsed)
}

fn parse_stream_value(
    provider_type: ProviderType,
    value: &Value,
) -> ModelProviderResult<(String, Vec<ModelToolCall>, bool)> {
    if provider_type == ProviderType::Ollama {
        let message = value
            .get("message")
            .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_STREAM_SCHEMA_INVALID"))?;
        let content = message
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let tools = parse_tool_calls(message.get("tool_calls"), provider_type)?;
        let done = value
            .get("done")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_STREAM_SCHEMA_INVALID"))?;
        return Ok((content, tools, done));
    }
    let choice = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_STREAM_SCHEMA_INVALID"))?;
    let delta = choice
        .get("delta")
        .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_STREAM_SCHEMA_INVALID"))?;
    let content = delta
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let tools = parse_tool_calls(delta.get("tool_calls"), provider_type)?;
    Ok((content, tools, false))
}

fn parse_embedding_response(
    provider_type: ProviderType,
    body: &[u8],
) -> ModelProviderResult<ModelEmbeddingResponse> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| invalid_schema_error("MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID"))?;
    let embeddings = if provider_type == ProviderType::Ollama {
        value
            .get("embeddings")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID"))?
            .iter()
            .map(parse_embedding)
            .collect::<ModelProviderResult<Vec<_>>>()?
    } else {
        value
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID"))?
            .iter()
            .map(|item| {
                item.get("embedding")
                    .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID"))
                    .and_then(parse_embedding)
            })
            .collect::<ModelProviderResult<Vec<_>>>()?
    };
    if embeddings.is_empty()
        || embeddings.iter().any(|embedding| {
            embedding.is_empty() || embedding.iter().any(|value| !value.is_finite())
        })
    {
        return Err(invalid_schema_error(
            "MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID",
        ));
    }
    let input_tokens = if provider_type == ProviderType::Ollama {
        optional_u64(&value, "prompt_eval_count")?
    } else {
        value
            .get("usage")
            .map(|usage| optional_u64(usage, "prompt_tokens"))
            .transpose()?
            .unwrap_or(0)
    };
    Ok(ModelEmbeddingResponse {
        embeddings,
        input_tokens,
    })
}

fn parse_embedding(value: &Value) -> ModelProviderResult<Vec<f32>> {
    value
        .as_array()
        .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID"))?
        .iter()
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .map(|value| value as f32)
                .ok_or_else(|| invalid_schema_error("MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID"))
        })
        .collect()
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}
