crate::define_api_realtime_contract_module!(
    "websocket_protocol",
    "WebsocketProtocolContractRecorded",
    "websocket_protocol.event_schema",
    crate::contract_core::ApiRealtimeOperation::PublishRealtimeDelta
);

use serde::{Deserialize, Serialize};

use crate::realtime_room_sync::RoomSubscription;
use crate::realtime_sync::RealtimeEvent;

pub const REALTIME_PROTOCOL_VERSION: &str = "trpg.realtime.v1";
pub const REALTIME_SUBPROTOCOL: &str = "trpg.realtime.v1";

pub const CLOSE_AUTHENTICATION_REQUIRED: u16 = 4001;
pub const CLOSE_AUTHORIZATION_REVOKED: u16 = 4003;
pub const CLOSE_RATE_LIMITED: u16 = 4008;
pub const CLOSE_RESYNC_REQUIRED: u16 = 4009;
pub const CLOSE_SLOW_CONSUMER: u16 = 4010;
pub const CLOSE_AUTHORITY_CHANGED: u16 = 4011;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientEnvelope {
    pub version: String,
    pub request_id: String,
    #[serde(flatten)]
    pub message: ClientMessage,
}

impl ClientEnvelope {
    pub fn parse_json(value: &str) -> Result<Self, ProtocolError> {
        let envelope: Self =
            serde_json::from_str(value).map_err(|_| ProtocolError::MalformedEnvelope)?;
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn to_json(&self) -> Result<String, ProtocolError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|_| ProtocolError::MalformedEnvelope)
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.version != REALTIME_PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        validate_identifier(&self.request_id)?;
        match &self.message {
            ClientMessage::Subscribe {
                subscription,
                resume_token,
                ..
            } => {
                subscription.validate()?;
                if resume_token
                    .as_deref()
                    .is_some_and(|token| token.is_empty() || token.len() > 2_048)
                {
                    return Err(ProtocolError::InvalidResumeToken);
                }
            }
            ClientMessage::Ack { .. } | ClientMessage::Pong { .. } => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMessage {
    Subscribe {
        subscription: RoomSubscription,
        #[serde(default)]
        cursor: u64,
        #[serde(default)]
        resume_token: Option<String>,
    },
    Ack {
        cursor: u64,
    },
    Pong {
        nonce: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerEnvelope {
    pub version: String,
    pub sequence: u64,
    #[serde(flatten)]
    pub message: ServerMessage,
}

impl ServerEnvelope {
    pub fn new(sequence: u64, message: ServerMessage) -> Result<Self, ProtocolError> {
        if sequence == 0 {
            return Err(ProtocolError::InvalidSequence);
        }
        Ok(Self {
            version: REALTIME_PROTOCOL_VERSION.to_owned(),
            sequence,
            message,
        })
    }

    pub fn parse_json(value: &str) -> Result<Self, ProtocolError> {
        let envelope: Self =
            serde_json::from_str(value).map_err(|_| ProtocolError::MalformedEnvelope)?;
        if envelope.version != REALTIME_PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        if envelope.sequence == 0 {
            return Err(ProtocolError::InvalidSequence);
        }
        Ok(envelope)
    }

    pub fn to_json(&self) -> Result<String, ProtocolError> {
        serde_json::to_string(self).map_err(|_| ProtocolError::MalformedEnvelope)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerMessage {
    Connected {
        binding: ConnectionBinding,
        heartbeat_interval_ms: u64,
    },
    Subscribed {
        request_id: String,
        subscription: RoomSubscription,
        cursor: u64,
        resume_token: String,
    },
    Event {
        cursor: u64,
        event: Box<RealtimeEvent>,
    },
    Checkpoint {
        cursor: u64,
        resume_token: String,
    },
    Acked {
        request_id: String,
        cursor: u64,
        resume_token: String,
    },
    Heartbeat {
        nonce: u64,
    },
    SubscriptionChanged {
        subscription: RoomSubscription,
        seat: String,
        authority_epoch: u64,
    },
    ResyncRequired {
        request_id: String,
        reason: String,
        earliest_cursor: u64,
        latest_cursor: u64,
    },
    Error {
        request_id: Option<String>,
        code: String,
        retryable: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionBinding {
    pub connection_id: String,
    pub tenant_id: String,
    pub user_id: String,
    pub campaign_id: String,
    pub seat: String,
    pub authority_mode: String,
    pub authority_epoch: u64,
}

impl ConnectionBinding {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        for value in [
            &self.connection_id,
            &self.tenant_id,
            &self.user_id,
            &self.campaign_id,
            &self.seat,
            &self.authority_mode,
        ] {
            validate_identifier(value)?;
        }
        if self.authority_epoch == 0 {
            return Err(ProtocolError::InvalidSequence);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    MalformedEnvelope,
    UnsupportedVersion,
    InvalidIdentifier,
    InvalidSequence,
    InvalidResumeToken,
}

impl ProtocolError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MalformedEnvelope => "REALTIME_ENVELOPE_MALFORMED",
            Self::UnsupportedVersion => "REALTIME_PROTOCOL_VERSION_UNSUPPORTED",
            Self::InvalidIdentifier => "REALTIME_IDENTIFIER_INVALID",
            Self::InvalidSequence => "REALTIME_SEQUENCE_INVALID",
            Self::InvalidResumeToken => "REALTIME_RESUME_TOKEN_INVALID",
        }
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProtocolError {}

pub(crate) fn validate_identifier(value: &str) -> Result<(), ProtocolError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(ProtocolError::InvalidIdentifier);
    }
    Ok(())
}
