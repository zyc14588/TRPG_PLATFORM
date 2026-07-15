use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use serde_json::{json, Value};

use crate::WireErrorCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventType {
    CampaignCreated,
    AuthorityContractLocked,
    CharacterSheetSubmitted,
    CharacterSheetVersionLocked,
    DiceRolled,
    ClueRevealed,
    SessionSummaryCreated,
    SkillCheckResolved,
    SanityLossApplied,
    CombatStateUpdated,
    ChaseSegmentResolved,
    Coc7CharacterTrackRecorded,
    Coc7GovernanceProfileRecorded,
    Coc7RuntimeGovernanceRecorded,
    Coc7NpcDecisionRecorded,
    Coc7ReadmeContractRecorded,
    Coc7RuleRuntimeDecisionRecorded,
    Coc7RulesetPackLoaded,
    Coc7RulesDispatchRecorded,
    Coc7RulesetPackSdkRegistered,
    Coc7SanityTransitionRecorded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EventDescriptor {
    pub event_type: EventType,
    pub name: &'static str,
    pub schema_version: u16,
    pub schema_id: &'static str,
    pub projection: &'static str,
}

macro_rules! define_canonical_events {
    ($($variant:ident => $descriptor:ident {
        name: $name:literal,
        version: $version:literal,
        schema: $schema:literal,
        projection: $projection:literal
    }),+ $(,)?) => {
        $(
            pub const $descriptor: EventDescriptor = EventDescriptor {
                event_type: EventType::$variant,
                name: $name,
                schema_version: $version,
                schema_id: $schema,
                projection: $projection,
            };
        )+

        pub const EVENT_REGISTRY: &[EventDescriptor] = &[$($descriptor),+];

        impl EventType {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn descriptor(self) -> &'static EventDescriptor {
                match self {
                    $(Self::$variant => &$descriptor),+
                }
            }

            pub const fn name(self) -> &'static str {
                self.descriptor().name
            }

            pub const fn schema_version(self) -> u16 {
                self.descriptor().schema_version
            }
        }
    };
}

