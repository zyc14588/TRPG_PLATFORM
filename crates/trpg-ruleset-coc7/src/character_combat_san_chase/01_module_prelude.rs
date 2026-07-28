use crate::{append_coc7_event, Coc7EventPayload};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt;
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterCombatSanChaseTrack {
    Character,
    Combat,
    Sanity,
    Chase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageBonus {
    MinusTwo,
    MinusOne,
    None,
    PlusD4,
    PlusD6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Coc7Characteristics {
    pub strength: u8,
    pub dexterity: u8,
    pub power: u8,
    pub constitution: u8,
    pub size: u8,
    pub appearance: u8,
    pub intelligence: u8,
    pub education: u8,
    pub luck: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Coc7CharacterSheet {
    pub name: String,
    pub age: u8,
    pub occupation: String,
    pub era: String,
    pub birthplace: String,
    pub characteristics: Coc7Characteristics,
    pub skills: BTreeMap<String, u8>,
    pub backstory_anchors: Vec<String>,
}

impl Coc7CharacterSheet {
    pub fn validate(&self) -> Result<DerivedStats, CharacterScenarioError> {
        for (field, value) in [
            ("name", self.name.as_str()),
            ("occupation", self.occupation.as_str()),
            ("era", self.era.as_str()),
            ("birthplace", self.birthplace.as_str()),
        ] {
            if value.trim().is_empty() || value.len() > 256 {
                return Err(CharacterScenarioError::InvalidCharacterField(field));
            }
        }
        if !(15..=100).contains(&self.age) {
            return Err(CharacterScenarioError::InvalidCharacterField("age"));
        }
        if self.skills.is_empty()
            || self
                .skills
                .iter()
                .any(|(name, value)| name.trim().is_empty() || *value > 100)
        {
            return Err(CharacterScenarioError::InvalidCharacterField("skills"));
        }
        if self.backstory_anchors.len() < 2
            || self
                .backstory_anchors
                .iter()
                .any(|anchor| anchor.trim().is_empty())
        {
            return Err(CharacterScenarioError::InvalidCharacterField(
                "backstory_anchors",
            ));
        }
        derive_character_stats(self.characteristics)
            .map_err(|_| CharacterScenarioError::InvalidCharacterField("characteristics"))
    }

    pub fn to_canonical_json(&self) -> Result<String, CharacterScenarioError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|_| CharacterScenarioError::Serialization)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DerivedStats {
    pub hit_points: u8,
    pub magic_points: u8,
    pub sanity: u8,
    pub luck: u8,
    pub movement_rate: u8,
    pub damage_bonus: DamageBonus,
    pub build: i8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CharacterScenarioError {
    Parse,
    Serialization,
    InvalidCharacterField(&'static str),
    InvalidScenarioField(&'static str),
    DuplicateScenarioId(&'static str),
    UnknownSceneExit(String),
    CoreClueHasSinglePath(String),
}

impl fmt::Display for CharacterScenarioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse => formatter.write_str("COC7_SCENARIO_PARSE_FAILED"),
            Self::Serialization => formatter.write_str("COC7_SERIALIZATION_FAILED"),
            Self::InvalidCharacterField(field) => {
                write!(formatter, "COC7_CHARACTER_FIELD_INVALID:{field}")
            }
            Self::InvalidScenarioField(field) => {
                write!(formatter, "COC7_SCENARIO_FIELD_INVALID:{field}")
            }
            Self::DuplicateScenarioId(kind) => {
                write!(formatter, "COC7_SCENARIO_DUPLICATE_ID:{kind}")
            }
            Self::UnknownSceneExit(scene) => {
                write!(formatter, "COC7_SCENARIO_UNKNOWN_SCENE_EXIT:{scene}")
            }
            Self::CoreClueHasSinglePath(clue) => {
                write!(formatter, "COC7_SCENARIO_CORE_CLUE_SINGLE_PATH:{clue}")
            }
        }
    }
}

impl Error for CharacterScenarioError {}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ScenarioDocument {
    pub metadata: ScenarioMetadata,
    pub keeper_truth: KeeperTruth,
    pub scenes: Vec<ScenarioScene>,
    #[serde(default)]
    pub npcs: Vec<serde_json::Value>,
    pub clues: Vec<ScenarioClue>,
    #[serde(default)]
    pub timeline: Vec<serde_json::Value>,
    #[serde(default)]
    pub encounters: Vec<ScenarioEncounter>,
    pub endings: Vec<ScenarioEnding>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ScenarioMetadata {
    pub scenario_id: String,
    pub title: String,
    pub ruleset_id: String,
    pub version: String,
    pub recommended_players: String,
    pub era: String,
    pub copyright_status: String,
    pub safety_notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeeperTruth {
    pub summary: String,
    pub visibility: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ScenarioScene {
    pub id: String,
    pub name: String,
    pub time: String,
    pub atmosphere: String,
    #[serde(default)]
    pub visible_npcs: Vec<String>,
    #[serde(default)]
    pub investigable_objects: Vec<serde_json::Value>,
    #[serde(default)]
    pub dangers: Vec<String>,
    #[serde(default)]
    pub clues: Vec<String>,
    #[serde(default)]
    pub exits: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ScenarioClue {
    pub id: String,
    #[serde(rename = "type")]
    pub clue_type: String,
    pub state: String,
    pub visibility: String,
    #[serde(default)]
    pub acquisition: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub sanity_check: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ScenarioEnding {
    pub id: String,
    pub condition: String,
    #[serde(default)]
    pub growth_awards: Vec<ScenarioGrowthAward>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ScenarioGrowthAward {
    pub skill_name: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ScenarioEncounter {
    pub id: String,
    #[serde(rename = "type")]
    pub encounter_type: String,
    pub scene_id: String,
    pub participants: Vec<String>,
    #[serde(default)]
    pub initial_range: Option<i8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedScenario {
    pub scenario_id: String,
    pub ruleset_id: String,
    pub format_version: String,
    pub content_hash: String,
    pub canonical_json: String,
    pub opening_scene_id: String,
    pub scene_ids: Vec<String>,
    pub encounter_ids: Vec<String>,
    pub ending_ids: Vec<String>,
    pub growth_skills: Vec<String>,
}

pub fn parse_scenario_yaml(source: &str) -> Result<ValidatedScenario, CharacterScenarioError> {
    let document: ScenarioDocument =
        serde_yaml::from_str(source).map_err(|_| CharacterScenarioError::Parse)?;
    validate_scenario(document)
}

pub fn parse_scenario_json(source: &str) -> Result<ValidatedScenario, CharacterScenarioError> {
    let document: ScenarioDocument =
        serde_json::from_str(source).map_err(|_| CharacterScenarioError::Parse)?;
    validate_scenario(document)
}
