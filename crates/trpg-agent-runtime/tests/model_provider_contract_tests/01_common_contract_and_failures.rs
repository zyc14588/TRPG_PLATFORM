#[derive(Default)]
struct CapturingSink {
    chunks: Mutex<Vec<ModelStreamChunk>>,
}

#[async_trait]
impl ModelStreamSink for CapturingSink {
    async fn send(
        &self,
        chunk: ModelStreamChunk,
    ) -> trpg_agent_runtime::model_provider::ModelProviderResult<()> {
        self.chunks.lock().unwrap().push(chunk);
        Ok(())
    }
}

async fn assert_common_provider_contract(provider_type: ProviderType) {
    let model_id = format!("{}-model", provider_type.route_name());
    let server = MockModelServer::spawn(provider_type, &model_id).await;
    let provider = make_provider(
        provider_type,
        &server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(2),
    );
    let cancellation = ProviderCancellation::default();

    let probe = provider.probe_capabilities(&cancellation).await.unwrap();
    assert_eq!(probe.output, ProviderCapabilities::v1_complete());
    assert_eq!(probe.route.operation, ModelOperation::CapabilityProbe);

    let chat = provider
        .chat(&full_chat_request(), &cancellation)
        .await
        .unwrap();
    assert_eq!(chat.route.operation, ModelOperation::Chat);
    assert_eq!(chat.route.fallback_policy, "none_no_automatic_fallback");
    assert_eq!(
        chat.output.structured_output,
        Some(serde_json::json!({"scene": "library"}))
    );
    assert_eq!(chat.output.tool_calls.len(), 1);
    assert_eq!(chat.output.tool_calls[0].name, "search_clue");

    let sink = CapturingSink::default();
    let stream_route = provider
        .stream_chat(
            &plain_chat_request(),
            &sink,
            &ProviderCancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(stream_route.operation, ModelOperation::StreamingChat);
    {
        let chunks = sink.chunks.lock().unwrap();
        assert!(chunks.len() >= 2);
        assert!(chunks.last().unwrap().done);
    }

    let embedding = provider
        .embed(
            &ModelEmbeddingRequest {
                inputs: vec!["public embedding input".to_owned()],
            },
            &ProviderCancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(embedding.route.operation, ModelOperation::Embedding);
    assert_eq!(embedding.output.embeddings, vec![vec![0.1, 0.2, 0.3]]);

    assert!(server.authorization_seen.load(Ordering::Relaxed));
    assert!(server
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|request| request.authorization_present));
    assert_eq!(server.chat_output_budgets(), vec![Some(256), Some(256)]);
    let expected_reasoning = if provider_type == ProviderType::Cloud {
        Some("none".to_owned())
    } else {
        None
    };
    assert_eq!(
        server.chat_reasoning_efforts(),
        vec![expected_reasoning.clone(), expected_reasoning]
    );
}

#[tokio::test]
async fn cloud_openai_compatible_provider_satisfies_the_common_contract() {
    assert_common_provider_contract(ProviderType::Cloud).await;
}

#[tokio::test]
async fn cloud_provider_retries_json_object_after_json_schema_is_rejected() {
    let server = MockModelServer::spawn(ProviderType::Cloud, "cloud-model").await;
    server.set_behavior(MockBehavior::ChatRejectJsonSchema);
    let provider = make_provider(
        ProviderType::Cloud,
        &server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(2),
    );

    let response = provider
        .chat(&full_chat_request(), &ProviderCancellation::default())
        .await
        .expect("cloud JSON-object compatibility retry");

    assert_eq!(
        response.output.structured_output,
        Some(serde_json::json!({"scene": "library"}))
    );
    assert_eq!(
        server.chat_structured_output_formats(),
        vec![
            Some("json_schema".to_owned()),
            Some("json_object".to_owned())
        ]
    );
}

#[tokio::test]
async fn cloud_json_object_compatibility_response_remains_schema_validated() {
    let server = MockModelServer::spawn(ProviderType::Cloud, "cloud-model").await;
    server.set_behavior(MockBehavior::ChatRejectJsonSchemaWithInvalidFallback);
    let provider = make_provider(
        ProviderType::Cloud,
        &server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(2),
    );

    let error = provider
        .chat(&full_chat_request(), &ProviderCancellation::default())
        .await
        .expect_err("schema-invalid JSON-object fallback must fail closed");

    assert_eq!(error.kind(), ModelProviderErrorKind::InvalidSchema);
    assert_eq!(error.code(), "MODEL_PROVIDER_STRUCTURED_OUTPUT_INVALID");
    assert_eq!(
        server.chat_structured_output_formats(),
        vec![
            Some("json_schema".to_owned()),
            Some("json_object".to_owned())
        ]
    );
}

#[tokio::test]
async fn ollama_provider_satisfies_the_common_contract() {
    assert_common_provider_contract(ProviderType::Ollama).await;
}

#[tokio::test]
async fn ollama_maps_only_an_explicit_no_think_directive_to_the_native_option() {
    let server = MockModelServer::spawn(ProviderType::Ollama, "ollama-model").await;
    let provider = make_provider(
        ProviderType::Ollama,
        &server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(1),
    );

    provider
        .chat(&full_chat_request(), &ProviderCancellation::default())
        .await
        .unwrap();
    let mut certification_request = full_chat_request();
    certification_request.messages[0].content =
        "/no_think\ncertification_case:latency".to_owned();
    provider
        .chat(
            &certification_request,
            &ProviderCancellation::default(),
        )
        .await
        .unwrap();

    assert_eq!(server.chat_thinking_modes(), vec![None, Some(false)]);
}

#[tokio::test]
async fn llama_cpp_provider_satisfies_the_common_contract() {
    assert_common_provider_contract(ProviderType::LlamaCpp).await;
}

#[tokio::test]
async fn invalid_json_and_duplicate_tool_calls_are_fail_closed() {
    let server = MockModelServer::spawn(ProviderType::LlamaCpp, "llama_cpp-model").await;
    let provider = make_provider(
        ProviderType::LlamaCpp,
        &server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(2),
    );
    provider
        .probe_capabilities(&ProviderCancellation::default())
        .await
        .unwrap();

    server.set_behavior(MockBehavior::ChatInvalidJson);
    let error = provider
        .chat(&plain_chat_request(), &ProviderCancellation::default())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ModelProviderErrorKind::InvalidSchema);
    assert_eq!(error.code(), "MODEL_PROVIDER_RESPONSE_SCHEMA_INVALID");

    server.set_behavior(MockBehavior::ChatDuplicateToolCall);
    let error = provider
        .chat(&full_chat_request(), &ProviderCancellation::default())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ModelProviderErrorKind::InvalidSchema);
    assert_eq!(error.code(), "MODEL_PROVIDER_DUPLICATE_TOOL_CALL");
}

