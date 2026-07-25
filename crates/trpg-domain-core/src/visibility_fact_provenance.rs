use crate::command_cqrs::{CommandAcceptedPayload, DomainCommandKind};
use crate::ddd::{
    DomainError, DomainResult, EntityId, EventStore, FactProvenance, FactSource, PrincipalScope,
    ProvenanceKind, Visibility, VisibilityKind, VisibilityLabel,
};
#[cfg(feature = "canonical-store-internal")]
use hmac::{Hmac, Mac};
#[cfg(feature = "canonical-store-internal")]
use sha2::Sha256;

#[cfg(feature = "canonical-store-internal")]
type HmacSha256 = Hmac<Sha256>;

/// Evidence that a confirmable fact source exists in the authoritative event
/// store. Fields are private and the only constructor verifies the recorded
/// event before exposing evidence to promotion APIs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommittedFactEvidence {
    target_fact_id: EntityId,
    campaign_id: EntityId,
    event_sequence: u64,
    stream_id: EntityId,
    stream_version: u64,
    source: FactSource,
    visibility: Visibility,
    fact_provenance: FactProvenance,
}

impl CommittedFactEvidence {
    pub fn load(
        store: &EventStore<CommandAcceptedPayload>,
        event_sequence: u64,
        target_fact_id: impl Into<String>,
    ) -> DomainResult<Self> {
        let target_fact_id = EntityId::new(target_fact_id)?;
        let event = store
            .events()
            .iter()
            .find(|event| event.sequence == event_sequence)
            .ok_or(DomainError::CommittedFactEvidenceMissing)?;
        event
            .verify_recorded_integrity()
            .map_err(|_| DomainError::CommittedFactEvidenceInvalid)?;
        if event.sequence == 0
            || event.stream_version == 0
            || event.event_type != expected_source_event_type(event.payload.fact_source)
            || event.payload.kind != DomainCommandKind::RecordDecision
            || event.payload.fact_source == FactSource::DiceRoll
            || event.payload.target_fact_id != target_fact_id.as_str()
        {
            return Err(DomainError::CommittedFactEvidenceInvalid);
        }

        validate_source_provenance(event.payload.fact_source, &event.fact_provenance.kind)?;

        Ok(Self {
            target_fact_id,
            campaign_id: event.campaign_id.clone(),
            event_sequence: event.sequence,
            stream_id: event.stream_id.clone(),
            stream_version: event.stream_version,
            source: event.payload.fact_source,
            visibility: event.visibility.clone(),
            fact_provenance: event.fact_provenance.clone(),
        })
    }

    /// Loads production evidence only from a record sealed with the canonical
    /// store integrity key. The database adapter first verifies the event,
    /// audit, outbox, and external witness chains and then creates this seal;
    /// an unverified replay row cannot be promoted through this path.
    #[cfg(feature = "canonical-store-internal")]
    #[doc(hidden)]
    pub fn load_persisted(
        record: &PersistedFactEvidenceRecord,
        canonical_integrity_key: &[u8],
    ) -> DomainResult<Self> {
        record.verify(canonical_integrity_key)?;
        if record.event_sequence == 0
            || record.stream_version == 0
            || record.event_type != expected_source_event_type(record.source)
            || record.command_kind != DomainCommandKind::RecordDecision
            || record.source == FactSource::DiceRoll
        {
            return Err(DomainError::CommittedFactEvidenceInvalid);
        }
        validate_source_provenance(record.source, &record.fact_provenance.kind)?;
        Ok(Self {
            target_fact_id: record.target_fact_id.clone(),
            campaign_id: record.campaign_id.clone(),
            event_sequence: record.event_sequence,
            stream_id: record.stream_id.clone(),
            stream_version: record.stream_version,
            source: record.source,
            visibility: record.visibility.clone(),
            fact_provenance: record.fact_provenance.clone(),
        })
    }

    pub const fn event_sequence(&self) -> u64 {
        self.event_sequence
    }

