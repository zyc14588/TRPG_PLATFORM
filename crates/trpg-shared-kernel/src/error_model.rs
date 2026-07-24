use std::fmt;

use crate::{shared_kernel::TrpgError, WireErrorCode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorSafetyEvaluation {
    SafeForPublicResponse,
    Restricted,
    NotEvaluated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorDescriptor {
    pub code: WireErrorCode,
    pub http_status: u16,
    pub retryable: bool,
    pub safety: ErrorSafetyEvaluation,
}

/// Converts a typed kernel error into an explicitly classified descriptor.
/// The classification is exhaustive: newly added `TrpgError` variants must be
/// consciously assigned rather than inheriting a permissive default.
pub fn describe_error(error: &TrpgError) -> ErrorDescriptor {
    let safety = match error {
        TrpgError::InvalidConfiguration(_)
        | TrpgError::DependencyViolation(_)
        | TrpgError::CrateOwnershipViolation(_)
        | TrpgError::WorkspaceViolation(_)
        | TrpgError::CodingPolicyViolation(_)
        | TrpgError::OpenSourceReferenceViolation(_)
        | TrpgError::InternalIdentityInvalid
        | TrpgError::PolicyUnavailable
        | TrpgError::PolicyEvidenceUntrusted
        | TrpgError::AuditIntegrityViolation => ErrorSafetyEvaluation::Restricted,
        TrpgError::InvalidEntityId
        | TrpgError::UnknownVisibilityLabel
        | TrpgError::MissingIdempotencyKey
        | TrpgError::MissingCorrelationId
        | TrpgError::MissingCausationId
        | TrpgError::MissingFactProvenance
        | TrpgError::AuthorityViolation
        | TrpgError::AuthorityContractMutation
        | TrpgError::DirectAgentStateWrite
        | TrpgError::PolicyDenied
        | TrpgError::ExpectedVersionConflict { .. }
        | TrpgError::DuplicateCommand
        | TrpgError::VisibilityDenied
        | TrpgError::EventContractUnknown
        | TrpgError::EventContractVersionMismatch
        | TrpgError::AuthenticationRequired
        | TrpgError::AuthorizationDenied
        | TrpgError::CampaignScopeMismatch
        | TrpgError::AuthorityOwnerMismatch
        | TrpgError::AuthorityContractVersionConflict
        | TrpgError::DecisionConfirmationRequired
        | TrpgError::DecisionDraftChanged
        | TrpgError::DecisionExpired
        | TrpgError::DecisionAlreadyCommitted => ErrorSafetyEvaluation::SafeForPublicResponse,
    };
    ErrorDescriptor {
        code: error.wire_code(),
        http_status: error.http_status(),
        retryable: error.retryable(),
        safety,
    }
}

/// A descriptor assembled from a wire code has no evidence that accompanying
/// context is safe. It therefore remains `NotEvaluated` until a typed error or
/// a dedicated policy performs the classification.
pub fn compose_error(code: WireErrorCode, retryable: bool) -> ErrorDescriptor {
    ErrorDescriptor {
        code,
        http_status: 500,
        retryable,
        safety: ErrorSafetyEvaluation::NotEvaluated,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PublicErrorResponse {
    pub code: &'static str,
    pub http_status: u16,
    pub retryable: bool,
    pub correlation_id: String,
}

/// Error context is intentionally not Clone and its Debug implementation
/// redacts the root cause. The complete cause can only cross the
/// `TrustedErrorLogSink` boundary.
pub struct InternalErrorContext {
    operation: String,
    resource: String,
    correlation_id: String,
    trace_id: String,
    root_cause: String,
}

impl fmt::Debug for InternalErrorContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InternalErrorContext")
            .field("operation", &self.operation)
            .field("resource", &self.resource)
            .field("correlation_id", &self.correlation_id)
            .field("trace_id", &self.trace_id)
            .field("root_cause", &"<restricted>")
            .finish()
    }
}

impl InternalErrorContext {
    pub fn new(
        operation: impl Into<String>,
        resource: impl Into<String>,
        correlation_id: impl Into<String>,
        trace_id: impl Into<String>,
        root_cause: impl Into<String>,
    ) -> Result<Self, TrpgError> {
        let operation = validated_context_field(operation.into(), 128)?;
        let resource = validated_context_field(resource.into(), 256)?;
        let correlation_id = validated_context_field(correlation_id.into(), 128)?;
        let trace_id = validated_context_field(trace_id.into(), 128)?;
        let root_cause = validated_context_field(root_cause.into(), 4_096)?;
        Ok(Self {
            operation,
            resource,
            correlation_id,
            trace_id,
            root_cause,
        })
    }

    pub fn public_response(&self, descriptor: &ErrorDescriptor) -> PublicErrorResponse {
        let (code, http_status, retryable) = match descriptor.safety {
            ErrorSafetyEvaluation::SafeForPublicResponse => (
                descriptor.code.as_str(),
                descriptor.http_status,
                descriptor.retryable,
            ),
            ErrorSafetyEvaluation::Restricted | ErrorSafetyEvaluation::NotEvaluated => (
                "INTERNAL_ERROR",
                descriptor.http_status,
                descriptor.retryable,
            ),
        };
        PublicErrorResponse {
            code,
            http_status,
            retryable,
            correlation_id: self.correlation_id.clone(),
        }
    }

    pub fn record(&self, sink: &mut impl TrustedErrorLogSink) {
        sink.record(TrustedErrorLogEntry {
            operation: &self.operation,
            resource: &self.resource,
            correlation_id: &self.correlation_id,
            trace_id: &self.trace_id,
            root_cause: &self.root_cause,
        });
    }
}

pub struct TrustedErrorLogEntry<'a> {
    pub operation: &'a str,
    pub resource: &'a str,
    pub correlation_id: &'a str,
    pub trace_id: &'a str,
    pub root_cause: &'a str,
}

pub trait TrustedErrorLogSink {
    fn record(&mut self, entry: TrustedErrorLogEntry<'_>);
}

fn validated_context_field(value: String, maximum_length: usize) -> Result<String, TrpgError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > maximum_length || trimmed.chars().any(char::is_control)
    {
        return Err(TrpgError::InvalidConfiguration("invalid_error_context"));
    }
    Ok(trimmed.to_owned())
}
