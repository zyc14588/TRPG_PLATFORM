
fn deletion_status_response(job: DeletionJob) -> HttpResponse {
    HttpResponse::json(
        200,
        json!({
            "job_id": job.job_id,
            "status": deletion_job_status_name(job.status),
            "evidence_status": match job.evidence_status {
                trpg_security_governance::security_privacy::DeletionEvidenceStatus::Pending => {
                    "pending"
                }
                trpg_security_governance::security_privacy::DeletionEvidenceStatus::Confirmed => {
                    "confirmed"
                }
            },
            "canonical_event_sequence": job.canonical_event_sequence,
            "failure_code": job.failure_code,
            "targets": job.targets.into_iter().map(|target| json!({
                "target": target.target.as_str(),
                "status": deletion_target_status_name(target.status),
                "error_code": target.error_code,
            })).collect::<Vec<_>>(),
        }),
    )
}

fn deletion_job_status_name(status: DeletionJobStatus) -> &'static str {
    match status {
        DeletionJobStatus::Requested => "requested",
        DeletionJobStatus::BlockedLegalHold => "blocked_legal_hold",
        DeletionJobStatus::Running => "running",
        DeletionJobStatus::Verifying => "verifying",
        DeletionJobStatus::Completed => "completed",
        DeletionJobStatus::Failed => "failed",
    }
}

fn deletion_target_status_name(status: DeletionTargetStatus) -> &'static str {
    match status {
        DeletionTargetStatus::Pending => "pending",
        DeletionTargetStatus::Deleted => "deleted",
        DeletionTargetStatus::Verified => "verified",
        DeletionTargetStatus::Failed => "failed",
    }
}

fn parse_json<T: for<'de> Deserialize<'de>>(request: &HttpRequest) -> Result<T, HttpResponse> {
    if request.header("content-type") != Some("application/json") {
        return Err(HttpResponse::json(
            400,
            json!({"error": "JSON_CONTENT_TYPE_REQUIRED"}),
        ));
    }
    serde_json::from_slice(&request.body)
        .map_err(|_| HttpResponse::json(400, json!({"error": "INVALID_JSON_BODY"})))
}

fn bearer_token(request: &HttpRequest) -> Result<&str, HttpResponse> {
    request
        .header("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or_else(|| HttpResponse::json(401, json!({"error": "AUTHENTICATION_REQUIRED"})))
}

fn required_safe_header(
    request: &HttpRequest,
    name: &str,
    max_len: usize,
) -> Result<String, HttpResponse> {
    request
        .header(name)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= max_len
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
        .map(str::to_owned)
        .ok_or_else(|| HttpResponse::json(400, json!({"error": "INVALID_IDEMPOTENCY_KEY"})))
}

fn parse_campaign_role(value: &str) -> Result<CampaignRole, HttpResponse> {
    match value {
        "CAMPAIGN_OWNER" => Ok(CampaignRole::CampaignOwner),
        "HUMAN_KEEPER" => Ok(CampaignRole::HumanKeeper),
        "PLAYER" => Ok(CampaignRole::Player),
        "SPECTATOR" => Ok(CampaignRole::Spectator),
        _ => Err(HttpResponse::json(
            400,
            json!({"error": "INVALID_CAMPAIGN_ROLE"}),
        )),
    }
}

fn campaign_role_name(role: CampaignRole) -> &'static str {
    match role {
        CampaignRole::CampaignOwner => "CAMPAIGN_OWNER",
        CampaignRole::HumanKeeper => "HUMAN_KEEPER",
        CampaignRole::Player => "PLAYER",
        CampaignRole::Spectator => "SPECTATOR",
    }
}

fn identity_error(error: IdentityError) -> HttpResponse {
    auth_error(ApiAuthError::from(error))
}

fn auth_error(error: ApiAuthError) -> HttpResponse {
    HttpResponse::json(error.status, json!({"error": error.code}))
}

fn player_action_api_error(error: CoreApiError) -> HttpResponse {
    HttpResponse::json(error.status_code(), json!({"error": error.to_string()}))
}

fn internal_error() -> HttpResponse {
    kernel_error_response_without_request(
        &TrpgError::AuditIntegrityViolation,
        "api_internal",
        "api_application",
        "internal API state unavailable",
    )
}

struct ProductionTrustedErrorLogSink;

impl TrustedErrorLogSink for ProductionTrustedErrorLogSink {
    fn record(&mut self, entry: TrustedErrorLogEntry<'_>) {
        let record = json!({
            "level": "error",
            "classification": "trusted_internal",
            "operation": entry.operation,
            "resource": entry.resource,
            "correlation_id": entry.correlation_id,
            "trace_id": entry.trace_id,
            "root_cause": entry.root_cause,
        });
        eprintln!("{record}");
    }
}

fn kernel_error_response(
    request: &HttpRequest,
    error: &TrpgError,
    operation: &str,
    resource: &str,
    root_cause: &str,
) -> HttpResponse {
    let correlation_id = safe_request_context(request.header("x-correlation-id"), "correlation");
    let trace_id = safe_request_context(request.header("x-trace-id"), "trace");
    build_kernel_error_response(
        error,
        operation,
        resource,
        &correlation_id,
        &trace_id,
        root_cause,
    )
}

fn kernel_error_response_without_request(
    error: &TrpgError,
    operation: &str,
    resource: &str,
    root_cause: &str,
) -> HttpResponse {
    let correlation_id = safe_request_context(None, "correlation");
    let trace_id = safe_request_context(None, "trace");
    build_kernel_error_response(
        error,
        operation,
        resource,
        &correlation_id,
        &trace_id,
        root_cause,
    )
}

fn build_kernel_error_response(
    error: &TrpgError,
    operation: &str,
    resource: &str,
    correlation_id: &str,
    trace_id: &str,
    root_cause: &str,
) -> HttpResponse {
    let descriptor = describe_error(error);
    let context =
        InternalErrorContext::new(operation, resource, correlation_id, trace_id, root_cause)
            .expect("fixed production error context must be valid");
    context.record(&mut ProductionTrustedErrorLogSink);
    let response = context.public_response(&descriptor);
    HttpResponse::json(
        response.http_status,
        serde_json::to_value(response).expect("public error response must serialize"),
    )
}

fn safe_request_context(candidate: Option<&str>, prefix: &str) -> String {
    candidate
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos());
            format!("{prefix}_{}_{}", std::process::id(), now)
        })
}

fn now_unix_ms() -> Result<u64, HttpResponse> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| internal_error())?
        .as_millis();
    u64::try_from(millis).map_err(|_| internal_error())
}