    pub fn target_fact_id(&self) -> &EntityId {
        &self.target_fact_id
    }

    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn stream_id(&self) -> &EntityId {
        &self.stream_id
    }

    pub const fn stream_version(&self) -> u64 {
        self.stream_version
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
}

/// Integrity-bound handoff from the production canonical store into the
/// domain. Fields and the seal are private so downstream code cannot mutate a
/// verified row into a different fact before promotion.
#[derive(Clone, PartialEq, Eq)]
#[cfg(feature = "canonical-store-internal")]
#[doc(hidden)]
pub struct PersistedFactEvidenceRecord {
    target_fact_id: EntityId,
    campaign_id: EntityId,
    event_sequence: u64,
    stream_id: EntityId,
    stream_version: u64,
    event_type: String,
    command_kind: DomainCommandKind,
    source: FactSource,
    visibility: Visibility,
    fact_provenance: FactProvenance,
    canonical_event_integrity_hash: String,
    canonical_request_hash: String,
    evidence_seal: String,
}

#[cfg(feature = "canonical-store-internal")]
impl std::fmt::Debug for PersistedFactEvidenceRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PersistedFactEvidenceRecord")
            .field("target_fact_id", &self.target_fact_id)
            .field("campaign_id", &self.campaign_id)
            .field("event_sequence", &self.event_sequence)
            .field("stream_id", &self.stream_id)
            .field("stream_version", &self.stream_version)
            .field("event_type", &self.event_type)
            .field("command_kind", &self.command_kind)
            .field("source", &self.source)
            .field("visibility", &self.visibility)
            .field("fact_provenance", &self.fact_provenance)
            .field(
                "canonical_event_integrity_hash",
                &self.canonical_event_integrity_hash,
            )
            .field("canonical_request_hash", &self.canonical_request_hash)
            .field("evidence_seal", &"[REDACTED]")
            .finish()
    }
}

#[cfg(feature = "canonical-store-internal")]
impl PersistedFactEvidenceRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn seal_verified(
        target_fact_id: impl Into<String>,
        campaign_id: impl Into<String>,
        event_sequence: u64,
        stream_id: impl Into<String>,
        stream_version: u64,
        event_type: impl Into<String>,
        command_kind: DomainCommandKind,
        source: FactSource,
        visibility: Visibility,
        fact_provenance: FactProvenance,
        canonical_event_integrity_hash: impl Into<String>,
        canonical_request_hash: impl Into<String>,
        canonical_integrity_key: &[u8],
    ) -> DomainResult<Self> {
        let mut record = Self {
            target_fact_id: EntityId::new(target_fact_id)?,
            campaign_id: EntityId::new(campaign_id)?,
            event_sequence,
            stream_id: EntityId::new(stream_id)?,
            stream_version,
            event_type: event_type.into(),
            command_kind,
            source,
            visibility,
            fact_provenance,
            canonical_event_integrity_hash: canonical_event_integrity_hash.into(),
            canonical_request_hash: canonical_request_hash.into(),
            evidence_seal: String::new(),
        };
        record.validate_metadata(canonical_integrity_key)?;
        record.evidence_seal = record.compute_seal(canonical_integrity_key)?;
        Ok(record)
    }

    fn verify(&self, canonical_integrity_key: &[u8]) -> DomainResult<()> {
        self.validate_metadata(canonical_integrity_key)?;
        let supplied = self
            .evidence_seal
            .strip_prefix("hmac-sha256:")
            .and_then(decode_hex_32)
            .ok_or(DomainError::CommittedFactEvidenceInvalid)?;
        let mut mac = HmacSha256::new_from_slice(canonical_integrity_key)
            .map_err(|_| DomainError::CommittedFactEvidenceInvalid)?;
        update_evidence_mac(&mut mac, self);
        mac.verify_slice(&supplied)
            .map_err(|_| DomainError::CommittedFactEvidenceInvalid)
    }

    fn validate_metadata(&self, canonical_integrity_key: &[u8]) -> DomainResult<()> {
        if canonical_integrity_key.len() != 32
            || !valid_hash(&self.canonical_event_integrity_hash, "hmac-sha256:")
            || !valid_hash(&self.canonical_request_hash, "sha256:")
            || !self.visibility.is_well_formed()
        {
            return Err(DomainError::CommittedFactEvidenceInvalid);
        }
        Ok(())
    }

    fn compute_seal(&self, canonical_integrity_key: &[u8]) -> DomainResult<String> {
        let mut mac = HmacSha256::new_from_slice(canonical_integrity_key)
            .map_err(|_| DomainError::CommittedFactEvidenceInvalid)?;
        update_evidence_mac(&mut mac, self);
        Ok(format!(
            "hmac-sha256:{}",
            encode_hex(&mac.finalize().into_bytes())
        ))
    }
}

