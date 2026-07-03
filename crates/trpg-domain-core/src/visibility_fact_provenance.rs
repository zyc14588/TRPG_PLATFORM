use crate::ddd::{
    require_confirmable_fact_source, DomainResult, EntityId, FactProvenance, FactSource,
    PrincipalScope, Visibility, VisibilityLabel,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivedObject {
    PlayerExport,
    SessionSummaryParty,
    AnyPlayerOrKeeperExport,
    AgentContextResult,
    AgentContextForPlayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedactionOutcome {
    Visible,
    Redacted,
    Omitted,
    RedactedOrAuditOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfirmedFact {
    pub fact_id: EntityId,
    pub source: FactSource,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
}

pub fn most_restrictive_label(labels: &[VisibilityLabel]) -> Option<VisibilityLabel> {
    labels.iter().cloned().max_by_key(label_rank)
}

pub fn redaction_for(
    visibility: &Visibility,
    derived_object: DerivedObject,
    principal: &PrincipalScope,
) -> RedactionOutcome {
    if matches!(visibility.label(), VisibilityLabel::AiInternal) {
        return match derived_object {
            DerivedObject::AnyPlayerOrKeeperExport => RedactionOutcome::RedactedOrAuditOnly,
            DerivedObject::AgentContextForPlayer => RedactionOutcome::Omitted,
            _ if visibility.can_view(principal) => RedactionOutcome::Visible,
            _ => RedactionOutcome::Redacted,
        };
    }

    if visibility.can_view(principal) {
        return RedactionOutcome::Visible;
    }

    match derived_object {
        DerivedObject::AgentContextForPlayer => RedactionOutcome::Omitted,
        _ => RedactionOutcome::Redacted,
    }
}

pub fn promote_fact_to_confirmed(
    fact_id: impl Into<String>,
    source: FactSource,
    visibility: Visibility,
    fact_provenance: FactProvenance,
) -> DomainResult<ConfirmedFact> {
    require_confirmable_fact_source(source)?;

    Ok(ConfirmedFact {
        fact_id: EntityId::new(fact_id)?,
        source,
        visibility,
        fact_provenance,
    })
}

fn label_rank(label: &VisibilityLabel) -> u8 {
    match label {
        VisibilityLabel::Public => 0,
        VisibilityLabel::PartyVisible => 1,
        VisibilityLabel::PrivateToPlayer | VisibilityLabel::InvestigatorPrivate => 2,
        VisibilityLabel::KeeperOnly => 3,
        VisibilityLabel::AiInternal => 4,
        VisibilityLabel::SystemOnly | VisibilityLabel::SystemPrivate => 5,
    }
}
