async fn assert_negative_provider_contracts(
    provider_type: ProviderType,
    definition: ProviderDefinition<'_>,
    credential: &[u8],
    root_certificate: Option<&[u8]>,
    wrong_root_certificate: Option<&[u8]>,
    wrong_host_url: Option<&str>,
    policy: &LocalProviderNetworkPolicy,
) {
    let cancellation = ProviderCancellation::default();
    if provider_type.is_local() {
        let wrong_ca_provider = provider(
            provider_type,
            ProviderDefinition {
                id: "real-provider-wrong-ca",
                ..definition
            },
            credential,
            wrong_root_certificate,
            policy,
        )
        .expect("wrong CA is syntactically valid");
        assert_eq!(
            wrong_ca_provider
                .probe_capabilities(&cancellation)
                .await
                .expect_err("wrong CA must fail TLS")
                .kind(),
            ModelProviderErrorKind::Transport
        );

        let wrong_host_provider = provider(
            provider_type,
            ProviderDefinition {
                id: "real-provider-wrong-host",
                base_url: wrong_host_url.expect("local provider wrong-host URL"),
                ..definition
            },
            credential,
            root_certificate,
            policy,
        )
        .expect("wrong hostname URL is otherwise valid");
        assert_eq!(
            wrong_host_provider
                .probe_capabilities(&cancellation)
                .await
                .expect_err("certificate hostname mismatch must fail TLS")
                .kind(),
            ModelProviderErrorKind::Transport
        );
    }

    let wrong_credential_provider = provider(
        provider_type,
        ProviderDefinition {
            id: "real-provider-wrong-credential",
            ..definition
        },
        b"deliberately-wrong-provider-credential",
        root_certificate,
        policy,
    )
    .expect("wrong credential provider construction");
    assert_eq!(
        wrong_credential_provider
            .probe_capabilities(&cancellation)
            .await
            .expect_err("wrong credential must fail authentication")
            .kind(),
        ModelProviderErrorKind::Authentication
    );

    let mut plaintext_url = url::Url::parse(definition.base_url).expect("provider URL");
    plaintext_url.set_scheme("http").expect("HTTP scheme");
    assert_eq!(
        provider(
            provider_type,
            ProviderDefinition {
                id: "real-provider-plaintext",
                base_url: plaintext_url.as_str(),
                ..definition
            },
            credential,
            root_certificate,
            policy,
        )
        .err()
        .expect("production plaintext transport must fail")
        .kind(),
        ModelProviderErrorKind::Configuration
    );

    if provider_type.is_local() {
        assert_eq!(
            provider(
                provider_type,
                ProviderDefinition {
                    id: "real-provider-unlisted",
                    base_url: "https://unlisted-provider:9443",
                    ..definition
                },
                credential,
                root_certificate,
                policy,
            )
            .err()
            .expect("unlisted private service name must fail")
            .kind(),
            ModelProviderErrorKind::Configuration
        );
    }
}
