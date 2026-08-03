
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WitnessPhase {
    Prepared,
    Committed,
    Aborted,
}

#[derive(Clone, Copy, Debug)]
enum AtomicProjection<'a> {
    PlayerAction(&'a serde_json::Value),
    CampaignInviteAcceptance(&'a serde_json::Value),
    GameplayRollReservation(&'a serde_json::Value),
    SessionEndingReservation(&'a serde_json::Value),
    AgentJobRequest(&'a serde_json::Value),
}

impl WitnessPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "PREPARED",
            Self::Committed => "COMMITTED",
            Self::Aborted => "ABORTED",
        }
    }
}

#[derive(Clone, Debug)]
struct WitnessRecord {
    sequence: i64,
    commit_id: String,
    phase: String,
    request_hash: String,
    first_sequence: Option<i64>,
    last_sequence: Option<i64>,
    reason: String,
    key_id: String,
    previous_hash: String,
    record_hash: String,
}

#[derive(Clone, Debug)]
struct AuditRecord {
    sequence: i64,
    commit_id: String,
    campaign_id: String,
    actor_id: String,
    actor_origin: String,
    authentication_reference: String,
    resource_type: String,
    resource_id: String,
    action: String,
    requested_role: String,
    visibility_label: String,
    visibility_subject: String,
    provenance_kind: String,
    provenance_reference: String,
    provenance_recorded_by: String,
    decision: String,
    openfga_decision_id: String,
    openfga_policy_revision: String,
    opa_decision_id: String,
    opa_policy_revision: String,
    trace_id: String,
    correlation_id: String,
    causation_id: String,
    event_batch_hash: String,
    witness_prepare_sequence: i64,
    witness_prepare_hash: String,
    occurred_at: DateTime<Utc>,
    integrity_version: i32,
    key_id: String,
    previous_hash: String,
    record_hash: String,
}
