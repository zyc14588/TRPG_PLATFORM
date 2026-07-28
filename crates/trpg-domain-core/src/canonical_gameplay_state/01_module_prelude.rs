use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use trpg_shared_kernel::{ServerDamageRoll, ServerPercentileRoll};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalGameplayStateError {
    InvalidJson,
    InvalidShape,
    InvalidTransition,
}

impl fmt::Display for CanonicalGameplayStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson => formatter.write_str("CANONICAL_GAMEPLAY_STATE_JSON_INVALID"),
            Self::InvalidShape => formatter.write_str("CANONICAL_GAMEPLAY_STATE_SHAPE_INVALID"),
            Self::InvalidTransition => {
                formatter.write_str("CANONICAL_GAMEPLAY_STATE_TRANSITION_INVALID")
            }
        }
    }
}

impl Error for CanonicalGameplayStateError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CombatCondition {
    Able,
    MajorWound,
    Dying,
    Dead,
}

impl CombatCondition {
    const fn can_act(self) -> bool {
        matches!(self, Self::Able | Self::MajorWound)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CombatStatus {
    Ongoing,
    Ended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SuccessLevel {
    Critical,
    Extreme,
    Hard,
    Regular,
    Failure,
    Fumble,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CombatActionKind {
    Melee,
    Firearm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CombatDefense {
    None,
    Dodge,
    FightBack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CombatExchangeOutcome {
    AttackerHit,
    DefenderFoughtBack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CombatSkillTargets {
    melee: u8,
    firearm: u8,
    dodge: u8,
    first_aid: u8,
    medicine: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CombatDamageFormula {
    dice_count: u8,
    die_sides: u8,
    flat_bonus: i8,
}

impl CombatDamageFormula {
    const fn is_valid(self) -> bool {
        let minimum = self.dice_count as i16 + self.flat_bonus as i16;
        let maximum = self.dice_count as i16 * self.die_sides as i16 + self.flat_bonus as i16;
        self.dice_count >= 1
            && self.dice_count <= 10
            && self.die_sides >= 2
            && self.die_sides <= 100
            && self.flat_bonus >= -20
            && self.flat_bonus <= 20
            && minimum >= 0
            && maximum <= u8::MAX as i16
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CombatWeapon {
    weapon_id: String,
    damage_formula: CombatDamageFormula,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CombatWeaponLoadout {
    melee: CombatWeapon,
    firearm: CombatWeapon,
}

impl CombatWeaponLoadout {
    const fn damage_formula(&self, action: CombatActionKind) -> CombatDamageFormula {
        match action {
            CombatActionKind::Melee => self.melee.damage_formula,
            CombatActionKind::Firearm => self.firearm.damage_formula,
        }
    }

    fn is_valid(&self) -> bool {
        valid_id(&self.melee.weapon_id)
            && self.melee.damage_formula.is_valid()
            && valid_id(&self.firearm.weapon_id)
            && self.firearm.damage_formula.is_valid()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CombatMedicalSkill {
    FirstAid,
    Medicine,
}

impl CombatSkillTargets {
    const fn attack_target(self, action: CombatActionKind) -> u8 {
        match action {
            CombatActionKind::Melee => self.melee,
            CombatActionKind::Firearm => self.firearm,
        }
    }

    const fn defense_target(self, defense: CombatDefense) -> Option<u8> {
        match defense {
            CombatDefense::None => None,
            CombatDefense::Dodge => Some(self.dodge),
            CombatDefense::FightBack => Some(self.melee),
        }
    }

    const fn medical_target(self, skill: CombatMedicalSkill) -> u8 {
        match skill {
            CombatMedicalSkill::FirstAid => self.first_aid,
            CombatMedicalSkill::Medicine => self.medicine,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PercentileRollEvidence {
    roll_id: String,
    target: u8,
    roll: u8,
    selected_tens_digit: u8,
    ones_digit: u8,
    success_level: SuccessLevel,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DamageRollEvidence {
    roll_id: String,
    dice_count: u8,
    die_sides: u8,
    flat_bonus: i8,
    dice_values: Vec<u8>,
    raw_damage: u8,
}

impl CombatStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ongoing => "ONGOING",
            Self::Ended => "ENDED",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
enum CombatMutation {
    Started,
    AttackMissed {
        attacker_id: String,
        target_id: String,
        action: CombatActionKind,
        defense: CombatDefense,
        attacker_roll: PercentileRollEvidence,
        defender_roll: Option<PercentileRollEvidence>,
    },
    DamageApplied {
        attacker_id: String,
        target_id: String,
        action: CombatActionKind,
        defense: CombatDefense,
        outcome: CombatExchangeOutcome,
        attacker_roll: PercentileRollEvidence,
        defender_roll: Option<PercentileRollEvidence>,
        damage_roll: DamageRollEvidence,
        raw_damage: u8,
    },
    MajorWoundRecoveryAttempted {
        healer_id: String,
        target_id: String,
        medical_skill: CombatMedicalSkill,
        medical_roll: PercentileRollEvidence,
        recovered: bool,
    },
    TurnAdvanced,
    Ended,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Combatant {
    participant_id: String,
    dexterity: u8,
    skill_targets: CombatSkillTargets,
    weapon_loadout: CombatWeaponLoadout,
    current_hp: u8,
    max_hp: u8,
    armor: u8,
    condition: CombatCondition,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CombatSnapshot {
    combat_id: String,
    participants: Vec<Combatant>,
    initiative_order: Vec<String>,
    round: u32,
    current_turn_index: usize,
    turn_action_consumed: bool,
    consumed_roll_ids: Vec<String>,
    status: CombatStatus,
    version: u64,
    last_transition: CombatMutation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedCombatState {
    combat_id: String,
    status: &'static str,
    round: u32,
    current_turn_index: usize,
    version: u64,
}

impl ValidatedCombatState {
    pub fn combat_id(&self) -> &str {
        &self.combat_id
    }

    pub const fn status(&self) -> &'static str {
        self.status
    }

    pub const fn round(&self) -> u32 {
        self.round
    }

    pub const fn current_turn_index(&self) -> usize {
        self.current_turn_index
    }

    pub const fn version(&self) -> u64 {
        self.version
    }
}

pub fn validate_combat_state_transition(
    previous_state_json: Option<&str>,
    next_state_json: &str,
) -> Result<ValidatedCombatState, CanonicalGameplayStateError> {
    let next = parse_combat(next_state_json)?;
    let expected = if let Some(previous_state_json) = previous_state_json {
        let mut previous = parse_combat(previous_state_json)?;
        apply_combat_mutation(&mut previous, &next.last_transition)?;
        previous
    } else {
        if next.version != 1
            || next.round != 1
            || next.current_turn_index != 0
            || next.turn_action_consumed
            || !next.consumed_roll_ids.is_empty()
            || next.status != CombatStatus::Ongoing
            || !matches!(next.last_transition, CombatMutation::Started)
        {
            return Err(CanonicalGameplayStateError::InvalidTransition);
        }
        next.clone()
    };
    if expected != next {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    Ok(combat_summary(next))
}

pub fn inspect_combat_state(
    state_json: &str,
) -> Result<ValidatedCombatState, CanonicalGameplayStateError> {
    parse_combat(state_json).map(combat_summary)
}
