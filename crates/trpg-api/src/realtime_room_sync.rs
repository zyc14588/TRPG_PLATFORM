crate::define_api_realtime_contract_module!(
    "realtime_room_sync",
    "RealtimeRoomSyncContractRecorded",
    "realtime_room_sync.event_schema",
    crate::contract_core::ApiRealtimeOperation::PublishRealtimeDelta
);

use serde::{Deserialize, Serialize};

use crate::realtime_sync::RealtimeEvent;
use crate::websocket_protocol::{validate_identifier, ProtocolError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomKind {
    Campaign,
    Scene,
    Group,
}

impl RoomKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Campaign => "campaign",
            Self::Scene => "scene",
            Self::Group => "group",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomSubscription {
    pub kind: RoomKind,
    pub room_id: String,
}

impl RoomSubscription {
    pub fn campaign(campaign_id: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = Self {
            kind: RoomKind::Campaign,
            room_id: campaign_id.into(),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_identifier(&self.room_id)
    }

    pub fn permits(&self, event: &RealtimeEvent) -> bool {
        match self.kind {
            RoomKind::Campaign => event.campaign_id == self.room_id,
            RoomKind::Scene => event.resource_type == "scene" && event.resource_id == self.room_id,
            RoomKind::Group => {
                (event.resource_type == "group" && event.resource_id == self.room_id)
                    || (event.visibility_label == "private_to_group"
                        && event.visibility_subject.as_deref() == Some(self.room_id.as_str()))
            }
        }
    }
}
