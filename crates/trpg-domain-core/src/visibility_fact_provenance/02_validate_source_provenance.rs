
fn validate_source_provenance(source: FactSource, kind: &ProvenanceKind) -> DomainResult<()> {
    if !source.can_be_confirmed()
        || matches!(
            kind,
            ProvenanceKind::UserStatement
                | ProvenanceKind::AgentProposal
                | ProvenanceKind::ImportedSource
                | ProvenanceKind::SystemFixture
        )
    {
        return Err(DomainError::InvalidConfirmedFactSource);
    }

    let kind_matches_source = match source {
        FactSource::DiceRoll => matches!(
            kind,
            ProvenanceKind::RulesEngineDecision | ProvenanceKind::ToolResult
        ),
        FactSource::GameEvent
        | FactSource::DecisionRecord
        | FactSource::CharacterSheetVersion
        | FactSource::ClueRevealEvent => matches!(
            kind,
            ProvenanceKind::HumanKeeperStatement
                | ProvenanceKind::RulesEngineDecision
                | ProvenanceKind::ToolResult
        ),
        FactSource::AgentDraft | FactSource::NpcClaim | FactSource::PlayerInference => false,
    };

    if kind_matches_source {
        Ok(())
    } else {
        Err(DomainError::CommittedFactEvidenceInvalid)
    }
}

pub const fn expected_source_event_type(source: FactSource) -> &'static str {
    match source {
        FactSource::GameEvent => "GameEventRecorded",
        FactSource::DecisionRecord => "DecisionCommitted",
        FactSource::DiceRoll => "DiceRolled",
        FactSource::CharacterSheetVersion => "CharacterSheetVersionRecorded",
        FactSource::ClueRevealEvent => "ClueRevealed",
        FactSource::AgentDraft => "AgentDrafted",
        FactSource::NpcClaim => "NpcClaimed",
        FactSource::PlayerInference => "PlayerInferenceRecorded",
    }
}

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
    fact_id: EntityId,
    source: FactSource,
    visibility: Visibility,
    fact_provenance: FactProvenance,
    source_event_sequence: u64,
}

impl ConfirmedFact {
    pub fn fact_id(&self) -> &EntityId {
        &self.fact_id
    }

    pub const fn source(&self) -> FactSource {
        self.source
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub fn fact_provenance(&self) -> &FactProvenance {
        &self.fact_provenance
    }

    pub const fn source_event_sequence(&self) -> u64 {
        self.source_event_sequence
    }
}

pub fn most_restrictive_label(labels: &[VisibilityLabel]) -> Option<VisibilityLabel> {
    labels
        .iter()
        .cloned()
        .reduce(|current, candidate| current.conservative_merge(&candidate))
}

/// Selects a derived audience without discarding targeted subject identity.
/// Private scopes for different subjects (or different target kinds) are
/// incomparable, so derivation fails closed rather than choosing whichever
/// equal-rank label happened to appear last.
pub fn most_restrictive_visibility(
    visibilities: &[Visibility],
) -> DomainResult<Option<Visibility>> {
    let mut selected: Option<Visibility> = None;
    for visibility in visibilities {
        if !visibility.is_well_formed() {
            return Err(crate::ddd::DomainError::VisibilityDenied);
        }
        selected = Some(match selected {
            None => visibility.clone(),
            Some(current) => restrict_visibility_pair(&current, visibility)?,
        });
    }
    Ok(selected)
}

pub fn redaction_for(
    visibility: &Visibility,
    derived_object: DerivedObject,
    processor: &PrincipalScope,
    target_audience: &PrincipalScope,
) -> RedactionOutcome {
    if !visibility.can_view(processor) {
        return match derived_object {
            DerivedObject::AgentContextForPlayer => RedactionOutcome::Omitted,
            _ => RedactionOutcome::Redacted,
        };
    }
    if visibility.label().kind() == VisibilityKind::AiInternal {
        return match derived_object {
            DerivedObject::AnyPlayerOrKeeperExport => RedactionOutcome::RedactedOrAuditOnly,
            DerivedObject::AgentContextForPlayer => RedactionOutcome::Omitted,
            _ if visibility.can_view(target_audience) => RedactionOutcome::Visible,
            _ => RedactionOutcome::Redacted,
        };
    }

    if visibility.can_view(target_audience) {
        return RedactionOutcome::Visible;
    }

    match derived_object {
        DerivedObject::AgentContextForPlayer => RedactionOutcome::Omitted,
        _ => RedactionOutcome::Redacted,
    }
}

pub fn promote_fact_to_confirmed(
    fact_id: impl Into<String>,
    evidence: &CommittedFactEvidence,
) -> DomainResult<ConfirmedFact> {
    let fact_id = EntityId::new(fact_id)?;
    if &fact_id != evidence.target_fact_id() {
        return Err(DomainError::CommittedFactEvidenceInvalid);
    }
    Ok(ConfirmedFact {
        fact_id,
        source: evidence.source(),
        visibility: evidence.visibility().clone(),
        fact_provenance: evidence.fact_provenance().clone(),
        source_event_sequence: evidence.event_sequence(),
    })
}

fn restrict_visibility_pair(
    current: &Visibility,
    candidate: &Visibility,
) -> DomainResult<Visibility> {
    Ok(current.intersection(candidate))
}