#[cfg(feature = "canonical-store-internal")]
fn update_evidence_mac(mac: &mut HmacSha256, record: &PersistedFactEvidenceRecord) {
    let fields = [
        record.target_fact_id.to_string(),
        record.campaign_id.to_string(),
        record.event_sequence.to_string(),
        record.stream_id.to_string(),
        record.stream_version.to_string(),
        record.event_type.clone(),
        command_kind_name(record.command_kind).to_owned(),
        fact_source_name(record.source).to_owned(),
        record.visibility.label().as_str().to_owned(),
        record
            .visibility
            .subject_id()
            .map(ToString::to_string)
            .unwrap_or_default(),
        provenance_kind_name(&record.fact_provenance.kind).to_owned(),
        record.fact_provenance.reference.to_string(),
        record.fact_provenance.recorded_by.to_string(),
        record.canonical_event_integrity_hash.clone(),
        record.canonical_request_hash.clone(),
    ];
    for field in fields {
        mac.update(&(field.len() as u64).to_be_bytes());
        mac.update(field.as_bytes());
    }
}

#[cfg(feature = "canonical-store-internal")]
fn command_kind_name(kind: DomainCommandKind) -> &'static str {
    match kind {
        DomainCommandKind::SubmitPlayerAction => "submit_player_action",
        DomainCommandKind::RecordDecision => "record_decision",
        DomainCommandKind::ForkCampaign => "fork_campaign",
        DomainCommandKind::PromoteFact => "promote_fact",
    }
}

#[cfg(feature = "canonical-store-internal")]
fn fact_source_name(source: FactSource) -> &'static str {
    match source {
        FactSource::GameEvent => "game_event",
        FactSource::DecisionRecord => "decision_record",
        FactSource::DiceRoll => "dice_roll",
        FactSource::CharacterSheetVersion => "character_sheet_version",
        FactSource::ClueRevealEvent => "clue_reveal_event",
        FactSource::AgentDraft => "agent_draft",
        FactSource::NpcClaim => "npc_claim",
        FactSource::PlayerInference => "player_inference",
    }
}

#[cfg(feature = "canonical-store-internal")]
fn provenance_kind_name(kind: &ProvenanceKind) -> &'static str {
    match kind {
        ProvenanceKind::UserStatement => "user_statement",
        ProvenanceKind::HumanKeeperStatement => "human_keeper_statement",
        ProvenanceKind::RulesEngineDecision => "rules_engine_decision",
        ProvenanceKind::ToolResult => "tool_result",
        ProvenanceKind::AgentProposal => "agent_proposal",
        ProvenanceKind::ImportedSource => "imported_source",
        ProvenanceKind::SystemFixture => "system_fixture",
    }
}

#[cfg(feature = "canonical-store-internal")]
fn valid_hash(value: &str, prefix: &str) -> bool {
    value.len() == prefix.len() + 64
        && value.starts_with(prefix)
        && value[prefix.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(feature = "canonical-store-internal")]
fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(feature = "canonical-store-internal")]
fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut output = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(output)
}

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
