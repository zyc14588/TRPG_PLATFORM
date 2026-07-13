use std::error::Error;
use std::fmt;
use std::str::FromStr;

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
        1
    }

    pub const fn schema_name(self) -> &'static str {
        match self {
            Self::CampaignCreated => "campaign_created.v1.schema.json",
            Self::AuthorityContractLocked => "authority_contract_locked.v1.schema.json",
            Self::CharacterSheetSubmitted => "character_sheet_submitted.v1.schema.json",
            Self::CharacterSheetVersionLocked => "character_sheet_version_locked.v1.schema.json",
            Self::DiceRolled => "dice_rolled.v1.schema.json",
            Self::ClueRevealed => "clue_revealed.v1.schema.json",
            Self::SessionSummaryCreated => "session_summary_created.v1.schema.json",
            Self::SharedKernelTypesValidated => "shared_kernel_types_validated.v1.schema.json",
            Self::ApiRequestAccepted => "api_request_accepted.v1.schema.json",
            Self::WebSocketStateSynced => "web_socket_state_synced.v1.schema.json",
            Self::NatsMessagePublished => "nats_message_published.v1.schema.json",
        }
    }

    pub const fn descriptor(self) -> EventDescriptor {
        EventDescriptor {
            event: self,
            name: self.name(),
            version: self.version(),
            schema_name: self.schema_name(),
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
    schema_name: &'static str,
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

    pub const fn schema_name(self) -> &'static str {
        self.schema_name
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