#[tokio::test]
async fn timeout_cancellation_and_disconnect_have_stable_errors() {
    let timeout_server = MockModelServer::spawn(ProviderType::LlamaCpp, "llama_cpp-model").await;
    let timeout_provider = make_provider(
        ProviderType::LlamaCpp,
        &timeout_server,
        ProviderCapabilities::v1_complete(),
        Duration::from_millis(50),
    );
    timeout_provider
        .probe_capabilities(&ProviderCancellation::default())
        .await
        .unwrap();
    timeout_server.set_behavior(MockBehavior::ChatDelay(250));
    let error = timeout_provider
        .chat(&plain_chat_request(), &ProviderCancellation::default())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ModelProviderErrorKind::Timeout);
    assert_eq!(error.code(), "MODEL_PROVIDER_TIMEOUT");

    let cancellation_server = MockModelServer::spawn(ProviderType::Ollama, "ollama-model").await;
    let cancellation_provider = make_provider(
        ProviderType::Ollama,
        &cancellation_server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(2),
    );
    cancellation_provider
        .probe_capabilities(&ProviderCancellation::default())
        .await
        .unwrap();
    cancellation_server.set_behavior(MockBehavior::ChatDelay(500));
    let cancellation = ProviderCancellation::default();
    let cancel_signal = cancellation.clone();
    let cancel = async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        cancel_signal.cancel();
    };
    let cancellation_request = plain_chat_request();
    let (result, ()) = tokio::join!(
        cancellation_provider.chat(&cancellation_request, &cancellation),
        cancel
    );
    let error = result.unwrap_err();
    assert_eq!(error.kind(), ModelProviderErrorKind::Cancelled);
    assert_eq!(error.code(), "MODEL_PROVIDER_CANCELLED");

    let disconnect_server = MockModelServer::spawn(ProviderType::Cloud, "cloud-model").await;
    let disconnect_provider = make_provider(
        ProviderType::Cloud,
        &disconnect_server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(2),
    );
    disconnect_provider
        .probe_capabilities(&ProviderCancellation::default())
        .await
        .unwrap();
    disconnect_server.set_behavior(MockBehavior::StreamDisconnect);
    let error = disconnect_provider
        .stream_chat(
            &plain_chat_request(),
            &CapturingSink::default(),
            &ProviderCancellation::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ModelProviderErrorKind::Transport);
    assert_eq!(error.code(), "MODEL_PROVIDER_STREAM_DISCONNECTED");
}
