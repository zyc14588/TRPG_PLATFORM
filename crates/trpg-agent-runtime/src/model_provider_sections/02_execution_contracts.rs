#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelProviderErrorKind {
    Authentication,
    Capability,
    Timeout,
    InvalidSchema,
    RateLimit,
    Transport,
    Cancelled,
    Configuration,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ModelProviderError {
    kind: ModelProviderErrorKind,
    code: &'static str,
    retryable: bool,
    upstream_status: Option<u16>,
}

impl ModelProviderError {
    pub(crate) const fn new(
        kind: ModelProviderErrorKind,
        code: &'static str,
        retryable: bool,
        upstream_status: Option<u16>,
    ) -> Self {
        Self {
            kind,
            code,
            retryable,
            upstream_status,
        }
    }

    pub const fn kind(&self) -> ModelProviderErrorKind {
        self.kind
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    pub const fn upstream_status(&self) -> Option<u16> {
        self.upstream_status
    }
}

impl std::fmt::Debug for ModelProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelProviderError")
            .field("kind", &self.kind)
            .field("code", &self.code)
            .field("retryable", &self.retryable)
            .field("upstream_status", &self.upstream_status)
            .finish()
    }
}

impl std::fmt::Display for ModelProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for ModelProviderError {}

pub type ModelProviderResult<T> = Result<T, ModelProviderError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutedModelRouteSnapshot {
    pub route_authorization_event_id: trpg_shared_kernel::EntityId,
    pub provider_id: trpg_shared_kernel::EntityId,
    pub provider_type: ProviderType,
    pub model_id: String,
    pub operation: ModelOperation,
    pub fallback_policy: &'static str,
    pub privacy_boundary: &'static str,
}

pub struct ProviderExecution<T> {
    pub route: ExecutedModelRouteSnapshot,
    pub output: T,
}

impl<T> std::fmt::Debug for ProviderExecution<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderExecution")
            .field("route", &self.route)
            .field("output", &"[redacted provider output]")
            .finish()
    }
}

#[derive(Clone)]
pub struct ModelProviderRuntimeConfig {
    pub provider: ProviderConfig,
    pub declared_capabilities: ProviderCapabilities,
    pub route_authorization_event_id: trpg_shared_kernel::EntityId,
    pub request_timeout: Duration,
    /// Test-only DNS injection for development `.test` endpoints. Production
    /// construction rejects this field.
    pub development_connect_override: Option<SocketAddr>,
}

impl std::fmt::Debug for ModelProviderRuntimeConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelProviderRuntimeConfig")
            .field("provider", &self.provider)
            .field("declared_capabilities", &self.declared_capabilities)
            .field(
                "route_authorization_event_id",
                &self.route_authorization_event_id,
            )
            .field("request_timeout", &self.request_timeout)
            .field(
                "development_connect_override",
                &self
                    .development_connect_override
                    .map(|_| "[development override]"),
            )
            .finish()
    }
}

#[derive(Clone)]
pub struct ProviderCancellation {
    sender: Arc<watch::Sender<bool>>,
}

impl Default for ProviderCancellation {
    fn default() -> Self {
        let (sender, _receiver) = watch::channel(false);
        Self {
            sender: Arc::new(sender),
        }
    }
}

impl ProviderCancellation {
    pub fn cancel(&self) {
        self.sender.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        *self.sender.borrow()
    }

    pub async fn cancelled(&self) {
        let mut receiver = self.sender.subscribe();
        loop {
            if *receiver.borrow() {
                return;
            }
            if receiver.changed().await.is_err() {
                return;
            }
        }
    }
}

#[async_trait]
pub trait ModelStreamSink: Send + Sync {
    async fn send(&self, chunk: ModelStreamChunk) -> ModelProviderResult<()>;
}

#[async_trait]
pub trait ExecutableModelProvider: Send + Sync {
    fn provider_id(&self) -> &trpg_shared_kernel::EntityId;

    fn provider_type(&self) -> ProviderType;

    fn model_id(&self) -> &str;

    fn model_artifact_sha256(&self) -> &str;

    fn startup_route_snapshot(&self) -> ExecutedModelRouteSnapshot;

    async fn probe_capabilities(
        &self,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ProviderCapabilities>>;

    async fn chat(
        &self,
        request: &ModelChatRequest,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelChatResponse>>;

    async fn stream_chat(
        &self,
        request: &ModelChatRequest,
        sink: &dyn ModelStreamSink,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ExecutedModelRouteSnapshot>;

    async fn embed(
        &self,
        request: &ModelEmbeddingRequest,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelEmbeddingResponse>>;
}
