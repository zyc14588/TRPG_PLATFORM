
#[derive(serde::Deserialize)]
struct ForkSnapshotSession {
    state: String,
    active_scene_id: Option<String>,
    started_at_unix_ms: u64,
    ended_at_unix_ms: u64,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ForkSnapshotCharacter {
    character_id: String,
    owner_user_id: String,
    display_name: String,
    state: String,
    initial_version_locked: bool,
    visibility_label: String,
    visibility_subject: String,
    current_sheet: Option<ForkSnapshotSheet>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ForkSnapshotSheet {
    sheet_json: Value,
    locked: bool,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotScene {
    scene_id: String,
    scene_key: String,
    name: String,
    state: String,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotWorld {
    ruleset_id: String,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ForkSnapshotPublicEvent {
    sequence: u64,
    event_type: String,
    resource_type: String,
    resource_id: String,
    payload: Value,
    event_integrity_hash: String,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotClue {
    clue_id: String,
    importance: String,
    outcome: String,
    cost: Option<String>,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotCombat {
    combat_id: String,
    status: String,
    round: u64,
    current_turn_index: u64,
    state: Value,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotChase {
    chase_id: String,
    status: String,
    range_band: u8,
    segment: u64,
    state: Value,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotConclusion {
    ending_event_id: String,
    ending_id: String,
    summary: String,
    growth_awards: Vec<ForkSnapshotGrowthAward>,
    consumed_growth_awards: Vec<ForkSnapshotConsumedGrowthAward>,
    ended_at_unix_ms: u64,
    visibility_label: String,
    visibility_subject: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ForkSnapshotGrowthAward {
    skill_name: String,
    reason: String,
    #[serde(default)]
    consumed_by_character_ids: Vec<String>,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotConsumedGrowthAward {
    character_id: String,
    skill_name: String,
}

#[derive(serde::Deserialize)]
struct ForkSnapshotNpcState {
    npc_id: String,
    state: Value,
    visibility_label: String,
    visibility_subject: String,
}

struct CampaignForkMaterializationBatch {
    rows: Vec<CampaignForkMaterializedRow>,
    visibility_label: String,
    visibility_subject: String,
    data_subject_id: String,
}

struct CampaignForkMaterialization {
    child_session_id: String,
    child_scenario_id: String,
    child_state_json: String,
    child_snapshot_hash: String,
    rows: Vec<CampaignForkMaterializedRow>,
    batches: Vec<CampaignForkMaterializationBatch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestReconsiderationRequest {
    pub reconsideration_id: String,
    pub campaign_id: String,
    pub original_event_sequence: i64,
    pub requested_by: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewReconsiderationRequest {
    pub reconsideration_id: String,
    pub campaign_id: String,
    pub review_event_id: String,
    pub review_summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolveReconsiderationRequest {
    pub reconsideration_id: String,
    pub campaign_id: String,
    pub resolution_event_id: String,
    pub outcome: ReconsiderationOutcome,
    pub resolution: String,
    pub corrected_event_type: Option<String>,
    pub corrected_payload_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordCombatStateRequest {
    pub campaign_id: String,
    pub session_id: String,
    pub state_json: String,
    pub attacker_roll: Option<ServerPercentileRoll>,
    pub defender_roll: Option<ServerPercentileRoll>,
    pub damage_roll: Option<ServerDamageRoll>,
    pub medical_roll: Option<ServerPercentileRoll>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordChaseStateRequest {
    pub campaign_id: String,
    pub session_id: String,
    pub state_json: String,
    pub participant_rolls: Vec<ServerPercentileRoll>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordEndingRequest {
    pub ending_event_id: String,
    pub campaign_id: String,
    pub session_id: String,
    pub ending_id: String,
    pub summary: String,
    pub ended_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordGrowthRequest {
    pub growth_event_id: String,
    pub campaign_id: String,
    pub session_id: String,
    pub ending_event_id: String,
    pub character_id: String,
    pub source_sheet_version_id: String,
    pub new_sheet_version_id: String,
    pub skill_name: String,
    pub growth_rolls: ServerGrowthRollEvidence,
}

#[derive(Debug)]
pub enum CoreDomainRepositoryError {
    InvalidInput(&'static str),
    Domain(CoreEntityError),
    Canonical(CanonicalStoreError),
    Database(&'static str),
    Serialization,
    NotFound(&'static str),
    Forbidden,
    PolicyEvidenceMismatch,
    Integrity(&'static str),
    ConcurrentStart,
}

impl fmt::Display for CoreDomainRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(field) => write!(formatter, "CORE_INPUT_INVALID:{field}"),
            Self::Domain(error) => error.fmt(formatter),
            Self::Canonical(error) => error.fmt(formatter),
            Self::Database(operation) => write!(formatter, "CORE_DATABASE_ERROR:{operation}"),
            Self::Serialization => formatter.write_str("CORE_SERIALIZATION_ERROR"),
            Self::NotFound(entity) => write!(formatter, "CORE_NOT_FOUND:{entity}"),
            Self::Forbidden => formatter.write_str("CORE_FORBIDDEN"),
            Self::PolicyEvidenceMismatch => formatter.write_str("CORE_POLICY_EVIDENCE_MISMATCH"),
            Self::Integrity(reason) => write!(formatter, "CORE_INTEGRITY_ERROR:{reason}"),
            Self::ConcurrentStart => formatter.write_str("CORE_SESSION_ALREADY_LIVE"),
        }
    }
}

impl Error for CoreDomainRepositoryError {}

impl From<CoreEntityError> for CoreDomainRepositoryError {
    fn from(error: CoreEntityError) -> Self {
        Self::Domain(error)
    }
}

impl From<CanonicalStoreError> for CoreDomainRepositoryError {
    fn from(error: CanonicalStoreError) -> Self {
        Self::Canonical(error)
    }
}

pub trait CoreDomainClock: fmt::Debug + Send + Sync {
    fn now_unix_ms(&self) -> Result<u64, CoreDomainRepositoryError>;
}

#[derive(Debug)]
struct SystemCoreDomainClock;

impl CoreDomainClock for SystemCoreDomainClock {
    fn now_unix_ms(&self) -> Result<u64, CoreDomainRepositoryError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| CoreDomainRepositoryError::Integrity("trusted_clock_before_epoch"))?;
        u64::try_from(elapsed.as_millis())
            .map_err(|_| CoreDomainRepositoryError::Integrity("trusted_clock_out_of_range"))
    }
}

#[derive(Clone)]
pub struct CoreDomainRepository {
    primary: PgPool,
    canonical: PostgresCanonicalStore,
    clock: Arc<dyn CoreDomainClock>,
}

impl fmt::Debug for CoreDomainRepository {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CoreDomainRepository")
            .field("primary", &"[POSTGRESQL POOL]")
            .field("canonical", &self.canonical)
            .field("clock", &"[TRUSTED SERVER CLOCK]")
            .finish()
    }
}
