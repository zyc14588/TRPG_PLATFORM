#[tokio::test]
async fn status_errors_are_classified_and_generation_is_never_blindly_retried() {
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

    for (status, kind, code) in [
        (
            401,
            ModelProviderErrorKind::Authentication,
            "MODEL_PROVIDER_AUTHENTICATION_FAILED",
        ),
        (
            429,
            ModelProviderErrorKind::RateLimit,
            "MODEL_PROVIDER_RATE_LIMITED",
        ),
        (
            503,
            ModelProviderErrorKind::Transport,
            "MODEL_PROVIDER_UPSTREAM_FAILED",
        ),
    ] {
        let before = server.request_count("/chat/completions");
        server.set_behavior(MockBehavior::ChatStatus(status));
        let error = provider
            .chat(&plain_chat_request(), &ProviderCancellation::default())
            .await
            .unwrap_err();
        assert_eq!(error.kind(), kind);
        assert_eq!(error.code(), code);
        assert_eq!(error.upstream_status(), Some(status));
        assert!(!error.retryable());
        assert_eq!(server.request_count("/chat/completions"), before + 1);
    }
}

#[tokio::test]
async fn only_the_idempotent_capability_probe_is_retried() {
    let server = MockModelServer::spawn(ProviderType::LlamaCpp, "llama_cpp-model").await;
    server.set_behavior(MockBehavior::ProbeStatus(503));
    let provider = make_provider(
        ProviderType::LlamaCpp,
        &server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(2),
    );
    let error = provider
        .probe_capabilities(&ProviderCancellation::default())
        .await
        .unwrap_err();
    assert_eq!(error.code(), "MODEL_PROVIDER_UPSTREAM_FAILED");
    assert!(error.retryable());
    assert_eq!(server.request_count("/models"), 2);
}

#[tokio::test]
async fn insufficient_capability_stops_before_generation() {
    let server = MockModelServer::spawn(ProviderType::Ollama, "ollama-model").await;
    server.set_behavior(MockBehavior::CapabilitiesWithoutTools);
    let provider = make_provider(
        ProviderType::Ollama,
        &server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(2),
    );
    let error = provider
        .chat(&full_chat_request(), &ProviderCancellation::default())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ModelProviderErrorKind::Capability);
    assert_eq!(error.code(), "MODEL_PROVIDER_CAPABILITY_UNAVAILABLE");
    assert_eq!(server.request_count("/api/show"), 1);
    assert_eq!(server.request_count("/api/chat"), 0);
}

#[tokio::test]
async fn local_failure_does_not_contact_the_configured_cloud_provider() {
    let local_server = MockModelServer::spawn(ProviderType::Ollama, "ollama-model").await;
    let cloud_server = MockModelServer::spawn(ProviderType::Cloud, "cloud-model").await;
    let local = make_provider(
        ProviderType::Ollama,
        &local_server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(2),
    );
    let cloud = make_provider(
        ProviderType::Cloud,
        &cloud_server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(2),
    );
    let local_id = local.provider_id().clone();
    let router = ExplicitModelProviderRouter::new(vec![local, cloud]).unwrap();
    assert_eq!(router.provider_count(), 2);

    local_server.set_behavior(MockBehavior::ChatStatus(503));
    let error = router
        .chat(
            &local_id,
            &plain_chat_request(),
            &ProviderCancellation::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "MODEL_PROVIDER_UPSTREAM_FAILED");
    assert_eq!(local_server.request_count("/api/show"), 1);
    assert_eq!(local_server.request_count("/api/chat"), 1);
    assert_eq!(cloud_server.total_requests(), 0);
}

#[tokio::test]
async fn provider_debug_and_errors_never_expose_secret_or_private_context_canaries() {
    let server = MockModelServer::spawn(ProviderType::Cloud, "cloud-model").await;
    let provider = make_provider(
        ProviderType::Cloud,
        &server,
        ProviderCapabilities::v1_complete(),
        Duration::from_secs(2),
    );
    let request = full_chat_request();
    assert!(!format!("{request:?}").contains(PRIVATE_PROMPT_CANARY));
    assert!(!format!("{:?}", provider.startup_route_snapshot()).contains(API_KEY_CANARY));

    provider
        .probe_capabilities(&ProviderCancellation::default())
        .await
        .unwrap();
    server.set_behavior(MockBehavior::ChatStatus(503));
    let error = provider
        .chat(&request, &ProviderCancellation::default())
        .await
        .unwrap_err();
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains(API_KEY_CANARY));
    assert!(!rendered.contains(PRIVATE_PROMPT_CANARY));
}