define_canonical_events! {
    CampaignCreated => CAMPAIGN_CREATED {
        name: "CampaignCreated",
        version: 1,
        schema: "trpg.events.campaign_created.v1",
        projection: "campaign_projection"
    },
    AuthorityContractLocked => AUTHORITY_CONTRACT_LOCKED {
        name: "AuthorityContractLocked",
        version: 1,
        schema: "trpg.events.authority_contract_locked.v1",
        projection: "campaign_projection"
    },
    CharacterSheetSubmitted => CHARACTER_SHEET_SUBMITTED {
        name: "CharacterSheetSubmitted",
        version: 1,
        schema: "trpg.events.character_sheet_submitted.v1",
        projection: "character_projection"
    },
    CharacterSheetVersionLocked => CHARACTER_SHEET_VERSION_LOCKED {
        name: "CharacterSheetVersionLocked",
        version: 1,
        schema: "trpg.events.character_sheet_version_locked.v1",
        projection: "character_projection"
    },
    DiceRolled => DICE_ROLLED {
        name: "DiceRolled",
        version: 1,
        schema: "trpg.events.dice_rolled.v1",
        projection: "dice_projection"
    },
    ClueRevealed => CLUE_REVEALED {
        name: "ClueRevealed",
        version: 1,
        schema: "trpg.events.clue_revealed.v1",
        projection: "clue_projection"
    },
    SessionSummaryCreated => SESSION_SUMMARY_CREATED {
        name: "SessionSummaryCreated",
        version: 1,
        schema: "trpg.events.session_summary_created.v1",
        projection: "session_projection"
    },
    SkillCheckResolved => SKILL_CHECK_RESOLVED {
        name: "SkillCheckResolved",
        version: 1,
        schema: "trpg.events.skill_check_resolved.v1",
        projection: "rules_projection"
    },
    SanityLossApplied => SANITY_LOSS_APPLIED {
        name: "SanityLossApplied",
        version: 1,
        schema: "trpg.events.sanity_loss_applied.v1",
        projection: "character_projection"
    },
    CombatStateUpdated => COMBAT_STATE_UPDATED {
        name: "CombatStateUpdated",
        version: 1,
        schema: "trpg.events.combat_state_updated.v1",
        projection: "combat_projection"
    },
    ChaseSegmentResolved => CHASE_SEGMENT_RESOLVED {
        name: "ChaseSegmentResolved",
        version: 1,
        schema: "trpg.events.chase_segment_resolved.v1",
        projection: "chase_projection"
    },
    Coc7CharacterTrackRecorded => COC7_CHARACTER_TRACK_RECORDED {
        name: "coc7.character_combat_san_chase_recorded",
        version: 1,
        schema: "trpg.events.coc7_character_track_recorded.v1",
        projection: "rules_projection"
    },
    Coc7GovernanceProfileRecorded => COC7_GOVERNANCE_PROFILE_RECORDED {
        name: "coc7.governance_profile_recorded",
        version: 1,
        schema: "trpg.events.coc7_governance_profile_recorded.v1",
        projection: "rules_projection"
    },
    Coc7RuntimeGovernanceRecorded => COC7_RUNTIME_GOVERNANCE_RECORDED {
        name: "coc7.runtime_governance_recorded",
        version: 1,
        schema: "trpg.events.coc7_runtime_governance_recorded.v1",
        projection: "rules_projection"
    },
    Coc7NpcDecisionRecorded => COC7_NPC_DECISION_RECORDED {
        name: "coc7.npc_decision_recorded",
        version: 1,
        schema: "trpg.events.coc7_npc_decision_recorded.v1",
        projection: "npc_projection"
    },
    Coc7ReadmeContractRecorded => COC7_README_CONTRACT_RECORDED {
        name: "coc7.readme_contract_recorded",
        version: 1,
        schema: "trpg.events.coc7_readme_contract_recorded.v1",
        projection: "rules_projection"
    },
    Coc7RuleRuntimeDecisionRecorded => COC7_RULE_RUNTIME_DECISION_RECORDED {
        name: "coc7.rule_runtime_decision_recorded",
        version: 1,
        schema: "trpg.events.coc7_rule_runtime_decision_recorded.v1",
        projection: "rules_projection"
    },
    Coc7RulesetPackLoaded => COC7_RULESET_PACK_LOADED {
        name: "coc7.ruleset_pack_loaded",
        version: 1,
        schema: "trpg.events.coc7_ruleset_pack_loaded.v1",
        projection: "rules_projection"
    },
    Coc7RulesDispatchRecorded => COC7_RULES_DISPATCH_RECORDED {
        name: "coc7.rules_dispatch_recorded",
        version: 1,
        schema: "trpg.events.coc7_rules_dispatch_recorded.v1",
        projection: "rules_projection"
    },
    Coc7RulesetPackSdkRegistered => COC7_RULESET_PACK_SDK_REGISTERED {
        name: "coc7.ruleset_pack_sdk_registered",
        version: 1,
        schema: "trpg.events.coc7_ruleset_pack_sdk_registered.v1",
        projection: "rules_projection"
    },
    Coc7SanityTransitionRecorded => COC7_SANITY_TRANSITION_RECORDED {
        name: "coc7.sanity_transition_recorded",
        version: 1,
        schema: "trpg.events.coc7_sanity_transition_recorded.v1",
        projection: "character_projection"
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalEventHeader {
    pub event_type: EventType,
    pub name: &'static str,
    pub schema_version: u16,
    pub schema_id: &'static str,
}

impl CanonicalEventHeader {
    pub const fn new(event_type: EventType) -> Self {
        let descriptor = event_type.descriptor();
        Self {
            event_type,
            name: descriptor.name,
            schema_version: descriptor.schema_version,
            schema_id: descriptor.schema_id,
        }
    }

    pub fn parse(name: &str, schema_version: u16) -> Result<Self, EventContractError> {
        let header = Self::resolve(name)?;
        if header.schema_version != schema_version {
            return Err(EventContractError::version_mismatch(
                name,
                header.schema_version,
                schema_version,
            ));
        }
        Ok(header)
    }

    pub fn resolve(name: &str) -> Result<Self, EventContractError> {
        let descriptor = EVENT_REGISTRY
            .iter()
            .find(|descriptor| descriptor.name == name)
            .ok_or_else(|| EventContractError::unknown(name))?;
        Ok(Self::new(descriptor.event_type))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventContractError {
    pub code: WireErrorCode,
    pub detail: String,
}

impl EventContractError {
    fn unknown(name: &str) -> Self {
        Self {
            code: WireErrorCode::EventContractUnknown,
            detail: format!("unregistered event type: {name}"),
        }
    }

    fn version_mismatch(name: &str, expected: u16, actual: u16) -> Self {
        Self {
            code: WireErrorCode::EventContractVersionMismatch,
            detail: format!(
                "event schema version mismatch for {name}: expected {expected}, got {actual}"
            ),
        }
    }
}

impl fmt::Display for EventContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl Error for EventContractError {}

pub const fn canonical_event_registry() -> &'static [EventDescriptor] {
    EVENT_REGISTRY
}

pub fn validate_event_registry() -> Result<(), EventContractError> {
    let mut names = HashSet::new();
    let mut schema_ids = HashSet::new();
    for descriptor in EVENT_REGISTRY {
        if descriptor.schema_version == 0
            || descriptor.name.is_empty()
            || descriptor.schema_id.is_empty()
            || descriptor.projection.is_empty()
            || !names.insert(descriptor.name)
            || !schema_ids.insert(descriptor.schema_id)
        {
            return Err(EventContractError {
                code: WireErrorCode::EventContractVersionMismatch,
                detail: format!("invalid or duplicate event descriptor: {}", descriptor.name),
            });
        }
    }
    Ok(())
}

pub fn canonical_event_schema() -> Value {
    let variants = EVENT_REGISTRY
        .iter()
        .map(|descriptor| {
            json!({
                "$id": descriptor.schema_id,
                "type": "object",
                "required": ["type", "schema_version"],
                "properties": {
                    "type": {"const": descriptor.name},
                    "schema_version": {"const": descriptor.schema_version}
                },
                "additionalProperties": true
            })
        })
        .collect::<Vec<_>>();
    json!({
        "$id": "trpg.events.registry.v1",
        "oneOf": variants
    })
}

pub fn canonical_openapi_components() -> Value {
    let events = EVENT_REGISTRY
        .iter()
        .map(|descriptor| {
            json!({
                "name": descriptor.name,
                "schema_version": descriptor.schema_version,
                "schema_id": descriptor.schema_id,
                "projection": descriptor.projection
            })
        })
        .collect::<Vec<_>>();
    json!({
        "EventEnvelope": {
            "discriminator": {"propertyName": "type"},
            "events": events
        }
    })
}
