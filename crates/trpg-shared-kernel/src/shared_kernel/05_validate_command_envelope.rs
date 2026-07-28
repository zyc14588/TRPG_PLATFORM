
pub fn validate_command_envelope<T>(command: &CommandEnvelope<T>) -> KernelResult<()> {
    if command.idempotency_key.trim().is_empty() {
        return Err(TrpgError::MissingIdempotencyKey);
    }

    match command.write_path {
        FormalWritePath::DirectAgent => return Err(TrpgError::DirectAgentStateWrite),
        FormalWritePath::DirectBusiness => return Err(TrpgError::PolicyDenied),
        FormalWritePath::WorkflowDecision
        | FormalWritePath::RulesDecision
        | FormalWritePath::ToolDecision => {}
    }

    let context = command.authenticated_context();
    if &command.actor != context.actor() {
        return Err(TrpgError::InternalIdentityInvalid);
    }
    if context.authentication_expires_at_unix_ms <= context.authenticated_at_unix_ms {
        return Err(TrpgError::AuthenticationRequired);
    }

    match (&command.authority_mode, command.actor.role()) {
        (AuthorityMode::HumanKp, ActorRole::HumanKeeper)
        | (AuthorityMode::HumanKp, ActorRole::Workflow)
        | (AuthorityMode::HumanKp, ActorRole::RulesEngine)
        | (AuthorityMode::HumanKp, ActorRole::System) => {}
        (AuthorityMode::AiKp, ActorRole::Workflow)
        | (AuthorityMode::AiKp, ActorRole::RulesEngine)
        | (AuthorityMode::AiKp, ActorRole::System) => {}
        _ => return Err(TrpgError::AuthorityViolation),
    }

    Ok(())
}

