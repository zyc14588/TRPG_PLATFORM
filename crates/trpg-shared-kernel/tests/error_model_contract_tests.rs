use trpg_shared_kernel::error_model::{
    compose_error, describe_error, ErrorSafetyEvaluation, InternalErrorContext,
    TrustedErrorLogEntry, TrustedErrorLogSink,
};
use trpg_shared_kernel::TrpgError;
use trpg_shared_kernel::WireErrorCode;

#[test]
fn error_model_exposes_stable_error_codes() {
    let descriptor = describe_error(&TrpgError::InvalidEntityId);

    assert_eq!(descriptor.code, WireErrorCode::InvalidEntityId);
    assert!(!descriptor.retryable);
    assert_eq!(
        descriptor.safety,
        ErrorSafetyEvaluation::SafeForPublicResponse
    );
    assert_eq!(descriptor.http_status, 400);
}

#[test]
fn error_model_marks_concurrency_errors_retryable() {
    let descriptor = describe_error(&TrpgError::ExpectedVersionConflict {
        expected: 0,
        actual: 1,
    });

    assert_eq!(descriptor.code, WireErrorCode::ExpectedVersionConflict);
    assert!(descriptor.retryable);
}

#[test]
fn error_model_composes_fixture_error_descriptor() {
    let descriptor = compose_error(WireErrorCode::UnknownVisibilityLabel, false);

    assert_eq!(descriptor.code, WireErrorCode::UnknownVisibilityLabel);
    assert_eq!(descriptor.safety, ErrorSafetyEvaluation::NotEvaluated);
}

#[derive(Default)]
struct CapturingTrustedSink {
    fields: Option<[String; 5]>,
}

impl TrustedErrorLogSink for CapturingTrustedSink {
    fn record(&mut self, entry: TrustedErrorLogEntry<'_>) {
        self.fields = Some([
            entry.operation.to_owned(),
            entry.resource.to_owned(),
            entry.correlation_id.to_owned(),
            entry.trace_id.to_owned(),
            entry.root_cause.to_owned(),
        ]);
    }
}

#[test]
fn restricted_root_cause_is_absent_from_public_response_but_present_in_trusted_log() {
    let context = InternalErrorContext::new(
        "load_campaign",
        "campaign_42",
        "correlation_42",
        "trace_42",
        "postgres relation private_campaign_facts was unavailable",
    )
    .unwrap();
    let descriptor = describe_error(&TrpgError::PolicyUnavailable);
    assert_eq!(descriptor.safety, ErrorSafetyEvaluation::Restricted);

    let response = context.public_response(&descriptor);
    let response_json = serde_json::to_string(&response).unwrap();
    assert_eq!(response.code, "INTERNAL_ERROR");
    assert!(response_json.contains("correlation_42"));
    assert!(!response_json.contains("private_campaign_facts"));
    assert!(!response_json.contains("postgres"));
    assert!(!format!("{context:?}").contains("private_campaign_facts"));

    let mut sink = CapturingTrustedSink::default();
    context.record(&mut sink);
    let fields = sink.fields.unwrap();
    assert_eq!(fields[0], "load_campaign");
    assert_eq!(fields[1], "campaign_42");
    assert_eq!(fields[2], "correlation_42");
    assert_eq!(fields[3], "trace_42");
    assert!(fields[4].contains("private_campaign_facts"));
}

#[test]
fn unevaluated_error_never_exposes_the_requested_wire_code() {
    let context = InternalErrorContext::new(
        "unknown_operation",
        "unknown_resource",
        "correlation_unknown",
        "trace_unknown",
        "unclassified lower-level failure",
    )
    .unwrap();
    let descriptor = compose_error(WireErrorCode::UnknownVisibilityLabel, false);
    let response = context.public_response(&descriptor);

    assert_eq!(descriptor.safety, ErrorSafetyEvaluation::NotEvaluated);
    assert_eq!(response.code, "INTERNAL_ERROR");
}
