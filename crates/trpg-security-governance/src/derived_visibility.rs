use trpg_shared_kernel::{PrincipalScope, Visibility, VisibilityKind, VisibilityLabel};

use crate::{DerivedObject, RedactionOutcome};

/// Complete policy input for a derived value. `processor` answers whether the
/// workload may read the sources; `target_audience` independently answers who
/// may receive the derived value. Keeping those identities separate prevents
/// a privileged background worker from widening a player-facing result.
#[derive(Clone, Debug)]
pub struct DerivationRequest<'a> {
    pub sources: &'a [Visibility],
    pub processor: &'a PrincipalScope,
    pub target_audience: &'a PrincipalScope,
    pub target: DerivedObject,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedVisibilityDecision {
    pub outcome: RedactionOutcome,
    pub result_visibility: Visibility,
    pub error_code: Option<&'static str>,
}

/// Evaluates source access and target access independently and computes the
/// result classification as the audience intersection of every source.
pub fn evaluate_derived_visibility(request: DerivationRequest<'_>) -> DerivedVisibilityDecision {
    let result_visibility = request
        .sources
        .iter()
        .cloned()
        .reduce(|current, source| current.intersection(&source))
        .unwrap_or_else(|| Visibility::new(VisibilityLabel::Public));

    if request.sources.is_empty() {
        return denied(
            request.target,
            result_visibility,
            "DERIVATION_SOURCE_REQUIRED",
        );
    }

    if request
        .sources
        .iter()
        .any(|source| !source.is_well_formed() || !source.can_view(request.processor))
    {
        return DerivedVisibilityDecision {
            outcome: RedactionOutcome::Omitted,
            result_visibility,
            error_code: Some("DERIVATION_PROCESSOR_NOT_AUTHORIZED"),
        };
    }

    let export_forbidden = request.sources.iter().any(|source| {
        matches!(
            source.label().kind(),
            VisibilityKind::AiInternal | VisibilityKind::SystemOnly | VisibilityKind::SystemPrivate
        )
    }) && matches!(
        request.target,
        DerivedObject::PlayerExport | DerivedObject::PartySummary | DerivedObject::RagChunk
    );
    if export_forbidden {
        let error_code = if request.target == DerivedObject::PlayerExport
            && request
                .sources
                .iter()
                .any(|source| source.label().kind() == VisibilityKind::AiInternal)
        {
            "AI_INTERNAL_EXPORT_FORBIDDEN"
        } else {
            "RESTRICTED_EXPORT_FORBIDDEN"
        };
        return denied(request.target, result_visibility, error_code);
    }

    // Debug logs carry policy metadata only. Restricted source text is never
    // made log-visible even to a privileged processor.
    if request.target == DerivedObject::DebugLog
        && request
            .sources
            .iter()
            .any(|source| source.label().is_restricted())
    {
        return DerivedVisibilityDecision {
            outcome: RedactionOutcome::Redacted,
            result_visibility,
            error_code: Some("RESTRICTED_LOG_CONTENT_REDACTED"),
        };
    }

    if request
        .sources
        .iter()
        .all(|source| source.can_view(request.target_audience))
    {
        return DerivedVisibilityDecision {
            outcome: RedactionOutcome::Visible,
            result_visibility,
            error_code: None,
        };
    }

    let error_code = if request.target == DerivedObject::PlayerExport
        && request
            .sources
            .iter()
            .any(|source| source.label().kind() == VisibilityKind::KeeperOnly)
    {
        "VISIBILITY_DOWNGRADE_FORBIDDEN"
    } else if request.target == DerivedObject::PartySummary
        && request.sources.iter().any(|source| {
            matches!(
                source.label().kind(),
                VisibilityKind::PrivateToPlayer
                    | VisibilityKind::PrivateToGroup
                    | VisibilityKind::InvestigatorPrivate
            )
        })
    {
        "VISIBILITY_SCOPE_VIOLATION"
    } else {
        "VISIBILITY_LEAKAGE_DETECTED"
    };

    denied(request.target, result_visibility, error_code)
}

fn denied(
    target: DerivedObject,
    result_visibility: Visibility,
    error_code: &'static str,
) -> DerivedVisibilityDecision {
    let outcome = if matches!(
        target,
        DerivedObject::RagChunk | DerivedObject::AgentContext
    ) {
        RedactionOutcome::Omitted
    } else {
        RedactionOutcome::Redacted
    };
    DerivedVisibilityDecision {
        outcome,
        result_visibility,
        error_code: Some(error_code),
    }
}
