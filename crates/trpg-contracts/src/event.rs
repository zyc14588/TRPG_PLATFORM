use std::error::Error;
use std::fmt;
use std::str::FromStr;

pub const CANONICAL_EVENT_VERSION: u16 = 1;
pub const CANONICAL_EVENT_SCHEMA_ID: &str = "trpg.events.canonical";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CanonicalEvent {
    CampaignCreated,
    AuthorityContractLocked,
    CharacterSheetSubmitted,
    CharacterSheetVersionLocked,
    DiceRolled,
    ClueRevealed,
    SessionSummaryCreated,
    SharedKernelTypesValidated,
    ApiRequestAccepted,
    WebSocketStateSynced,
    NatsMessagePublished,
}

impl CanonicalEvent {
    pub const ALL: &'static [Self] = &[
        Self::CampaignCreated,
        Self::AuthorityContractLocked,
        Self::CharacterSheetSubmitted,
        Self::CharacterSheetVersionLocked,
        Self::DiceRolled,
        Self::ClueRevealed,
        Self::SessionSummaryCreated,
        Self::SharedKernelTypesValidated,
        Self::ApiRequestAccepted,
        Self::WebSocketStateSynced,
        Self::NatsMessagePublished,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::CampaignCreated => "CampaignCreated",
            Self::AuthorityContractLocked => "AuthorityContractLocked",
            Self::CharacterSheetSubmitted => "CharacterSheetSubmitted",
            Self::CharacterSheetVersionLocked => "CharacterSheetVersionLocked",
            Self::DiceRolled => "DiceRolled",
            Self::ClueRevealed => "ClueRevealed",
            Self::SessionSummaryCreated => "SessionSummaryCreated",
            Self::SharedKernelTypesValidated => "SharedKernelTypesValidated",
            Self::ApiRequestAccepted => "ApiRequestAccepted",
            Self::WebSocketStateSynced => "WebSocketStateSynced",
            Self::NatsMessagePublished => "NatsMessagePublished",
        }
    }

    pub const fn version(self) -> u16 {
        CANONICAL_EVENT_VERSION
    }

    pub const fn schema_id(self) -> &'static str {
        CANONICAL_EVENT_SCHEMA_ID
    }

    pub const fn descriptor(self) -> EventDescriptor {
        EventDescriptor {
            event: self,
            name: self.name(),
            version: self.version(),
            schema_id: self.schema_id(),
        }
    }

    pub fn lookup(value: &str) -> Result<Self, UnknownEventName> {
        Self::ALL
            .iter()
            .copied()
            .find(|event| event.name() == value)
            .ok_or(UnknownEventName)
    }
}

impl fmt::Display for CanonicalEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

impl FromStr for CanonicalEvent {
    type Err = UnknownEventName;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::lookup(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EventDescriptor {
    event: CanonicalEvent,
    name: &'static str,
    version: u16,
    schema_id: &'static str,
}

impl EventDescriptor {
    pub const fn event(self) -> CanonicalEvent {
        self.event
    }

    pub const fn name(self) -> &'static str {
        self.name
    }

    pub const fn version(self) -> u16 {
        self.version
    }

    pub const fn schema_id(self) -> &'static str {
        self.schema_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownEventName;

impl fmt::Display for UnknownEventName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unknown canonical event name")
    }
}

impl Error for UnknownEventName {}