/// Persistence-neutral request used by runtime and agent layers to hand a
/// fully authorized formal event batch to the canonical Event Store adapter.
/// The port lives in the shared kernel so production composition roots can
/// inject a durable adapter without reversing crate dependency direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCommitEvent {
    pub event_type: String,
    pub payload_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalPolicyAudit {
    pub actor_id: String,
    pub actor_origin: String,
    pub authentication_reference: String,
    pub resource_type: String,
    pub resource_id: String,
    pub action: String,
    pub requested_role: String,
    pub openfga_decision_id: String,
    pub openfga_policy_revision: String,
    pub opa_decision_id: String,
    pub opa_policy_revision: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCommitRequest {
    pub commit_id: String,
    pub campaign_id: String,
    pub idempotency_key: String,
    pub expected_version: u64,
    pub command_id: String,
    pub authenticated_actor_id: String,
    pub authenticated_actor_role: String,
    pub authenticated_actor_origin: EventActorOriginWire,
    pub authority_mode: String,
    pub authority_contract_version: u64,
    pub authority_contract_id: String,
    pub authority_owner: String,
    pub visibility_label: String,
    pub visibility_subject: String,
    /// Independent personal-data owner for crypto-erasure and data-subject
    /// workflows. This is deliberately not derived from the visibility
    /// audience: public, party, keeper, and group-visible records can still
    /// contain one person's data.
    pub data_subject_id: String,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub trace_id: String,
    pub events: Vec<CanonicalCommitEvent>,
    pub audit: CanonicalPolicyAudit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCommittedEvent {
    pub sequence: u64,
    pub stream_version: u64,
    pub event_type: String,
    pub payload_json: String,
    pub command_id: String,
    pub idempotency_key: String,
    pub occurred_at_unix_ms: u64,
    /// Store-generated HMAC for this exact canonical event. Consumers that
    /// bind a secondary workflow to an event must use this value rather than
    /// synthesizing a process-local digest.
    pub event_integrity_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCommitReceipt {
    pub first_stream_version: u64,
    pub last_stream_version: u64,
    /// Exact durable identities; consumers must not invent local replacements.
    pub events: Vec<CanonicalCommittedEvent>,
}

/// Stable lookup scope used to resolve an already committed command before a
/// caller repeats any non-idempotent external tool execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCommitKey {
    pub commit_id: String,
    pub campaign_id: String,
    pub stream_id: String,
    pub idempotency_key: String,
    pub expected_version: u64,
}

pub trait CanonicalCommitPort: fmt::Debug + Send + Sync {
    /// Resolves a previously committed canonical batch without requiring the
    /// caller to reconstruct tool-produced payload fields. Implementations
    /// must return only receipts backed by their trusted canonical custody.
    fn load_receipt(
        &self,
        key: &CanonicalCommitKey,
    ) -> KernelResult<Option<CanonicalCommitReceipt>>;

    /// Atomically validates the campaign stream version and idempotency key,
    /// persists the complete formal batch, and returns its durable range.
    fn commit(&self, request: &CanonicalCommitRequest) -> KernelResult<CanonicalCommitReceipt>;

    /// Revalidates an exact receipt against the port's trusted canonical
    /// custody. Durable adapters must prove the keyed primary/audit chains and
    /// external witness binding; callers must never accept a hash-shaped
    /// string as equivalent evidence.
    fn verify_receipt(
        &self,
        request: &CanonicalCommitRequest,
        receipt: &CanonicalCommitReceipt,
    ) -> KernelResult<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventEnvelope<P> {
    pub sequence: u64,
    pub stream_id: EntityId,
    pub stream_version: u64,
    pub event_type: &'static str,
    pub campaign_id: EntityId,
    pub authenticated_actor: Actor,
    pub resource: ResourceRef,
    pub authority_contract_id: EntityId,
    pub authority_owner: EntityId,
    pub command_id: EntityId,
    pub idempotency_key: String,
    pub authority_contract_version: u64,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
    pub correlation_id: EntityId,
    pub causation_id: EntityId,
    pub trace_id: EntityId,
    pub occurred_at_unix_ms: u64,
    pub payload: P,
    recorded_payload: P,
    integrity_hash: [u8; 32],
}

pub const EVENT_ENVELOPE_WIRE_SCHEMA_VERSION: u16 = 2;

/// Stable, versioned representation used at persistence and transport
/// boundaries. Domain-only private fields stay inside `EventEnvelope`, while
/// every authoritative classification and provenance field is explicit here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelopeWire<P> {
    pub schema_version: u16,
    pub event_schema_version: u32,
    pub sequence: u64,
    pub stream_id: String,
    pub stream_version: u64,
    pub event_type: String,
    pub campaign_id: String,
    pub authenticated_actor_id: String,
    pub authenticated_actor_role: String,
    pub authenticated_actor_origin: EventActorOriginWire,
    pub resource_campaign_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub authority_contract_id: String,
    pub authority_owner: String,
    pub command_id: String,
    pub idempotency_key: String,
    pub authority_contract_version: u64,
    pub visibility_label: String,
    pub visibility_subject: Option<String>,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub trace_id: String,
    pub occurred_at_unix_ms: u64,
    pub payload: P,
    pub request_hash_source: String,
    pub integrity_status: String,
    /// Historical imports can predate the HMAC domain. Absence is explicit
    /// and must be interpreted together with the persisted integrity status;
    /// callers must never synthesize a hash for those records.
    pub integrity_hash: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventActorOriginWire {
    UserSession {
        session_id: String,
    },
    Workload {
        role: String,
    },
    AgentRun {
        run_id: String,
        class: String,
        campaign_id: String,
    },
}

impl<P: Serialize> EventEnvelopeWire<P> {
    pub fn to_canonical_json(&self) -> KernelResult<String> {
        let value = serde_json::to_value(self).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        serde_json::to_string(&value).map_err(|_| TrpgError::AuditIntegrityViolation)
    }
}

impl<P: PartialEq + Serialize> EventEnvelope<P> {
    pub fn verify_recorded_integrity(&self) -> KernelResult<()> {
        if self.payload != self.recorded_payload
            || self.integrity_hash != event_integrity_hash(self)?
        {
            return Err(TrpgError::PolicyEvidenceUntrusted);
        }
        Ok(())
    }
}

impl<P: Clone + PartialEq + Serialize> EventEnvelope<P> {
    pub fn to_canonical_wire(&self) -> EventEnvelopeWire<P> {
        EventEnvelopeWire {
            schema_version: EVENT_ENVELOPE_WIRE_SCHEMA_VERSION,
            event_schema_version: 1,
            sequence: self.sequence,
            stream_id: self.stream_id.to_string(),
            stream_version: self.stream_version,
            event_type: self.event_type.to_owned(),
            campaign_id: self.campaign_id.to_string(),
            authenticated_actor_id: self.authenticated_actor.id().to_string(),
            authenticated_actor_role: actor_role_integrity_name(self.authenticated_actor.role())
                .to_owned(),
            authenticated_actor_origin: event_actor_origin_wire(self.authenticated_actor.origin()),
            resource_campaign_id: self.resource.campaign_id().to_string(),
            resource_type: self.resource.resource_type().to_string(),
            resource_id: self.resource.resource_id().to_string(),
            authority_contract_id: self.authority_contract_id.to_string(),
            authority_owner: self.authority_owner.to_string(),
            command_id: self.command_id.to_string(),
            idempotency_key: self.idempotency_key.clone(),
            authority_contract_version: self.authority_contract_version,
            visibility_label: self.visibility.label().as_str().to_owned(),
            visibility_subject: self.visibility.subject_id().map(ToString::to_string),
            provenance_kind: provenance_kind_integrity_name(&self.fact_provenance.kind).to_owned(),
            provenance_reference: self.fact_provenance.reference.to_string(),
            provenance_recorded_by: self.fact_provenance.recorded_by.to_string(),
            correlation_id: self.correlation_id.to_string(),
            causation_id: self.causation_id.to_string(),
            trace_id: self.trace_id.to_string(),
            occurred_at_unix_ms: self.occurred_at_unix_ms,
            payload: self.payload.clone(),
            request_hash_source: "shared_kernel_append".to_owned(),
            integrity_status: "verified_sha256".to_owned(),
            integrity_hash: Some(format!("sha256:{}", hex_lower(&self.integrity_hash))),
        }
    }

    pub fn to_canonical_json(&self) -> KernelResult<String> {
        self.to_canonical_wire().to_canonical_json()
    }
}

#[derive(Clone, Debug)]
pub struct EventStore<P> {
    stream_base_versions: HashMap<(EntityId, EntityId), u64>,
    events: Vec<EventEnvelope<P>>,
    idempotency_index: HashMap<(EntityId, EntityId, String), IdempotencyRecord>,
}

#[derive(Clone, Debug)]
struct IdempotencyRecord {
    request_hash: [u8; 32],
    event_index: usize,
}

impl<P> Default for EventStore<P> {
    fn default() -> Self {
        Self {
            stream_base_versions: HashMap::new(),
            events: Vec::new(),
            idempotency_index: HashMap::new(),
        }
    }
}
