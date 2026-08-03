#[async_trait]
impl<R: SecretResolver + 'static> ExecutableModelProvider for HttpModelProvider<R> {
    fn provider_id(&self) -> &trpg_shared_kernel::EntityId {
        &self.runtime.provider.provider_id
    }

    fn provider_type(&self) -> ProviderType {
        self.runtime.provider.provider_type
    }

    fn model_id(&self) -> &str {
        &self.runtime.provider.model_id
    }

    fn model_artifact_sha256(&self) -> &str {
        &self.runtime.provider.model_artifact_sha256
    }

    fn provider_runtime_sha256(&self) -> String {
        resolve_provider_runtime_sha256(&self.runtime.provider).unwrap_or_default()
    }

    fn startup_route_snapshot(&self) -> ExecutedModelRouteSnapshot {
        self.route_snapshot(ModelOperation::CapabilityProbe)
    }

    async fn probe_capabilities(
        &self,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ProviderCapabilities>> {
        if let Some(capabilities) = *self.effective_capabilities.read().await {
            return Ok(ProviderExecution {
                route: self.route_snapshot(ModelOperation::CapabilityProbe),
                output: capabilities,
            });
        }

        let mut last_error = None;
        for attempt in 0..=1 {
            match self.probe_once(cancellation).await {
                Ok(capabilities) => {
                    *self.effective_capabilities.write().await = Some(capabilities);
                    return Ok(ProviderExecution {
                        route: self.route_snapshot(ModelOperation::CapabilityProbe),
                        output: capabilities,
                    });
                }
                Err(error) if attempt == 0 && error.retryable() => last_error = Some(error),
                Err(error) => return Err(error),
            }
        }
        Err(last_error.expect("probe retry captures an error"))
    }

    async fn chat(
        &self,
        request: &ModelChatRequest,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelChatResponse>> {
        validate_chat_request(request)?;
        let mut required = vec![RequiredProviderCapability::Chat];
        if request.structured_output.is_some() {
            required.push(RequiredProviderCapability::StructuredOutput);
        }
        if !request.tools.is_empty() {
            required.push(RequiredProviderCapability::ToolRequests);
        }
        self.require_capabilities(&required, cancellation).await?;

        let endpoint = self.endpoint(ModelOperation::Chat)?;
        let mut use_json_object_format = false;
        let response = loop {
            let request_builder = self
                .authorized_request(Method::POST, endpoint.clone())?
                .json(&self.chat_payload(request, false, use_json_object_format));
            match self
                .send(request_builder, ModelOperation::Chat, cancellation)
                .await
            {
                Ok(response) => break response,
                Err(error)
                    if !use_json_object_format
                        && request.structured_output.is_some()
                        && self.runtime.provider.provider_type == ProviderType::Cloud
                        && error.upstream_status() == Some(400) =>
                {
                    use_json_object_format = true;
                }
                Err(error) => return Err(error),
            }
        };
        let body = self
            .read_bounded(response, ModelOperation::Chat, cancellation)
            .await?;
        let output = parse_chat_response(
            self.runtime.provider.provider_type,
            request
                .structured_output
                .as_ref()
                .map(|structured| &structured.schema),
            &body,
        )?;
        Ok(ProviderExecution {
            route: self.route_snapshot(ModelOperation::Chat),
            output,
        })
    }

    async fn stream_chat(
        &self,
        request: &ModelChatRequest,
        sink: &dyn ModelStreamSink,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ExecutedModelRouteSnapshot> {
        validate_chat_request(request)?;
        let mut required = vec![
            RequiredProviderCapability::Chat,
            RequiredProviderCapability::Streaming,
        ];
        if request.structured_output.is_some() {
            required.push(RequiredProviderCapability::StructuredOutput);
        }
        if !request.tools.is_empty() {
            required.push(RequiredProviderCapability::ToolRequests);
        }
        self.require_capabilities(&required, cancellation).await?;

        let endpoint = self.endpoint(ModelOperation::StreamingChat)?;
        let request_builder = self
            .authorized_request(Method::POST, endpoint)?
            .json(&self.chat_payload(request, true, false));
        let response = self
            .send(request_builder, ModelOperation::StreamingChat, cancellation)
            .await?;
        self.parse_stream(response, sink, cancellation).await?;
        Ok(self.route_snapshot(ModelOperation::StreamingChat))
    }

    async fn embed(
        &self,
        request: &ModelEmbeddingRequest,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelEmbeddingResponse>> {
        validate_embedding_request(request)?;
        self.require_capabilities(&[RequiredProviderCapability::Embeddings], cancellation)
            .await?;
        let endpoint = self.endpoint(ModelOperation::Embedding)?;
        let payload = json!({
            "model": self.runtime.provider.model_id,
            "input": request.inputs,
        });
        let request_builder = self
            .authorized_request(Method::POST, endpoint)?
            .json(&payload);
        let response = self
            .send(request_builder, ModelOperation::Embedding, cancellation)
            .await?;
        let body = self
            .read_bounded(response, ModelOperation::Embedding, cancellation)
            .await?;
        let output = parse_embedding_response(self.runtime.provider.provider_type, &body)?;
        Ok(ProviderExecution {
            route: self.route_snapshot(ModelOperation::Embedding),
            output,
        })
    }
}

async fn send_stream_chunk(
    sink: &dyn ModelStreamSink,
    chunk: ModelStreamChunk,
    cancellation: &ProviderCancellation,
) -> ModelProviderResult<()> {
    tokio::select! {
        _ = cancellation.cancelled() => Err(cancelled_error()),
        result = sink.send(chunk) => result,
    }
}

fn validate_chat_request(request: &ModelChatRequest) -> ModelProviderResult<()> {
    if request.messages.is_empty()
        || request.messages.len() > MAX_MESSAGES
        || request.tools.len() > MAX_TOOLS
    {
        return Err(invalid_schema_error("MODEL_PROVIDER_REQUEST_INVALID"));
    }
    let context_bytes = request.messages.iter().try_fold(0_usize, |total, message| {
        if message.content.trim().is_empty() {
            return None;
        }
        total.checked_add(message.content.len())
    });
    if !matches!(context_bytes, Some(total) if total <= MAX_MODEL_CONTEXT_BYTES) {
        return Err(invalid_schema_error("MODEL_PROVIDER_REQUEST_INVALID"));
    }
    if request.tools.iter().any(|tool| {
        !valid_wire_name(&tool.name)
            || tool.description.trim().is_empty()
            || !tool.input_schema.is_object()
    }) {
        return Err(invalid_schema_error("MODEL_PROVIDER_TOOL_SCHEMA_INVALID"));
    }
    if request
        .structured_output
        .as_ref()
        .is_some_and(|structured| {
            !valid_wire_name(&structured.name) || !structured.schema.is_object()
        })
    {
        return Err(invalid_schema_error(
            "MODEL_PROVIDER_STRUCTURED_SCHEMA_INVALID",
        ));
    }
    Ok(())
}

fn validate_embedding_request(request: &ModelEmbeddingRequest) -> ModelProviderResult<()> {
    if request.inputs.is_empty() || request.inputs.len() > MAX_MESSAGES {
        return Err(invalid_schema_error("MODEL_PROVIDER_REQUEST_INVALID"));
    }
    let input_bytes = request.inputs.iter().try_fold(0_usize, |total, input| {
        if input.trim().is_empty() {
            return None;
        }
        total.checked_add(input.len())
    });
    if !matches!(input_bytes, Some(total) if total <= MAX_MODEL_CONTEXT_BYTES) {
        return Err(invalid_schema_error("MODEL_PROVIDER_REQUEST_INVALID"));
    }
    Ok(())
}

fn valid_wire_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
