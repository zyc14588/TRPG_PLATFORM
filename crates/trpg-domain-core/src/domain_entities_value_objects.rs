use crate::authority_contract::DomainAuthorityContract;
use crate::ddd::{DomainResult, EntityId, FactProvenance, FactSource, Visibility};
use crate::visibility_fact_provenance::CommittedFactEvidence;
use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Campaign {
    pub campaign_id: EntityId,
    pub authority_contract: DomainAuthorityContract,
    pub current_version: u64,
}

macro_rules! stable_id {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name(EntityId);

        impl $name {
            pub fn new(value: impl Into<String>) -> CoreEntityResult<Self> {
                Ok(Self(
                    EntityId::new(value).map_err(|_| CoreEntityError::InvalidIdentifier)?,
                ))
            }

            pub fn as_entity_id(&self) -> &EntityId {
                &self.0
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

stable_id!(UserId);
stable_id!(CampaignId);
stable_id!(RoomId);
stable_id!(SessionId);
stable_id!(SceneId);
stable_id!(ScenarioId);
stable_id!(CharacterId);
stable_id!(CharacterSheetVersionId);
stable_id!(InviteId);
stable_id!(CampaignForkId);
stable_id!(ReconsiderationId);

pub type CoreEntityResult<T> = Result<T, CoreEntityError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreEntityError {
    InvalidIdentifier,
    InvalidText(&'static str),
    InvalidVersion,
    InvalidTimestamp,
    InvalidTransition {
        aggregate: &'static str,
        from: &'static str,
        to: &'static str,
    },
    InviteExpired,
    InviteSubjectMismatch,
    CharacterSheetNotSubmitted,
    CharacterSheetAlreadyLocked,
    EventChainInvalid,
}

impl fmt::Display for CoreEntityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier => formatter.write_str("CORE_INVALID_IDENTIFIER"),
            Self::InvalidText(field) => write!(formatter, "CORE_INVALID_TEXT:{field}"),
            Self::InvalidVersion => formatter.write_str("CORE_INVALID_VERSION"),
            Self::InvalidTimestamp => formatter.write_str("CORE_INVALID_TIMESTAMP"),
            Self::InvalidTransition {
                aggregate,
                from,
                to,
            } => write!(formatter, "CORE_INVALID_TRANSITION:{aggregate}:{from}:{to}"),
            Self::InviteExpired => formatter.write_str("CAMPAIGN_INVITE_EXPIRED"),
            Self::InviteSubjectMismatch => formatter.write_str("CAMPAIGN_INVITE_SUBJECT_MISMATCH"),
            Self::CharacterSheetNotSubmitted => {
                formatter.write_str("CHARACTER_SHEET_NOT_SUBMITTED")
            }
            Self::CharacterSheetAlreadyLocked => {
                formatter.write_str("CHARACTER_SHEET_ALREADY_LOCKED")
            }
            Self::EventChainInvalid => formatter.write_str("RECONSIDERATION_EVENT_CHAIN_INVALID"),
        }
    }
}

impl Error for CoreEntityError {}

fn required_text(value: impl Into<String>, field: &'static str) -> CoreEntityResult<String> {
    let value = value.into();
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 512 {
        return Err(CoreEntityError::InvalidText(field));
    }
    Ok(trimmed.to_owned())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CampaignState {
    Draft,
    Ready,
    Active,
    Ended,
    Archived,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignAggregate {
    pub campaign_id: CampaignId,
    pub owner_user_id: UserId,
    pub authority_contract_id: EntityId,
    pub title: String,
    pub state: CampaignState,
    pub version: u64,
    pub created_at_unix_ms: u64,
}

impl CampaignAggregate {
    pub fn new(
        campaign_id: impl Into<String>,
        owner_user_id: impl Into<String>,
        authority_contract_id: impl Into<String>,
        title: impl Into<String>,
        created_at_unix_ms: u64,
    ) -> CoreEntityResult<Self> {
        if created_at_unix_ms == 0 {
            return Err(CoreEntityError::InvalidTimestamp);
        }
        Ok(Self {
            campaign_id: CampaignId::new(campaign_id)?,
            owner_user_id: UserId::new(owner_user_id)?,
            authority_contract_id: EntityId::new(authority_contract_id)
                .map_err(|_| CoreEntityError::InvalidIdentifier)?,
            title: required_text(title, "campaign.title")?,
            state: CampaignState::Draft,
            version: 1,
            created_at_unix_ms,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityContractReference {
    pub contract_id: EntityId,
    pub campaign_id: CampaignId,
    pub version: u64,
}

impl AuthorityContractReference {
    pub fn new(
        contract_id: impl Into<String>,
        campaign_id: impl Into<String>,
        version: u64,
    ) -> CoreEntityResult<Self> {
        if version == 0 {
            return Err(CoreEntityError::InvalidVersion);
        }
        Ok(Self {
            contract_id: EntityId::new(contract_id)
                .map_err(|_| CoreEntityError::InvalidIdentifier)?,
            campaign_id: CampaignId::new(campaign_id)?,
            version,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
    pub room_id: RoomId,
    pub campaign_id: CampaignId,
    pub name: String,
    pub version: u64,
}

impl Room {
    pub fn new(
        room_id: impl Into<String>,
        campaign_id: impl Into<String>,
        name: impl Into<String>,
    ) -> CoreEntityResult<Self> {
        Ok(Self {
            room_id: RoomId::new(room_id)?,
            campaign_id: CampaignId::new(campaign_id)?,
            name: required_text(name, "room.name")?,
            version: 1,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionState {
    Scheduled,
    Active,
    Paused,
    Ended,
}

impl SessionState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "SCHEDULED",
            Self::Active => "ACTIVE",
            Self::Paused => "PAUSED",
            Self::Ended => "ENDED",
        }
    }

    pub fn transition(self, next: Self) -> CoreEntityResult<Self> {
        let legal = matches!(
            (self, next),
            (Self::Scheduled, Self::Active)
                | (Self::Active, Self::Paused)
                | (Self::Paused, Self::Active)
                | (Self::Active | Self::Paused, Self::Ended)
        );
        if legal {
            Ok(next)
        } else {
            Err(CoreEntityError::InvalidTransition {
                aggregate: "session",
                from: self.as_str(),
                to: next.as_str(),
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub session_id: SessionId,
    pub campaign_id: CampaignId,
    pub room_id: RoomId,
    pub scenario_id: ScenarioId,
    pub state: SessionState,
    pub active_scene_id: Option<SceneId>,
    pub version: u64,
}

impl Session {
    pub fn scheduled(
        session_id: impl Into<String>,
        campaign_id: impl Into<String>,
        room_id: impl Into<String>,
        scenario_id: impl Into<String>,
    ) -> CoreEntityResult<Self> {
        Ok(Self {
            session_id: SessionId::new(session_id)?,
            campaign_id: CampaignId::new(campaign_id)?,
            room_id: RoomId::new(room_id)?,
            scenario_id: ScenarioId::new(scenario_id)?,
            state: SessionState::Scheduled,
            active_scene_id: None,
            version: 0,
        })
    }

    pub fn transition(&mut self, next: SessionState) -> CoreEntityResult<()> {
        self.state = self.state.transition(next)?;
        self.version = self
            .version
            .checked_add(1)
            .ok_or(CoreEntityError::InvalidVersion)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SceneState {
    Ready,
    Active,
    Closed,
}

impl SceneState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Active => "ACTIVE",
            Self::Closed => "CLOSED",
        }
    }

    pub fn transition(self, next: Self) -> CoreEntityResult<Self> {
        let legal = matches!(
            (self, next),
            (Self::Ready, Self::Active) | (Self::Active, Self::Closed)
        );
        if legal {
            Ok(next)
        } else {
            Err(CoreEntityError::InvalidTransition {
                aggregate: "scene",
                from: self.as_str(),
                to: next.as_str(),
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene {
    pub scene_id: SceneId,
    pub campaign_id: CampaignId,
    pub session_id: Option<SessionId>,
    pub scenario_id: ScenarioId,
    pub room_id: Option<RoomId>,
    pub scene_key: String,
    pub name: String,
    pub state: SceneState,
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    pub scenario_id: ScenarioId,
    pub campaign_id: CampaignId,
    pub ruleset_id: EntityId,
    pub format_version: String,
    pub content_hash: String,
    pub validated: bool,
    pub version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MembershipRole {
    Player,
    Spectator,
}

impl MembershipRole {
    pub const fn as_database_role(self) -> &'static str {
        match self {
            Self::Player => "PLAYER",
            Self::Spectator => "SPECTATOR",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignInvite {
    pub invite_id: InviteId,
    pub campaign_id: CampaignId,
    pub invited_user_id: UserId,
    pub issued_by: UserId,
    pub role: MembershipRole,
    pub token_digest: String,
    pub expires_at_unix_ms: u64,
}

impl CampaignInvite {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        invite_id: impl Into<String>,
        campaign_id: impl Into<String>,
        invited_user_id: impl Into<String>,
        issued_by: impl Into<String>,
        role: MembershipRole,
        token_digest: impl Into<String>,
        expires_at_unix_ms: u64,
        now_unix_ms: u64,
    ) -> CoreEntityResult<Self> {
        let token_digest = token_digest.into();
        if expires_at_unix_ms <= now_unix_ms {
            return Err(CoreEntityError::InviteExpired);
        }
        if !token_digest.strip_prefix("sha256:").is_some_and(|digest| {
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(CoreEntityError::InvalidText("invite.token_digest"));
        }
        Ok(Self {
            invite_id: InviteId::new(invite_id)?,
            campaign_id: CampaignId::new(campaign_id)?,
            invited_user_id: UserId::new(invited_user_id)?,
            issued_by: UserId::new(issued_by)?,
            role,
            token_digest,
            expires_at_unix_ms,
        })
    }

    pub fn validate_acceptance(
        &self,
        accepting_user_id: &UserId,
        now_unix_ms: u64,
    ) -> CoreEntityResult<()> {
        if now_unix_ms >= self.expires_at_unix_ms {
            return Err(CoreEntityError::InviteExpired);
        }
        if accepting_user_id != &self.invited_user_id {
            return Err(CoreEntityError::InviteSubjectMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CharacterState {
    Draft,
    Submitted,
    Approved,
}

impl CharacterState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Submitted => "SUBMITTED",
            Self::Approved => "APPROVED",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Character {
    pub character_id: CharacterId,
    pub campaign_id: CampaignId,
    pub owner_user_id: UserId,
    pub display_name: String,
    pub state: CharacterState,
    pub current_sheet_version: u64,
    pub initial_version_locked: bool,
    pub version: u64,
}

impl Character {
    pub fn draft(
        character_id: impl Into<String>,
        campaign_id: impl Into<String>,
        owner_user_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> CoreEntityResult<Self> {
        Ok(Self {
            character_id: CharacterId::new(character_id)?,
            campaign_id: CampaignId::new(campaign_id)?,
            owner_user_id: UserId::new(owner_user_id)?,
            display_name: required_text(display_name, "character.display_name")?,
            state: CharacterState::Draft,
            current_sheet_version: 1,
            initial_version_locked: false,
            version: 1,
        })
    }

    pub fn submit(&mut self) -> CoreEntityResult<()> {
        if self.state != CharacterState::Draft || self.initial_version_locked {
            return Err(CoreEntityError::CharacterSheetAlreadyLocked);
        }
        self.state = CharacterState::Submitted;
        self.version += 1;
        Ok(())
    }

    pub fn approve_initial_version(&mut self) -> CoreEntityResult<()> {
        if self.state != CharacterState::Submitted {
            return Err(CoreEntityError::CharacterSheetNotSubmitted);
        }
        if self.initial_version_locked {
            return Err(CoreEntityError::CharacterSheetAlreadyLocked);
        }
        self.state = CharacterState::Approved;
        self.initial_version_locked = true;
        self.version += 1;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterSheetVersion {
    pub sheet_version_id: Option<CharacterSheetVersionId>,
    pub character_id: EntityId,
    pub version: u64,
    pub source_event_id: EntityId,
    pub sheet_json: Option<String>,
    pub locked: bool,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReconsiderationState {
    Requested,
    Reviewed,
    Resolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReconsiderationOutcome {
    Upheld,
    Corrected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reconsideration {
    pub reconsideration_id: ReconsiderationId,
    pub campaign_id: CampaignId,
    pub original_event_sequence: u64,
    pub requested_by: UserId,
    pub state: ReconsiderationState,
    pub outcome: Option<ReconsiderationOutcome>,
    pub event_chain: Vec<EntityId>,
    pub version: u64,
}

impl Reconsideration {
    pub fn requested(
        reconsideration_id: impl Into<String>,
        campaign_id: impl Into<String>,
        original_event_sequence: u64,
        requested_by: impl Into<String>,
        request_event_id: impl Into<String>,
    ) -> CoreEntityResult<Self> {
        if original_event_sequence == 0 {
            return Err(CoreEntityError::EventChainInvalid);
        }
        Ok(Self {
            reconsideration_id: ReconsiderationId::new(reconsideration_id)?,
            campaign_id: CampaignId::new(campaign_id)?,
            original_event_sequence,
            requested_by: UserId::new(requested_by)?,
            state: ReconsiderationState::Requested,
            outcome: None,
            event_chain: vec![
                EntityId::new(request_event_id).map_err(|_| CoreEntityError::InvalidIdentifier)?
            ],
            version: 1,
        })
    }

    pub fn append_review_event(&mut self, event_id: impl Into<String>) -> CoreEntityResult<()> {
        if self.state != ReconsiderationState::Requested {
            return Err(CoreEntityError::EventChainInvalid);
        }
        self.event_chain
            .push(EntityId::new(event_id).map_err(|_| CoreEntityError::InvalidIdentifier)?);
        self.state = ReconsiderationState::Reviewed;
        self.version += 1;
        Ok(())
    }

    pub fn resolve(
        &mut self,
        event_id: impl Into<String>,
        outcome: ReconsiderationOutcome,
    ) -> CoreEntityResult<()> {
        if self.state != ReconsiderationState::Reviewed {
            return Err(CoreEntityError::EventChainInvalid);
        }
        self.event_chain
            .push(EntityId::new(event_id).map_err(|_| CoreEntityError::InvalidIdentifier)?);
        self.state = ReconsiderationState::Resolved;
        self.outcome = Some(outcome);
        self.version += 1;
        Ok(())
    }
}

/// A child projection row derived from an immutable campaign-fork snapshot.
/// IDs are child-owned and deterministic; source IDs are retained only inside
/// the fork snapshot and materialization manifest for lineage/replay.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "row_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CampaignForkMaterializedRow {
    Scenario {
        scenario_id: String,
        ruleset_id: String,
        format_version: String,
        content_hash: String,
        document_json: String,
        visibility_label: String,
        visibility_subject: String,
    },
    Character {
        character_id: String,
        owner_user_id: String,
        display_name: String,
        state: String,
        initial_version_locked: bool,
        sheet_version_id: String,
        sheet_json: String,
        sheet_locked: bool,
        visibility_label: String,
        visibility_subject: String,
    },
    Session {
        session_id: String,
        room_id: String,
        scenario_id: String,
        state: String,
        active_scene_id: Option<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
        visibility_label: String,
        visibility_subject: String,
    },
    Scene {
        scene_id: String,
        session_id: String,
        scenario_id: String,
        room_id: String,
        scene_key: String,
        name: String,
        state: String,
        visibility_label: String,
        visibility_subject: String,
    },
    PublicEvent {
        fork_event_id: String,
        source_event_sequence: u64,
        source_event_type: String,
        source_resource_type: String,
        source_resource_id: String,
        source_payload_json: String,
        source_event_integrity_hash: String,
        visibility_label: String,
        visibility_subject: String,
    },
    DiscoveredClue {
        fork_clue_id: String,
        source_clue_id: String,
        importance: String,
        outcome: String,
        cost: Option<String>,
        visibility_label: String,
        visibility_subject: String,
    },
    NpcState {
        npc_state_id: String,
        source_npc_id: String,
        state_json: String,
        visibility_label: String,
        visibility_subject: String,
    },
    Combat {
        combat_id: String,
        session_id: String,
        status: String,
        round: u64,
        current_turn_index: u64,
        state_json: String,
        visibility_label: String,
        visibility_subject: String,
    },
    Chase {
        chase_id: String,
        session_id: String,
        status: String,
        range_band: u8,
        segment: u64,
        state_json: String,
        visibility_label: String,
        visibility_subject: String,
    },
    Conclusion {
        ending_event_id: String,
        session_id: String,
        ending_id: String,
        summary: String,
        ended_at_unix_ms: u64,
        visibility_label: String,
        visibility_subject: String,
    },
}

impl CampaignForkMaterializedRow {
    pub const fn projection_target_count(&self) -> usize {
        match self {
            Self::Character { .. } => 2,
            Self::Scenario { .. }
            | Self::Session { .. }
            | Self::Scene { .. }
            | Self::PublicEvent { .. }
            | Self::DiscoveredClue { .. }
            | Self::NpcState { .. }
            | Self::Combat { .. }
            | Self::Chase { .. }
            | Self::Conclusion { .. } => 1,
        }
    }
}

/// Versioned canonical payloads for P06 aggregates. The Event Store envelope
/// carries authority, visibility, provenance and command metadata; these
/// payloads carry only aggregate facts needed to rebuild projections.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CharacterCombatHealthUpdate {
    pub character_id: String,
    pub new_sheet_version_id: String,
    pub source_sheet_version: u64,
    pub source_character_version: u64,
    pub hp_before: u8,
    pub hp_after: u8,
    pub condition_before: String,
    pub condition_after: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "event_type", content = "data")]
pub enum CoreDomainEvent {
    CampaignCreated {
        schema_version: u16,
        campaign_id: String,
        owner_user_id: String,
        authority_contract_id: String,
        authority_mode: String,
        authority_owner: String,
        title: String,
        room_id: String,
        room_name: String,
        created_at_unix_ms: u64,
    },
    CampaignInviteIssued {
        schema_version: u16,
        invite_id: String,
        campaign_id: String,
        invited_user_id: String,
        issued_by: String,
        role: MembershipRole,
        token_digest: String,
        expires_at_unix_ms: u64,
    },
    CampaignInviteAccepted {
        schema_version: u16,
        invite_id: String,
        campaign_id: String,
        user_id: String,
        role: MembershipRole,
        accepted_at_unix_ms: u64,
    },
    CharacterCreated {
        schema_version: u16,
        character_id: String,
        campaign_id: String,
        owner_user_id: String,
        display_name: String,
        sheet_version_id: String,
        sheet_json: String,
    },
    CharacterSubmitted {
        schema_version: u16,
        character_id: String,
    },
    CharacterInitialVersionApproved {
        schema_version: u16,
        character_id: String,
        reviewed_by: String,
    },
    ScenarioImported {
        schema_version: u16,
        scenario_id: String,
        campaign_id: String,
        ruleset_id: String,
        format_version: String,
        content_hash: String,
        document_json: String,
    },
    SessionStarted {
        schema_version: u16,
        session_id: String,
        campaign_id: String,
        room_id: String,
        scenario_id: String,
        scene_id: String,
        scene_key: String,
        scene_name: String,
        started_at_unix_ms: u64,
    },
    SessionStateChanged {
        schema_version: u16,
        session_id: String,
        from: SessionState,
        to: SessionState,
        changed_at_unix_ms: u64,
    },
    SceneSwitched {
        schema_version: u16,
        session_id: String,
        previous_scene_id: String,
        next_scene_id: String,
        next_scene_key: String,
        next_scene_name: String,
        switched_at_unix_ms: u64,
    },
    CampaignForkRecorded {
        schema_version: u16,
        fork_id: String,
        parent_campaign_id: String,
        child_campaign_id: String,
        source_session_id: String,
        snapshot_hash: String,
        child_snapshot_hash: String,
        copy_scopes: Vec<crate::fork_canon_lineage::CopyScope>,
        canonical_snapshot_json: String,
        reason: String,
    },
    CampaignForkMaterializationRecorded {
        schema_version: u16,
        fork_id: String,
        child_campaign_id: String,
        child_session_id: String,
        child_scenario_id: String,
        child_snapshot_hash: String,
        child_state_json: String,
        materialized_row_count: u64,
        batch_count: u64,
    },
    CampaignForkMaterialized {
        schema_version: u16,
        fork_id: String,
        child_campaign_id: String,
        batch_index: u64,
        batch_count: u64,
        rows: Vec<CampaignForkMaterializedRow>,
    },
    ReconsiderationRequested {
        schema_version: u16,
        reconsideration_id: String,
        campaign_id: String,
        original_event_sequence: u64,
        requested_by: String,
        reason: String,
    },
    ReconsiderationReviewed {
        schema_version: u16,
        reconsideration_id: String,
        review_event_id: String,
        review_summary: String,
    },
    ReconsiderationUpheld {
        schema_version: u16,
        reconsideration_id: String,
        resolution_event_id: String,
        original_event_sequence: u64,
        resolution: String,
    },
    ReconsiderationCorrected {
        schema_version: u16,
        reconsideration_id: String,
        resolution_event_id: String,
        original_event_sequence: u64,
        resolution: String,
        corrected_event_type: String,
        corrected_payload_json: String,
    },
    CombatStateRecorded {
        schema_version: u16,
        combat_id: String,
        campaign_id: String,
        session_id: String,
        status: String,
        round: u64,
        turn_index: u64,
        version: u64,
        state_json: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        character_health_updates: Vec<CharacterCombatHealthUpdate>,
    },
    ChaseStateRecorded {
        schema_version: u16,
        chase_id: String,
        campaign_id: String,
        session_id: String,
        status: String,
        range_band: u8,
        segment: u64,
        version: u64,
        state_json: String,
    },
    EndingRecorded {
        schema_version: u16,
        ending_event_id: String,
        campaign_id: String,
        session_id: String,
        ending_id: String,
        summary: String,
        ended_at_unix_ms: u64,
    },
    CharacterGrowthApplied {
        schema_version: u16,
        growth_event_id: String,
        campaign_id: String,
        session_id: String,
        ending_event_id: String,
        character_id: String,
        source_sheet_version_id: String,
        new_sheet_version_id: String,
        skill_name: String,
        skill_before: u8,
        improvement_check_roll: u8,
        increase_roll: Option<u8>,
        skill_after: u8,
        server_roll_id: String,
        increase_roll_id: Option<String>,
    },
}

impl CoreDomainEvent {
    pub const SCHEMA_VERSION: u16 = 1;

    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::CampaignCreated { .. } => "CampaignCreated",
            Self::CampaignInviteIssued { .. } => "CampaignInviteIssued",
            Self::CampaignInviteAccepted { .. } => "CampaignInviteAccepted",
            Self::CharacterCreated { .. } => "CharacterCreated",
            Self::CharacterSubmitted { .. } => "CharacterSubmitted",
            Self::CharacterInitialVersionApproved { .. } => "CharacterInitialVersionApproved",
            Self::ScenarioImported { .. } => "ScenarioImported",
            Self::SessionStarted { .. } => "SessionStarted",
            Self::SessionStateChanged { .. } => "SessionStateChanged",
            Self::SceneSwitched { .. } => "SceneSwitched",
            Self::CampaignForkRecorded { .. } => "CampaignForkRecorded",
            Self::CampaignForkMaterializationRecorded { .. } => {
                "CampaignForkMaterializationRecorded"
            }
            Self::CampaignForkMaterialized { .. } => "CampaignForkMaterialized",
            Self::ReconsiderationRequested { .. } => "ReconsiderationRequested",
            Self::ReconsiderationReviewed { .. } => "ReconsiderationReviewed",
            Self::ReconsiderationUpheld { .. } => "ReconsiderationUpheld",
            Self::ReconsiderationCorrected { .. } => "ReconsiderationCorrected",
            Self::CombatStateRecorded { .. } => "CombatStateRecorded",
            Self::ChaseStateRecorded { .. } => "ChaseStateRecorded",
            Self::EndingRecorded { .. } => "EndingRecorded",
            Self::CharacterGrowthApplied { .. } => "CharacterGrowthApplied",
        }
    }

    pub const fn schema_version(&self) -> u16 {
        match self {
            Self::CampaignCreated { schema_version, .. }
            | Self::CampaignInviteIssued { schema_version, .. }
            | Self::CampaignInviteAccepted { schema_version, .. }
            | Self::CharacterCreated { schema_version, .. }
            | Self::CharacterSubmitted { schema_version, .. }
            | Self::CharacterInitialVersionApproved { schema_version, .. }
            | Self::ScenarioImported { schema_version, .. }
            | Self::SessionStarted { schema_version, .. }
            | Self::SessionStateChanged { schema_version, .. }
            | Self::SceneSwitched { schema_version, .. }
            | Self::CampaignForkRecorded { schema_version, .. }
            | Self::CampaignForkMaterializationRecorded { schema_version, .. }
            | Self::CampaignForkMaterialized { schema_version, .. }
            | Self::ReconsiderationRequested { schema_version, .. }
            | Self::ReconsiderationReviewed { schema_version, .. }
            | Self::ReconsiderationUpheld { schema_version, .. }
            | Self::ReconsiderationCorrected { schema_version, .. }
            | Self::CombatStateRecorded { schema_version, .. }
            | Self::ChaseStateRecorded { schema_version, .. }
            | Self::EndingRecorded { schema_version, .. }
            | Self::CharacterGrowthApplied { schema_version, .. } => *schema_version,
        }
    }

    pub fn validate_schema_version(&self) -> CoreEntityResult<()> {
        if self.schema_version() == Self::SCHEMA_VERSION {
            Ok(())
        } else {
            Err(CoreEntityError::InvalidVersion)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryFact {
    fact_id: EntityId,
    source: FactSource,
    visibility: Visibility,
    fact_provenance: FactProvenance,
    source_event_sequence: u64,
    confirmed: bool,
}

impl MemoryFact {
    pub fn confirmed(
        fact_id: impl Into<String>,
        evidence: &CommittedFactEvidence,
    ) -> DomainResult<Self> {
        let fact_id = EntityId::new(fact_id)?;
        if &fact_id != evidence.target_fact_id() {
            return Err(crate::ddd::DomainError::CommittedFactEvidenceInvalid);
        }
        Ok(Self {
            fact_id,
            source: evidence.source(),
            visibility: evidence.visibility().clone(),
            fact_provenance: evidence.fact_provenance().clone(),
            source_event_sequence: evidence.event_sequence(),
            confirmed: true,
        })
    }

    pub const fn is_confirmed(&self) -> bool {
        self.confirmed
    }

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
