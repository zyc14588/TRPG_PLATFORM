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

fn validate_scenario(
    document: ScenarioDocument,
) -> Result<ValidatedScenario, CharacterScenarioError> {
    let metadata = &document.metadata;
    for (field, value) in [
        ("metadata.scenario_id", metadata.scenario_id.as_str()),
        ("metadata.title", metadata.title.as_str()),
        ("metadata.version", metadata.version.as_str()),
        (
            "metadata.recommended_players",
            metadata.recommended_players.as_str(),
        ),
        ("metadata.era", metadata.era.as_str()),
        (
            "metadata.copyright_status",
            metadata.copyright_status.as_str(),
        ),
        (
            "keeper_truth.summary",
            document.keeper_truth.summary.as_str(),
        ),
    ] {
        if value.trim().is_empty() {
            return Err(CharacterScenarioError::InvalidScenarioField(field));
        }
    }
    if metadata.ruleset_id != "coc7" {
        return Err(CharacterScenarioError::InvalidScenarioField(
            "metadata.ruleset_id",
        ));
    }
    if metadata.safety_notes.is_empty() {
        return Err(CharacterScenarioError::InvalidScenarioField(
            "metadata.safety_notes",
        ));
    }
    if document.keeper_truth.visibility != "keeper_only" {
        return Err(CharacterScenarioError::InvalidScenarioField(
            "keeper_truth.visibility",
        ));
    }
    if document.scenes.is_empty() {
        return Err(CharacterScenarioError::InvalidScenarioField("scenes"));
    }
    if document.clues.is_empty() {
        return Err(CharacterScenarioError::InvalidScenarioField("clues"));
    }
    if document.endings.is_empty() {
        return Err(CharacterScenarioError::InvalidScenarioField("endings"));
    }

    let mut scene_ids = HashSet::new();
    for scene in &document.scenes {
        if scene.id.trim().is_empty()
            || scene.name.trim().is_empty()
            || scene.atmosphere.trim().is_empty()
        {
            return Err(CharacterScenarioError::InvalidScenarioField("scene"));
        }
        if !scene_ids.insert(scene.id.clone()) {
            return Err(CharacterScenarioError::DuplicateScenarioId("scene"));
        }
    }

    let mut encounter_ids = HashSet::new();
    for encounter in &document.encounters {
        let unique_participants = encounter
            .participants
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        if encounter.id.trim().is_empty()
            || !encounter_ids.insert(encounter.id.clone())
            || !matches!(encounter.encounter_type.as_str(), "combat" | "chase")
            || !scene_ids.contains(&encounter.scene_id)
            || encounter.participants.len() < 2
            || unique_participants.len() != encounter.participants.len()
            || encounter
                .participants
                .iter()
                .any(|participant| participant.trim().is_empty())
            || (encounter.encounter_type == "chase"
                && !encounter
                    .initial_range
                    .is_some_and(|range| (1..=4).contains(&range)))
            || (encounter.encounter_type == "combat" && encounter.initial_range.is_some())
        {
            return Err(CharacterScenarioError::InvalidScenarioField("encounters"));
        }
    }

    let mut ending_ids = HashSet::new();
    for ending in &document.endings {
        if ending.id.trim().is_empty()
            || ending.id != ending.id.trim()
            || ending.condition.trim().is_empty()
            || !ending_ids.insert(ending.id.clone())
            || ending.growth_awards.iter().any(|award| {
                award.skill_name.trim().is_empty()
                    || award.skill_name != award.skill_name.trim()
                    || award.reason.trim().is_empty()
            })
        {
            return Err(CharacterScenarioError::InvalidScenarioField("endings"));
        }
    }
    for scene in &document.scenes {
        for exit in &scene.exits {
            if !scene_ids.contains(exit) {
                return Err(CharacterScenarioError::UnknownSceneExit(exit.clone()));
            }
        }
    }

    let mut clue_ids = HashSet::new();
    for clue in &document.clues {
        if clue.id.trim().is_empty() || !clue_ids.insert(clue.id.clone()) {
            return Err(CharacterScenarioError::DuplicateScenarioId("clue"));
        }
        if clue.clue_type == "core" && clue.acquisition.len() < 2 {
            return Err(CharacterScenarioError::CoreClueHasSinglePath(
                clue.id.clone(),
            ));
        }
    }
    for scene in &document.scenes {
        if scene.clues.iter().any(|clue| !clue_ids.contains(clue)) {
            return Err(CharacterScenarioError::InvalidScenarioField("scene.clues"));
        }
    }

    let canonical_value =
        serde_json::to_value(&document).map_err(|_| CharacterScenarioError::Serialization)?;
    let canonical_json = serde_json::to_string(&canonical_value)
        .map_err(|_| CharacterScenarioError::Serialization)?;
    let content_hash = format!("sha256:{:x}", Sha256::digest(canonical_json.as_bytes()));
    Ok(ValidatedScenario {
        scenario_id: metadata.scenario_id.clone(),
        ruleset_id: metadata.ruleset_id.clone(),
        format_version: metadata.version.clone(),
        content_hash,
        canonical_json,
        opening_scene_id: document.scenes[0].id.clone(),
        scene_ids: document
            .scenes
            .iter()
            .map(|scene| scene.id.clone())
            .collect(),
        encounter_ids: document
            .encounters
            .iter()
            .map(|encounter| encounter.id.clone())
            .collect(),
        ending_ids: document
            .endings
            .iter()
            .map(|ending| ending.id.clone())
            .collect(),
        growth_skills: document
            .endings
            .iter()
            .flat_map(|ending| ending.growth_awards.iter())
            .map(|award| award.skill_name.clone())
            .collect(),
    })
}

pub fn derive_character_stats(characteristics: Coc7Characteristics) -> KernelResult<DerivedStats> {
    let values = [
        characteristics.strength,
        characteristics.dexterity,
        characteristics.power,
        characteristics.constitution,
        characteristics.size,
        characteristics.appearance,
        characteristics.intelligence,
        characteristics.education,
        characteristics.luck,
    ];
    if values.iter().any(|value| *value == 0 || *value > 100) {
        return Err(TrpgError::InvalidConfiguration("characteristic_range"));
    }

    let physical_total = characteristics.strength as u16 + characteristics.size as u16;
    let (damage_bonus, build) = match physical_total {
        2..=64 => (DamageBonus::MinusTwo, -2),
        65..=84 => (DamageBonus::MinusOne, -1),
        85..=124 => (DamageBonus::None, 0),
        125..=164 => (DamageBonus::PlusD4, 1),
        _ => (DamageBonus::PlusD6, 2),
    };
    let movement_rate = match (
        characteristics.strength > characteristics.size,
        characteristics.dexterity > characteristics.size,
    ) {
        (true, true) => 9,
        (false, false) => 7,
        _ => 8,
    };

    Ok(DerivedStats {
        hit_points: ((characteristics.constitution as u16 + characteristics.size as u16) / 10)
            as u8,
        magic_points: characteristics.power / 5,
        sanity: characteristics.power,
        luck: characteristics.luck,
        movement_rate,
        damage_bonus,
        build,
    })
}

pub fn record_character_combat_san_chase_decision<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    track: CharacterCombatSanChaseTrack,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        trpg_contracts::EventType::Coc7CharacterTrackRecorded.name(),
        "character_combat_san_chase",
        format!("track={:?}", track),
    )
}
