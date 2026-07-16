use serde_json::Value;
use std::fmt;

pub const CURRENT_EVENT_SCHEMA_VERSION: i32 = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct UpcastedEventPayload {
    pub event_schema_version: i32,
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventUpcastError {
    InvalidVersion(i32),
    UnknownFutureVersion { event_type: String, version: i32 },
    MissingUpcaster { event_type: String, version: i32 },
}

impl fmt::Display for EventUpcastError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidVersion(version) => {
                write!(formatter, "invalid event schema version {version}")
            }
            Self::UnknownFutureVersion {
                event_type,
                version,
            } => write!(
                formatter,
                "unknown future schema version {version} for event {event_type}"
            ),
            Self::MissingUpcaster {
                event_type,
                version,
            } => write!(
                formatter,
                "missing upcaster from schema version {version} for event {event_type}"
            ),
        }
    }
}

impl std::error::Error for EventUpcastError {}

type UpcastStep = fn(Value) -> Value;

#[derive(Clone, Copy, Debug)]
struct RegisteredUpcast {
    from_version: i32,
    to_version: i32,
    apply: UpcastStep,
}

fn lossless_envelope_v1_to_v2(payload: Value) -> Value {
    payload
}

const CANONICAL_UPCASTS: &[RegisteredUpcast] = &[RegisteredUpcast {
    from_version: 1,
    to_version: 2,
    apply: lossless_envelope_v1_to_v2,
}];

/// Registry-backed replay boundary for persisted event JSON.
///
/// Version 1 is the pre-P03 envelope.  P03 adds campaign/stream scope,
/// request-hash binding, and an explicit payload version while preserving the
/// payload's JSON meaning, so its v1 -> v2 step is intentionally lossless.
#[derive(Clone, Copy, Debug)]
pub struct EventPayloadUpcaster {
    steps: &'static [RegisteredUpcast],
}

impl Default for EventPayloadUpcaster {
    fn default() -> Self {
        Self::canonical()
    }
}

impl EventPayloadUpcaster {
    pub const fn canonical() -> Self {
        Self {
            steps: CANONICAL_UPCASTS,
        }
    }

    pub fn upcast(
        &self,
        event_type: &str,
        from_version: i32,
        payload: Value,
    ) -> Result<UpcastedEventPayload, EventUpcastError> {
        if from_version <= 0 {
            return Err(EventUpcastError::InvalidVersion(from_version));
        }
        if from_version > CURRENT_EVENT_SCHEMA_VERSION {
            return Err(EventUpcastError::UnknownFutureVersion {
                event_type: event_type.to_owned(),
                version: from_version,
            });
        }

        let mut version = from_version;
        let mut payload = payload;
        while version < CURRENT_EVENT_SCHEMA_VERSION {
            let step = self
                .steps
                .iter()
                .find(|step| step.from_version == version && step.to_version == version + 1)
                .ok_or_else(|| EventUpcastError::MissingUpcaster {
                    event_type: event_type.to_owned(),
                    version,
                })?;
            payload = (step.apply)(payload);
            version = step.to_version;
        }

        Ok(UpcastedEventPayload {
            event_schema_version: version,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_registry_applies_each_version_step() {
        let result = EventPayloadUpcaster::canonical()
            .upcast("CampaignStarted", 1, json!({"legacy": true}))
            .unwrap();
        assert_eq!(result.event_schema_version, CURRENT_EVENT_SCHEMA_VERSION);
        assert_eq!(result.payload, json!({"legacy": true}));
    }

    #[test]
    fn missing_registry_step_fails_closed() {
        let registry = EventPayloadUpcaster { steps: &[] };
        assert!(matches!(
            registry.upcast("CampaignStarted", 1, Value::Null),
            Err(EventUpcastError::MissingUpcaster { version: 1, .. })
        ));
    }
}
