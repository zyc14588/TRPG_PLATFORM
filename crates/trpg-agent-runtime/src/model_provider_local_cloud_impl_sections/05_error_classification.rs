fn classify_reqwest_error(error: reqwest::Error, operation: ModelOperation) -> ModelProviderError {
    let safe_retry = operation == ModelOperation::CapabilityProbe;
    if error.is_timeout() {
        ModelProviderError::new(
            ModelProviderErrorKind::Timeout,
            "MODEL_PROVIDER_TIMEOUT",
            safe_retry,
            None,
        )
    } else {
        ModelProviderError::new(
            ModelProviderErrorKind::Transport,
            "MODEL_PROVIDER_TRANSPORT_FAILED",
            safe_retry,
            None,
        )
    }
}

fn classify_status(status: StatusCode, operation: ModelOperation) -> ModelProviderError {
    let safe_retry = operation == ModelOperation::CapabilityProbe;
    match status.as_u16() {
        401 | 403 => ModelProviderError::new(
            ModelProviderErrorKind::Authentication,
            "MODEL_PROVIDER_AUTHENTICATION_FAILED",
            false,
            Some(status.as_u16()),
        ),
        429 => ModelProviderError::new(
            ModelProviderErrorKind::RateLimit,
            "MODEL_PROVIDER_RATE_LIMITED",
            safe_retry,
            Some(status.as_u16()),
        ),
        500..=599 => ModelProviderError::new(
            ModelProviderErrorKind::Transport,
            "MODEL_PROVIDER_UPSTREAM_FAILED",
            safe_retry,
            Some(status.as_u16()),
        ),
        404 if operation == ModelOperation::CapabilityProbe => ModelProviderError::new(
            ModelProviderErrorKind::Capability,
            "MODEL_PROVIDER_CAPABILITY_PROBE_UNAVAILABLE",
            false,
            Some(status.as_u16()),
        ),
        _ => ModelProviderError::new(
            ModelProviderErrorKind::InvalidSchema,
            "MODEL_PROVIDER_REQUEST_REJECTED",
            false,
            Some(status.as_u16()),
        ),
    }
}

fn configuration_error() -> ModelProviderError {
    ModelProviderError::new(
        ModelProviderErrorKind::Configuration,
        "MODEL_PROVIDER_CONFIGURATION_INVALID",
        false,
        None,
    )
}

fn capability_error() -> ModelProviderError {
    ModelProviderError::new(
        ModelProviderErrorKind::Capability,
        "MODEL_PROVIDER_CAPABILITY_UNAVAILABLE",
        false,
        None,
    )
}

fn invalid_schema_error(code: &'static str) -> ModelProviderError {
    ModelProviderError::new(ModelProviderErrorKind::InvalidSchema, code, false, None)
}

fn cancelled_error() -> ModelProviderError {
    ModelProviderError::new(
        ModelProviderErrorKind::Cancelled,
        "MODEL_PROVIDER_CANCELLED",
        false,
        None,
    )
}
