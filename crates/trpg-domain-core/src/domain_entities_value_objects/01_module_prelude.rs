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
