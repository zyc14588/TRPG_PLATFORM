pub struct HttpModelProvider<R> {
    runtime: ModelProviderRuntimeConfig,
    credential_manager: Arc<SecretManager<R>>,
    client: Client,
    effective_capabilities: RwLock<Option<ProviderCapabilities>>,
}

impl<R: SecretResolver + 'static> HttpModelProvider<R> {
    pub fn new(
        runtime: ModelProviderRuntimeConfig,
        credential_manager: Arc<SecretManager<R>>,
    ) -> ModelProviderResult<Self> {
        validate_provider_config(&runtime.provider).map_err(|_| configuration_error())?;
        if runtime.request_timeout < MIN_REQUEST_TIMEOUT
            || runtime.request_timeout > MAX_REQUEST_TIMEOUT
        {
            return Err(configuration_error());
        }
        if matches!(
            runtime.provider.provider_type,
            ProviderType::LocalOpenAiCompatible
        ) {
            return Err(ModelProviderError::new(
                ModelProviderErrorKind::Configuration,
                "MODEL_PROVIDER_TYPE_NOT_ENABLED",
                false,
                None,
            ));
        }

        let mut client = Client::builder()
            .timeout(runtime.request_timeout)
            .connect_timeout(runtime.request_timeout.min(Duration::from_secs(10)))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("trpg-agent-worker/model-provider");

        if runtime.provider.provider_type.is_local() {
            client = client.no_proxy();
        }
        if let Some(address) = runtime.development_connect_override {
            if runtime.provider.environment != crate::model_provider::Environment::Dev {
                return Err(configuration_error());
            }
            let endpoint =
                url::Url::parse(&runtime.provider.base_url).map_err(|_| configuration_error())?;
            let host = endpoint.host_str().ok_or_else(configuration_error)?;
            if !host.ends_with(".test") {
                return Err(configuration_error());
            }
            client = client.no_proxy().resolve(host, address);
        }

        let client = client.build().map_err(|_| configuration_error())?;
        Ok(Self {
            runtime,
            credential_manager,
            client,
            effective_capabilities: RwLock::new(None),
        })
    }

    pub fn declared_capabilities(&self) -> ProviderCapabilities {
        self.runtime.declared_capabilities
    }

    fn route_snapshot(&self, operation: ModelOperation) -> ExecutedModelRouteSnapshot {
        ExecutedModelRouteSnapshot {
            route_authorization_event_id: self.runtime.route_authorization_event_id.clone(),
            provider_id: self.runtime.provider.provider_id.clone(),
            provider_type: self.runtime.provider.provider_type,
            model_id: self.runtime.provider.model_id.clone(),
            operation,
            fallback_policy: "none_no_automatic_fallback",
            privacy_boundary: "explicit_route_authorization_event",
        }
    }

    fn endpoint(&self, operation: ModelOperation) -> ModelProviderResult<url::Url> {
        let mut base =
            url::Url::parse(&self.runtime.provider.base_url).map_err(|_| configuration_error())?;
        if !base.path().ends_with('/') {
            let mut path = base.path().to_owned();
            path.push('/');
            base.set_path(&path);
        }
        let route = match (self.runtime.provider.provider_type, operation) {
            (ProviderType::Ollama, ModelOperation::CapabilityProbe) => "api/show",
            (ProviderType::Ollama, ModelOperation::Chat | ModelOperation::StreamingChat) => {
                "api/chat"
            }
            (ProviderType::Ollama, ModelOperation::Embedding) => "api/embed",
            (_, ModelOperation::CapabilityProbe) => "models",
            (_, ModelOperation::Chat | ModelOperation::StreamingChat) => "chat/completions",
            (_, ModelOperation::Embedding) => "embeddings",
        };
        base.join(route).map_err(|_| configuration_error())
    }

    fn authorized_request(
        &self,
        method: Method,
        endpoint: url::Url,
    ) -> ModelProviderResult<RequestBuilder> {
        let request = self.client.request(method, endpoint);
        let credential = self
            .credential_manager
            .resolve(&self.runtime.provider.credential)
            .map_err(|_| {
                ModelProviderError::new(
                    ModelProviderErrorKind::Authentication,
                    "MODEL_PROVIDER_CREDENTIAL_UNAVAILABLE",
                    false,
                    None,
                )
            })?;
        let header = credential.expose_to(|secret| {
            let mut bearer = Zeroizing::new(Vec::with_capacity(7 + secret.len()));
            bearer.extend_from_slice(b"Bearer ");
            bearer.extend_from_slice(secret);
            HeaderValue::from_bytes(bearer.as_slice())
        });
        let mut header = header.map_err(|_| {
            ModelProviderError::new(
                ModelProviderErrorKind::Authentication,
                "MODEL_PROVIDER_CREDENTIAL_INVALID",
                false,
                None,
            )
        })?;
        header.set_sensitive(true);
        Ok(request.header(AUTHORIZATION, header))
    }

    async fn send(
        &self,
        request: RequestBuilder,
        operation: ModelOperation,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<Response> {
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        let result = tokio::select! {
            _ = cancellation.cancelled() => return Err(cancelled_error()),
            response = request.send() => response,
        };
        let response = result.map_err(|error| classify_reqwest_error(error, operation))?;
        if !response.status().is_success() {
            return Err(classify_status(response.status(), operation));
        }
        Ok(response)
    }

    async fn read_bounded(
        &self,
        mut response: Response,
        operation: ModelOperation,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<Vec<u8>> {
        let mut body = Vec::new();
        loop {
            let chunk = tokio::select! {
                _ = cancellation.cancelled() => return Err(cancelled_error()),
                chunk = response.chunk() => chunk,
            }
            .map_err(|error| classify_reqwest_error(error, operation))?;
            let Some(chunk) = chunk else {
                break;
            };
            if body.len().saturating_add(chunk.len()) > MAX_PROVIDER_RESPONSE_BYTES {
                return Err(invalid_schema_error("MODEL_PROVIDER_RESPONSE_TOO_LARGE"));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    async fn probe_once(
        &self,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderCapabilities> {
        let endpoint = self.endpoint(ModelOperation::CapabilityProbe)?;
        let request = if self.runtime.provider.provider_type == ProviderType::Ollama {
            self.authorized_request(Method::POST, endpoint)?
                .json(&json!({ "model": self.runtime.provider.model_id }))
        } else {
            self.authorized_request(Method::GET, endpoint)?
        };
        let response = self
            .send(request, ModelOperation::CapabilityProbe, cancellation)
            .await?;
        let body = self
            .read_bounded(response, ModelOperation::CapabilityProbe, cancellation)
            .await?;
        let value: Value = serde_json::from_slice(&body)
            .map_err(|_| invalid_schema_error("MODEL_PROVIDER_PROBE_SCHEMA_INVALID"))?;
        parse_probe_response(
            self.runtime.provider.provider_type,
            &self.runtime.provider.model_id,
            self.runtime.declared_capabilities,
            &value,
        )
    }
}
