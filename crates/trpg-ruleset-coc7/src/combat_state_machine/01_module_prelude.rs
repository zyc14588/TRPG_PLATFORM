use crate::dice_roll_contract::{success_level, ServerDiceRoll, SuccessLevel};
use crate::{append_coc7_event, Coc7EventPayload};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use trpg_contracts::EventType;
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, ServerDamageRoll,
    ServerPercentileRoll, TrpgError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CombatCondition {
    Able,
    MajorWound,
    Dying,
    Dead,
}

impl CombatCondition {
    pub const fn can_act(self) -> bool {
        matches!(self, Self::Able | Self::MajorWound)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CombatStatus {
    Ongoing,
    Ended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CombatActionKind {
    Melee,
    Firearm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CombatDefense {
    None,
    Dodge,
    FightBack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CombatMedicalSkill {
    FirstAid,
    Medicine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CombatExchangeOutcome {
    AttackerHit,
    DefenderFoughtBack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttackResolution {
    pub action: CombatActionKind,
    pub defense: CombatDefense,
    pub attacker_success: SuccessLevel,
    pub defender_success: Option<SuccessLevel>,
    pub hit: bool,
    pub counterattack: bool,
    pub outcome: Option<CombatExchangeOutcome>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CombatTransition {
    pub before_hp: u8,
    pub after_hp: u8,
    pub raw_damage: u8,
    pub armor_absorbed: u8,
    pub damage: u8,
    pub prior_condition: CombatCondition,
    pub condition: CombatCondition,
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

impl PercentileRollEvidence {
    fn from_server_roll(target: u8, roll: &ServerPercentileRoll) -> KernelResult<Self> {
        Ok(Self {
            roll_id: roll.roll_id().to_owned(),
            target,
            roll: roll.value(),
            selected_tens_digit: roll.selected_tens_digit(),
            ones_digit: roll.ones_digit(),
            success_level: success_level(roll.value(), target)?,
        })
    }

    fn validate(&self, expected_target: u8) -> KernelResult<()> {
        if self.selected_tens_digit > 9 || self.ones_digit > 9 {
            return Err(TrpgError::InvalidConfiguration(
                "combat_percentile_evidence",
            ));
        }
        let reconstructed = if self.selected_tens_digit == 0 && self.ones_digit == 0 {
            100
        } else {
            self.selected_tens_digit * 10 + self.ones_digit
        };
        if !valid_combat_id(&self.roll_id)
            || self.target != expected_target
            || reconstructed != self.roll
            || success_level(self.roll, self.target)? != self.success_level
        {
            return Err(TrpgError::InvalidConfiguration(
                "combat_percentile_evidence",
            ));
        }
        Ok(())
    }
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

impl DamageRollEvidence {
    fn from_server_roll(roll: &ServerDamageRoll) -> Self {
        Self {
            roll_id: roll.roll_id().to_owned(),
            dice_count: roll.dice_count(),
            die_sides: roll.die_sides(),
            flat_bonus: roll.flat_bonus(),
            dice_values: roll.dice_values().to_vec(),
            raw_damage: roll.value(),
        }
    }

    fn validate(&self, expected_formula: CombatDamageFormula) -> KernelResult<()> {
        let total = self
            .dice_values
            .iter()
            .try_fold(i16::from(self.flat_bonus), |sum, value| {
                sum.checked_add(i16::from(*value))
            })
            .and_then(|value| u8::try_from(value).ok());
        if !valid_combat_id(&self.roll_id)
            || (self.dice_count, self.die_sides, self.flat_bonus)
                != (
                    expected_formula.dice_count,
                    expected_formula.die_sides,
                    expected_formula.flat_bonus,
                )
            || self.dice_values.len() != usize::from(self.dice_count)
            || self
                .dice_values
                .iter()
                .any(|value| *value == 0 || *value > self.die_sides)
            || total != Some(self.raw_damage)
        {
            return Err(TrpgError::InvalidConfiguration("combat_damage_evidence"));
        }
        Ok(())
    }
}

struct VerifiedAttackInput<'a> {
    attacker_id: &'a str,
    target_id: &'a str,
    action: CombatActionKind,
    defense: CombatDefense,
    attacker_roll: &'a PercentileRollEvidence,
    defender_roll: Option<&'a PercentileRollEvidence>,
    damage_roll: Option<&'a DamageRollEvidence>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombatSkillTargets {
    melee: u8,
    firearm: u8,
    dodge: u8,
    first_aid: u8,
    medicine: u8,
}

impl CombatSkillTargets {
    pub fn new(
        melee: u8,
        firearm: u8,
        dodge: u8,
        first_aid: u8,
        medicine: u8,
    ) -> KernelResult<Self> {
        if !(1..=100).contains(&melee)
            || !(1..=100).contains(&firearm)
            || !(1..=100).contains(&dodge)
            || !(1..=100).contains(&first_aid)
            || !(1..=100).contains(&medicine)
        {
            return Err(TrpgError::InvalidConfiguration("combat_skill_targets"));
        }
        Ok(Self {
            melee,
            firearm,
            dodge,
            first_aid,
            medicine,
        })
    }

    pub const fn melee(self) -> u8 {
        self.melee
    }

    pub const fn firearm(self) -> u8 {
        self.firearm
    }

    pub const fn dodge(self) -> u8 {
        self.dodge
    }

    pub const fn first_aid(self) -> u8 {
        self.first_aid
    }

    pub const fn medicine(self) -> u8 {
        self.medicine
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombatDamageFormula {
    dice_count: u8,
    die_sides: u8,
    flat_bonus: i8,
}

impl CombatDamageFormula {
    pub fn new(dice_count: u8, die_sides: u8, flat_bonus: i8) -> KernelResult<Self> {
        let minimum = i16::from(dice_count) + i16::from(flat_bonus);
        let maximum = i16::from(dice_count) * i16::from(die_sides) + i16::from(flat_bonus);
        if !(1..=10).contains(&dice_count)
            || !(2..=100).contains(&die_sides)
            || !(-20..=20).contains(&flat_bonus)
            || minimum < 0
            || maximum > i16::from(u8::MAX)
        {
            return Err(TrpgError::InvalidConfiguration("combat_damage_formula"));
        }
        Ok(Self {
            dice_count,
            die_sides,
            flat_bonus,
        })
    }

    pub const fn dice_count(self) -> u8 {
        self.dice_count
    }

    pub const fn die_sides(self) -> u8 {
        self.die_sides
    }

    pub const fn flat_bonus(self) -> i8 {
        self.flat_bonus
    }

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
pub struct CombatWeapon {
    weapon_id: String,
    damage_formula: CombatDamageFormula,
}

impl CombatWeapon {
    pub fn new(
        weapon_id: impl Into<String>,
        damage_formula: CombatDamageFormula,
    ) -> KernelResult<Self> {
        let weapon_id = weapon_id.into();
        if !valid_combat_id(&weapon_id) || !damage_formula.is_valid() {
            return Err(TrpgError::InvalidConfiguration("combat_weapon"));
        }
        Ok(Self {
            weapon_id,
            damage_formula,
        })
    }

    pub fn weapon_id(&self) -> &str {
        &self.weapon_id
    }

    pub const fn damage_formula(&self) -> CombatDamageFormula {
        self.damage_formula
    }

    fn is_valid(&self) -> bool {
        valid_combat_id(&self.weapon_id) && self.damage_formula.is_valid()
    }
}
