impl<R: SecretResolver + 'static> HttpModelProvider<R> {
    async fn require_capabilities(
        &self,
        required: &[RequiredProviderCapability],
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderCapabilities> {
        for capability in required {
            if !self.runtime.declared_capabilities.supports(*capability) {
                return Err(capability_error());
            }
        }

        let cached = *self.effective_capabilities.read().await;
        let capabilities = match cached {
            Some(capabilities) => capabilities,
            None => self.probe_capabilities(cancellation).await?.output,
        };
        if required
            .iter()
            .all(|capability| capabilities.supports(*capability))
        {
            Ok(capabilities)
        } else {
            Err(capability_error())
        }
    }

    fn chat_payload(&self, request: &ModelChatRequest, stream: bool) -> Value {
        let messages = request
            .messages
            .iter()
            .map(|message| {
                json!({
                    "role": message.role.as_str(),
                    "content": message.content,
                })
            })
            .collect::<Vec<_>>();
        let tools = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.input_schema,
                    }
                })
            })
            .collect::<Vec<_>>();
        let mut payload = Map::new();
        payload.insert(
            "model".to_owned(),
            Value::String(self.runtime.provider.model_id.clone()),
        );
        payload.insert("messages".to_owned(), Value::Array(messages));
        payload.insert("stream".to_owned(), Value::Bool(stream));
        let max_output_tokens = Value::from(self.runtime.max_output_tokens.get());
        match self.runtime.provider.provider_type {
            ProviderType::Cloud => {
                payload.insert("max_completion_tokens".to_owned(), max_output_tokens);
                if let Some(reasoning_effort) = &self.runtime.cloud_reasoning_effort {
                    payload.insert(
                        "reasoning_effort".to_owned(),
                        Value::String(reasoning_effort.as_str().to_owned()),
                    );
                }
            }
            ProviderType::Ollama => {
                payload.insert(
                    "options".to_owned(),
                    json!({"num_predict": max_output_tokens}),
                );
            }
            ProviderType::LlamaCpp | ProviderType::LocalOpenAiCompatible => {
                payload.insert("max_tokens".to_owned(), max_output_tokens);
            }
        }
        if !tools.is_empty() {
            payload.insert("tools".to_owned(), Value::Array(tools));
        }
        if self.runtime.provider.provider_type == ProviderType::Ollama
            && request
                .messages
                .last()
                .is_some_and(|message| message.content.starts_with("/no_think\n"))
        {
            // Ollama exposes thinking as an explicit request option. Models such
            // as Qwen do not reliably interpret the textual directive alone,
            // so preserve the caller's explicit no-think contract natively.
            payload.insert("think".to_owned(), Value::Bool(false));
        }
        if let Some(structured) = &request.structured_output {
            if self.runtime.provider.provider_type == ProviderType::Ollama {
                payload.insert("format".to_owned(), structured.schema.clone());
            } else {
                payload.insert(
                    "response_format".to_owned(),
                    json!({
                        "type": "json_schema",
                        "json_schema": {
                            "name": structured.name,
                            "strict": true,
                            "schema": structured.schema,
                        }
                    }),
                );
            }
        }
        Value::Object(payload)
    }

    async fn parse_stream(
        &self,
        mut response: Response,
        sink: &dyn ModelStreamSink,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<()> {
        let mut buffer = Vec::new();
        let mut sequence = 0_u64;
        let mut terminal_seen = false;
        let mut tool_call_ids = HashSet::new();
        loop {
            let chunk = tokio::select! {
                _ = cancellation.cancelled() => return Err(cancelled_error()),
                chunk = response.chunk() => chunk,
            }
            .map_err(|error| classify_reqwest_error(error, ModelOperation::StreamingChat))?;
            let Some(chunk) = chunk else {
                break;
            };
            if buffer.len().saturating_add(chunk.len()) > MAX_STREAM_LINE_BYTES {
                return Err(invalid_schema_error(
                    "MODEL_PROVIDER_STREAM_FRAME_TOO_LARGE",
                ));
            }
            buffer.extend_from_slice(&chunk);
            while let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
                let mut line = buffer.drain(..=newline).collect::<Vec<_>>();
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if self
                    .emit_stream_line(
                        &line,
                        &mut sequence,
                        &mut terminal_seen,
                        &mut tool_call_ids,
                        sink,
                        cancellation,
                    )
                    .await?
                {
                    return Ok(());
                }
            }
        }
        if !buffer.is_empty() {
            self.emit_stream_line(
                &buffer,
                &mut sequence,
                &mut terminal_seen,
                &mut tool_call_ids,
                sink,
                cancellation,
            )
            .await?;
        }
        if terminal_seen {
            Ok(())
        } else {
            Err(ModelProviderError::new(
                ModelProviderErrorKind::Transport,
                "MODEL_PROVIDER_STREAM_DISCONNECTED",
                false,
                None,
            ))
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn emit_stream_line(
        &self,
        line: &[u8],
        sequence: &mut u64,
        terminal_seen: &mut bool,
        tool_call_ids: &mut HashSet<String>,
        sink: &dyn ModelStreamSink,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<bool> {
        let trimmed = trim_ascii(line);
        if trimmed.is_empty() {
            return Ok(false);
        }
        let payload = if self.runtime.provider.provider_type == ProviderType::Ollama {
            trimmed
        } else {
            let Some(payload) = trimmed.strip_prefix(b"data:") else {
                return Ok(false);
            };
            trim_ascii(payload)
        };
        if payload == b"[DONE]" {
            *terminal_seen = true;
            let chunk = ModelStreamChunk {
                sequence: *sequence,
                content_delta: String::new(),
                tool_calls: Vec::new(),
                done: true,
            };
            send_stream_chunk(sink, chunk, cancellation).await?;
            return Ok(true);
        }

        let value: Value = serde_json::from_slice(payload)
            .map_err(|_| invalid_schema_error("MODEL_PROVIDER_STREAM_SCHEMA_INVALID"))?;
        let (content_delta, tool_calls, done) =
            parse_stream_value(self.runtime.provider.provider_type, &value)?;
        for tool_call in &tool_calls {
            if !tool_call_ids.insert(tool_call.id.clone()) {
                return Err(invalid_schema_error("MODEL_PROVIDER_DUPLICATE_TOOL_CALL"));
            }
        }
        let chunk = ModelStreamChunk {
            sequence: *sequence,
            content_delta,
            tool_calls,
            done,
        };
        *sequence = sequence.saturating_add(1);
        send_stream_chunk(sink, chunk, cancellation).await?;
        if done {
            *terminal_seen = true;
        }
        Ok(done)
    }
}
