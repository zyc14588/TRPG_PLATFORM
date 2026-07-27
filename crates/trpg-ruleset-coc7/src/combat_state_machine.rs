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
        let reconstructed = if self.selected_tens_digit == 0 && self.ones_digit == 0 {
            100
        } else {
            self.selected_tens_digit * 10 + self.ones_digit
        };
        if !valid_combat_id(&self.roll_id)
            || self.target != expected_target
            || self.selected_tens_digit > 9
            || self.ones_digit > 9
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CombatWeaponLoadout {
    melee: CombatWeapon,
    firearm: CombatWeapon,
}

impl CombatWeaponLoadout {
    pub fn new(melee: CombatWeapon, firearm: CombatWeapon) -> KernelResult<Self> {
        if !melee.is_valid() || !firearm.is_valid() {
            return Err(TrpgError::InvalidConfiguration("combat_weapon_loadout"));
        }
        Ok(Self { melee, firearm })
    }

    pub fn melee(&self) -> &CombatWeapon {
        &self.melee
    }

    pub fn firearm(&self) -> &CombatWeapon {
        &self.firearm
    }

    const fn damage_formula(&self, action: CombatActionKind) -> CombatDamageFormula {
        match action {
            CombatActionKind::Melee => self.melee.damage_formula,
            CombatActionKind::Firearm => self.firearm.damage_formula,
        }
    }

    fn is_valid(&self) -> bool {
        self.melee.is_valid() && self.firearm.is_valid()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CombatantState {
    participant_id: String,
    dexterity: u8,
    skill_targets: CombatSkillTargets,
    weapon_loadout: CombatWeaponLoadout,
    current_hp: u8,
    max_hp: u8,
    armor: u8,
    condition: CombatCondition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatHealth {
    current_hp: u8,
    max_hp: u8,
    condition: CombatCondition,
}

impl CombatHealth {
    pub fn new(current_hp: u8, max_hp: u8, condition: CombatCondition) -> KernelResult<Self> {
        if max_hp == 0
            || current_hp > max_hp
            || (current_hp == 0)
                != matches!(condition, CombatCondition::Dying | CombatCondition::Dead)
        {
            return Err(TrpgError::InvalidConfiguration("combat_health"));
        }
        Ok(Self {
            current_hp,
            max_hp,
            condition,
        })
    }

    pub const fn current_hp(self) -> u8 {
        self.current_hp
    }

    pub const fn max_hp(self) -> u8 {
        self.max_hp
    }

    pub const fn condition(self) -> CombatCondition {
        self.condition
    }
}

impl CombatantState {
    /// Constructs an encounter participant from the campaign's persisted
    /// health snapshot; starting a new combat never implies healing.
    pub fn new(
        participant_id: impl Into<String>,
        dexterity: u8,
        health: CombatHealth,
        armor: u8,
        skill_targets: CombatSkillTargets,
        weapon_loadout: CombatWeaponLoadout,
    ) -> KernelResult<Self> {
        let participant_id = participant_id.into();
        if !valid_combat_id(&participant_id)
            || dexterity == 0
            || dexterity > 100
            || armor > 30
            || !weapon_loadout.is_valid()
        {
            return Err(TrpgError::InvalidConfiguration("combat_participant"));
        }
        Ok(Self {
            participant_id,
            dexterity,
            skill_targets,
            weapon_loadout,
            current_hp: health.current_hp,
            max_hp: health.max_hp,
            armor,
            condition: health.condition,
        })
    }

    pub fn participant_id(&self) -> &str {
        &self.participant_id
    }

    pub const fn dexterity(&self) -> u8 {
        self.dexterity
    }

    pub const fn skill_targets(&self) -> CombatSkillTargets {
        self.skill_targets
    }

    pub const fn weapon_loadout(&self) -> &CombatWeaponLoadout {
        &self.weapon_loadout
    }

    pub const fn current_hp(&self) -> u8 {
        self.current_hp
    }

    pub const fn max_hp(&self) -> u8 {
        self.max_hp
    }

    pub const fn armor(&self) -> u8 {
        self.armor
    }

    pub const fn condition(&self) -> CombatCondition {
        self.condition
    }

    pub const fn health(&self) -> CombatHealth {
        CombatHealth {
            current_hp: self.current_hp,
            max_hp: self.max_hp,
            condition: self.condition,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CombatState {
    combat_id: String,
    participants: Vec<CombatantState>,
    initiative_order: Vec<String>,
    round: u32,
    current_turn_index: usize,
    turn_action_consumed: bool,
    consumed_roll_ids: Vec<String>,
    status: CombatStatus,
    version: u64,
    last_transition: CombatMutation,
}

#[derive(Deserialize)]
struct CombatantStateWire {
    participant_id: String,
    dexterity: u8,
    skill_targets: CombatSkillTargets,
    weapon_loadout: CombatWeaponLoadout,
    current_hp: u8,
    max_hp: u8,
    armor: u8,
    condition: CombatCondition,
}

#[derive(Deserialize)]
struct CombatStateWire {
    combat_id: String,
    participants: Vec<CombatantStateWire>,
    initiative_order: Vec<String>,
    round: u32,
    current_turn_index: usize,
    turn_action_consumed: bool,
    consumed_roll_ids: Vec<String>,
    status: CombatStatus,
    version: u64,
    last_transition: CombatMutation,
}

impl CombatState {
    pub fn start(
        combat_id: impl Into<String>,
        participants: Vec<CombatantState>,
    ) -> KernelResult<Self> {
        let combat_id = combat_id.into();
        let unique: HashSet<&str> = participants
            .iter()
            .map(|participant| participant.participant_id.as_str())
            .collect();
        if !valid_combat_id(&combat_id)
            || participants.len() < 2
            || unique.len() != participants.len()
        {
            return Err(TrpgError::InvalidConfiguration("combat_state"));
        }

        let mut initiative = participants
            .iter()
            .map(|participant| (participant.dexterity, participant.participant_id.clone()))
            .collect::<Vec<_>>();
        initiative.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        Ok(Self {
            combat_id,
            participants,
            initiative_order: initiative.into_iter().map(|(_, id)| id).collect(),
            round: 1,
            current_turn_index: 0,
            turn_action_consumed: false,
            consumed_roll_ids: Vec::new(),
            status: CombatStatus::Ongoing,
            version: 1,
            last_transition: CombatMutation::Started,
        })
    }

    pub fn combat_id(&self) -> &str {
        &self.combat_id
    }

    pub fn participants(&self) -> &[CombatantState] {
        &self.participants
    }

    pub fn initiative_order(&self) -> &[String] {
        &self.initiative_order
    }

    pub const fn round(&self) -> u32 {
        self.round
    }

    pub const fn current_turn_index(&self) -> usize {
        self.current_turn_index
    }

    pub const fn turn_action_consumed(&self) -> bool {
        self.turn_action_consumed
    }

    pub const fn status(&self) -> CombatStatus {
        self.status
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn current_actor(&self) -> Option<&str> {
        (self.status == CombatStatus::Ongoing)
            .then(|| self.initiative_order.get(self.current_turn_index))
            .flatten()
            .map(String::as_str)
    }

    pub fn apply_damage(
        &mut self,
        target_id: &str,
        action: CombatActionKind,
        defense: CombatDefense,
        attacker_roll: &ServerPercentileRoll,
        defender_roll: Option<&ServerPercentileRoll>,
        damage_roll: Option<&ServerDamageRoll>,
    ) -> KernelResult<CombatTransition> {
        let attacker_id = self
            .current_actor()
            .ok_or(TrpgError::InvalidConfiguration("combat_actor"))?
            .to_owned();
        let attacker = self
            .participants
            .iter()
            .find(|participant| participant.participant_id == attacker_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_actor"))?;
        if !attacker.condition.can_act() {
            return Err(TrpgError::InvalidConfiguration(
                "combat_actor_incapacitated",
            ));
        }
        if self.turn_action_consumed {
            return Err(TrpgError::InvalidConfiguration(
                "combat_turn_action_consumed",
            ));
        }
        let attacker_target = attacker.skill_targets.attack_target(action);
        let defender = self
            .participants
            .iter()
            .find(|participant| participant.participant_id == target_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
        if defense != CombatDefense::None && !defender.condition.can_act() {
            return Err(TrpgError::InvalidConfiguration(
                "combat_defender_incapacitated",
            ));
        }
        if (defense != CombatDefense::None) != defender_roll.is_some() {
            return Err(TrpgError::InvalidConfiguration("combat_defense_roll"));
        }
        if defense == CombatDefense::FightBack && action != CombatActionKind::Melee {
            return Err(TrpgError::InvalidConfiguration("combat_fight_back_action"));
        }
        let defender_target = defender.skill_targets.defense_target(defense);
        if defense == CombatDefense::None {
            let attacker_roll =
                PercentileRollEvidence::from_server_roll(attacker_target, attacker_roll)?;
            let damage_roll = damage_roll.map(DamageRollEvidence::from_server_roll);
            return self.apply_verified_attack(VerifiedAttackInput {
                attacker_id: &attacker_id,
                target_id,
                action,
                defense,
                attacker_roll: &attacker_roll,
                defender_roll: None,
                damage_roll: damage_roll.as_ref(),
            });
        }
        let defender_target =
            defender_target.ok_or(TrpgError::InvalidConfiguration("combat_defense_roll"))?;
        let attacker_roll =
            PercentileRollEvidence::from_server_roll(attacker_target, attacker_roll)?;
        let defender_roll = defender_roll
            .map(|roll| PercentileRollEvidence::from_server_roll(defender_target, roll))
            .transpose()?;
        let damage_roll = damage_roll.map(DamageRollEvidence::from_server_roll);
        self.apply_verified_attack(VerifiedAttackInput {
            attacker_id: &attacker_id,
            target_id,
            action,
            defense,
            attacker_roll: &attacker_roll,
            defender_roll: defender_roll.as_ref(),
            damage_roll: damage_roll.as_ref(),
        })
    }

    fn apply_verified_attack(
        &mut self,
        input: VerifiedAttackInput<'_>,
    ) -> KernelResult<CombatTransition> {
        let VerifiedAttackInput {
            attacker_id,
            target_id,
            action,
            defense,
            attacker_roll,
            defender_roll,
            damage_roll,
        } = input;
        if self.status != CombatStatus::Ongoing {
            return Err(TrpgError::InvalidConfiguration("combat_terminal"));
        }
        if self.current_actor() != Some(attacker_id) || attacker_id == target_id {
            return Err(TrpgError::InvalidConfiguration("combat_actor"));
        }
        let attacker = self
            .participants
            .iter()
            .find(|participant| participant.participant_id == attacker_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_actor"))?;
        if !attacker.condition.can_act() {
            return Err(TrpgError::InvalidConfiguration(
                "combat_actor_incapacitated",
            ));
        }
        if self.turn_action_consumed {
            return Err(TrpgError::InvalidConfiguration(
                "combat_turn_action_consumed",
            ));
        }
        let attacker_target = attacker.skill_targets.attack_target(action);
        let defender = self
            .participants
            .iter()
            .find(|participant| participant.participant_id == target_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
        if defense != CombatDefense::None && !defender.condition.can_act() {
            return Err(TrpgError::InvalidConfiguration(
                "combat_defender_incapacitated",
            ));
        }
        let defender_target = defender.skill_targets.defense_target(defense);
        if defense == CombatDefense::FightBack && action != CombatActionKind::Melee {
            return Err(TrpgError::InvalidConfiguration("combat_fight_back_action"));
        }
        attacker_roll.validate(attacker_target)?;
        if (defense != CombatDefense::None) != defender_roll.is_some() {
            return Err(TrpgError::InvalidConfiguration("combat_defense_roll"));
        }
        if let (Some(defender_roll), Some(defender_target)) = (defender_roll, defender_target) {
            defender_roll.validate(defender_target)?;
        }
        let mut attack_roll_ids = vec![attacker_roll.roll_id.as_str()];
        if let Some(defender_roll) = defender_roll {
            attack_roll_ids.push(defender_roll.roll_id.as_str());
        }
        if attack_roll_ids.iter().collect::<HashSet<_>>().len() != attack_roll_ids.len()
            || attack_roll_ids.iter().any(|roll_id| {
                self.consumed_roll_ids
                    .iter()
                    .any(|consumed| consumed == roll_id)
            })
        {
            return Err(TrpgError::InvalidConfiguration("combat_roll_reuse"));
        }
        let outcome = exchange_outcome(
            defense,
            attacker_roll.success_level,
            defender_roll.map(|roll| roll.success_level),
        )?;
        let Some(outcome) = outcome else {
            if damage_roll.is_some() {
                return Err(TrpgError::InvalidConfiguration(
                    "combat_damage_roll_unexpected",
                ));
            }
            let target = self
                .participants
                .iter()
                .find(|participant| participant.participant_id == target_id)
                .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
            let transition = CombatTransition {
                before_hp: target.current_hp,
                after_hp: target.current_hp,
                raw_damage: 0,
                armor_absorbed: 0,
                damage: 0,
                prior_condition: target.condition,
                condition: target.condition,
            };
            self.last_transition = CombatMutation::AttackMissed {
                attacker_id: attacker_id.to_owned(),
                target_id: target_id.to_owned(),
                action,
                defense,
                attacker_roll: attacker_roll.clone(),
                defender_roll: defender_roll.cloned(),
            };
            self.consumed_roll_ids
                .extend(attack_roll_ids.into_iter().map(str::to_owned));
            self.turn_action_consumed = true;
            self.version = self
                .version
                .checked_add(1)
                .ok_or(TrpgError::InvalidConfiguration("combat_version"))?;
            return Ok(transition);
        };
        let damage_roll =
            damage_roll.ok_or(TrpgError::InvalidConfiguration("combat_damage_evidence"))?;
        attack_roll_ids.push(damage_roll.roll_id.as_str());
        if attack_roll_ids.iter().collect::<HashSet<_>>().len() != attack_roll_ids.len()
            || self
                .consumed_roll_ids
                .iter()
                .any(|consumed| attack_roll_ids.contains(&consumed.as_str()))
        {
            return Err(TrpgError::InvalidConfiguration("combat_roll_reuse"));
        }
        let damaged_participant_id = match outcome {
            CombatExchangeOutcome::AttackerHit => target_id,
            CombatExchangeOutcome::DefenderFoughtBack => attacker_id,
        };
        let expected_damage_formula = match outcome {
            CombatExchangeOutcome::AttackerHit => attacker.weapon_loadout.damage_formula(action),
            CombatExchangeOutcome::DefenderFoughtBack => defender
                .weapon_loadout
                .damage_formula(CombatActionKind::Melee),
        };
        damage_roll.validate(expected_damage_formula)?;
        let target = self
            .participants
            .iter_mut()
            .find(|participant| participant.participant_id == damaged_participant_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
        let transition = apply_damage_with_armor(
            target.current_hp,
            target.max_hp,
            damage_roll.raw_damage,
            target.armor,
            target.condition,
        )?;
        target.current_hp = transition.after_hp;
        target.condition = transition.condition;
        self.last_transition = CombatMutation::DamageApplied {
            attacker_id: attacker_id.to_owned(),
            target_id: target_id.to_owned(),
            action,
            defense,
            outcome,
            attacker_roll: attacker_roll.clone(),
            defender_roll: defender_roll.cloned(),
            damage_roll: damage_roll.clone(),
            raw_damage: damage_roll.raw_damage,
        };
        self.consumed_roll_ids
            .extend(attack_roll_ids.into_iter().map(str::to_owned));
        self.turn_action_consumed = true;
        self.version = self
            .version
            .checked_add(1)
            .ok_or(TrpgError::InvalidConfiguration("combat_version"))?;
        Ok(transition)
    }

    pub fn recover_major_wound(
        &mut self,
        healer_id: &str,
        target_id: &str,
        medical_skill: CombatMedicalSkill,
        medical_roll: &ServerPercentileRoll,
    ) -> KernelResult<CombatCondition> {
        if self.status != CombatStatus::Ongoing {
            return Err(TrpgError::InvalidConfiguration("combat_terminal"));
        }
        let healer = self
            .participants
            .iter()
            .find(|participant| participant.participant_id == healer_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_healer"))?;
        if self.current_actor() != Some(healer_id) || !healer.condition.can_act() {
            return Err(TrpgError::InvalidConfiguration("combat_healer"));
        }
        if self.turn_action_consumed {
            return Err(TrpgError::InvalidConfiguration(
                "combat_turn_action_consumed",
            ));
        }
        let medical_target = healer.skill_targets.medical_target(medical_skill);
        let medical_roll = PercentileRollEvidence::from_server_roll(medical_target, medical_roll)?;
        self.apply_verified_recovery(healer_id, target_id, medical_skill, &medical_roll)
    }

    fn apply_verified_recovery(
        &mut self,
        healer_id: &str,
        target_id: &str,
        medical_skill: CombatMedicalSkill,
        medical_roll: &PercentileRollEvidence,
    ) -> KernelResult<CombatCondition> {
        if self.status != CombatStatus::Ongoing {
            return Err(TrpgError::InvalidConfiguration("combat_terminal"));
        }
        let healer = self
            .participants
            .iter()
            .find(|participant| participant.participant_id == healer_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_healer"))?;
        if self.current_actor() != Some(healer_id) || !healer.condition.can_act() {
            return Err(TrpgError::InvalidConfiguration("combat_healer"));
        }
        if self.turn_action_consumed {
            return Err(TrpgError::InvalidConfiguration(
                "combat_turn_action_consumed",
            ));
        }
        medical_roll.validate(healer.skill_targets.medical_target(medical_skill))?;
        if self
            .consumed_roll_ids
            .iter()
            .any(|consumed| consumed == &medical_roll.roll_id)
        {
            return Err(TrpgError::InvalidConfiguration("combat_roll_reuse"));
        }
        let target = self
            .participants
            .iter_mut()
            .find(|participant| participant.participant_id == target_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
        let stabilizes_dying = target.current_hp == 0
            && target.condition == CombatCondition::Dying
            && medical_skill == CombatMedicalSkill::FirstAid;
        let treats_major_wound =
            target.current_hp > 0 && target.condition == CombatCondition::MajorWound;
        if !stabilizes_dying && !treats_major_wound {
            return Err(TrpgError::InvalidConfiguration("major_wound_recovery"));
        }
        let recovered = success_rank(medical_roll.success_level) > 0;
        if recovered {
            if stabilizes_dying {
                target.current_hp = 1;
                target.condition = CombatCondition::MajorWound;
            } else {
                target.condition = recover_major_wound(target.current_hp, target.condition, true)?;
            }
        }
        self.last_transition = CombatMutation::MajorWoundRecoveryAttempted {
            healer_id: healer_id.to_owned(),
            target_id: target_id.to_owned(),
            medical_skill,
            medical_roll: medical_roll.clone(),
            recovered,
        };
        self.consumed_roll_ids.push(medical_roll.roll_id.clone());
        self.turn_action_consumed = true;
        self.version = self
            .version
            .checked_add(1)
            .ok_or(TrpgError::InvalidConfiguration("combat_version"))?;
        Ok(target.condition)
    }

    pub fn advance_turn(&mut self) -> KernelResult<&str> {
        if self.status != CombatStatus::Ongoing {
            return Err(TrpgError::InvalidConfiguration("combat_terminal"));
        }
        let participant_count = self.initiative_order.len();
        let mut next_turn_index = self.current_turn_index;
        let mut next_round = self.round;
        for _ in 0..participant_count {
            next_turn_index += 1;
            if next_turn_index == participant_count {
                next_turn_index = 0;
                next_round = next_round
                    .checked_add(1)
                    .ok_or(TrpgError::InvalidConfiguration("combat_round"))?;
            }
            let candidate = &self.initiative_order[next_turn_index];
            if self
                .participants
                .iter()
                .find(|participant| participant.participant_id == *candidate)
                .is_some_and(|participant| participant.condition.can_act())
            {
                self.current_turn_index = next_turn_index;
                self.round = next_round;
                self.turn_action_consumed = false;
                self.last_transition = CombatMutation::TurnAdvanced;
                self.version = self
                    .version
                    .checked_add(1)
                    .ok_or(TrpgError::InvalidConfiguration("combat_version"))?;
                return Ok(candidate);
            }
        }
        Err(TrpgError::InvalidConfiguration(
            "combat_no_active_participant",
        ))
    }

    pub fn end(&mut self) -> KernelResult<()> {
        if self.status != CombatStatus::Ongoing {
            return Err(TrpgError::InvalidConfiguration("combat_terminal"));
        }
        self.status = CombatStatus::Ended;
        self.last_transition = CombatMutation::Ended;
        self.version = self
            .version
            .checked_add(1)
            .ok_or(TrpgError::InvalidConfiguration("combat_version"))?;
        Ok(())
    }

    /// Serializes a state-machine-produced snapshot for a formal event. The
    /// aggregate deliberately does not implement `Deserialize`, so callers
    /// cannot manufacture a recordable state from arbitrary JSON.
    pub fn persistence_json(&self) -> KernelResult<String> {
        serde_json::to_string(self)
            .map_err(|_| TrpgError::InvalidConfiguration("combat_state_serialization"))
    }

    /// Proves that this opaque aggregate is either a valid initial state or
    /// exactly one legal successor of the persisted state. Persistence
    /// adapters must call this before appending a formal event.
    pub fn validate_persistence_transition(
        &self,
        previous_state_json: Option<&str>,
    ) -> KernelResult<()> {
        let Some(previous_state_json) = previous_state_json else {
            if self.version == 1
                && self.round == 1
                && self.current_turn_index == 0
                && !self.turn_action_consumed
                && self.consumed_roll_ids.is_empty()
                && self.status == CombatStatus::Ongoing
                && matches!(self.last_transition, CombatMutation::Started)
            {
                return Ok(());
            }
            return Err(TrpgError::InvalidConfiguration("combat_initial_state"));
        };

        let wire: CombatStateWire = serde_json::from_str(previous_state_json)
            .map_err(|_| TrpgError::InvalidConfiguration("combat_persisted_state"))?;
        let mut expected = Self::try_from_wire(wire)?;
        match &self.last_transition {
            CombatMutation::Started => {
                return Err(TrpgError::InvalidConfiguration(
                    "combat_transition_restarted",
                ));
            }
            CombatMutation::AttackMissed {
                attacker_id,
                target_id,
                action,
                defense,
                attacker_roll,
                defender_roll,
            } => {
                expected.apply_verified_attack(VerifiedAttackInput {
                    attacker_id,
                    target_id,
                    action: *action,
                    defense: *defense,
                    attacker_roll,
                    defender_roll: defender_roll.as_ref(),
                    damage_roll: None,
                })?;
            }
            CombatMutation::DamageApplied {
                attacker_id,
                target_id,
                action,
                defense,
                outcome: _,
                attacker_roll,
                defender_roll,
                damage_roll,
                raw_damage,
            } => {
                if *raw_damage != damage_roll.raw_damage {
                    return Err(TrpgError::InvalidConfiguration("combat_damage_evidence"));
                }
                expected.apply_verified_attack(VerifiedAttackInput {
                    attacker_id,
                    target_id,
                    action: *action,
                    defense: *defense,
                    attacker_roll,
                    defender_roll: defender_roll.as_ref(),
                    damage_roll: Some(damage_roll),
                })?;
            }
            CombatMutation::MajorWoundRecoveryAttempted {
                healer_id,
                target_id,
                medical_skill,
                medical_roll,
                recovered: _,
            } => {
                expected.apply_verified_recovery(
                    healer_id,
                    target_id,
                    *medical_skill,
                    medical_roll,
                )?;
            }
            CombatMutation::TurnAdvanced => {
                expected.advance_turn()?;
            }
            CombatMutation::Ended => {
                expected.end()?;
            }
        }
        if expected == *self {
            Ok(())
        } else {
            Err(TrpgError::InvalidConfiguration(
                "combat_transition_mismatch",
            ))
        }
    }

    /// Validates an Event Store snapshot during projection replay without
    /// exposing a deserializable/constructible formal aggregate to callers.
    pub fn validate_serialized_persistence_transition(
        previous_state_json: Option<&str>,
        next_state_json: &str,
    ) -> KernelResult<()> {
        let wire: CombatStateWire = serde_json::from_str(next_state_json)
            .map_err(|_| TrpgError::InvalidConfiguration("combat_persisted_state"))?;
        let next = Self::try_from_wire(wire)?;
        next.validate_persistence_transition(previous_state_json)
    }

    fn try_from_wire(wire: CombatStateWire) -> KernelResult<Self> {
        let participants = wire
            .participants
            .into_iter()
            .map(|participant| CombatantState {
                participant_id: participant.participant_id,
                dexterity: participant.dexterity,
                skill_targets: participant.skill_targets,
                weapon_loadout: participant.weapon_loadout,
                current_hp: participant.current_hp,
                max_hp: participant.max_hp,
                armor: participant.armor,
                condition: participant.condition,
            })
            .collect::<Vec<_>>();
        let unique = participants
            .iter()
            .map(|participant| participant.participant_id.as_str())
            .collect::<HashSet<_>>();
        let mut derived_order = participants
            .iter()
            .map(|participant| (participant.dexterity, participant.participant_id.clone()))
            .collect::<Vec<_>>();
        derived_order
            .sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        let derived_order = derived_order
            .into_iter()
            .map(|(_, participant_id)| participant_id)
            .collect::<Vec<_>>();
        if !valid_combat_id(&wire.combat_id)
            || participants.len() < 2
            || unique.len() != participants.len()
            || participants.iter().any(|participant| {
                !valid_combat_id(&participant.participant_id)
                    || participant.dexterity == 0
                    || participant.dexterity > 100
                    || !(1..=100).contains(&participant.skill_targets.melee)
                    || !(1..=100).contains(&participant.skill_targets.firearm)
                    || !(1..=100).contains(&participant.skill_targets.dodge)
                    || !(1..=100).contains(&participant.skill_targets.first_aid)
                    || !(1..=100).contains(&participant.skill_targets.medicine)
                    || !participant.weapon_loadout.is_valid()
                    || participant.max_hp == 0
                    || participant.current_hp > participant.max_hp
                    || participant.armor > 30
                    || (participant.current_hp == 0)
                        != matches!(
                            participant.condition,
                            CombatCondition::Dying | CombatCondition::Dead
                        )
            })
            || derived_order != wire.initiative_order
            || wire.round == 0
            || wire.current_turn_index >= participants.len()
            || wire
                .consumed_roll_ids
                .iter()
                .any(|roll_id| !valid_combat_id(roll_id))
            || wire.consumed_roll_ids.iter().collect::<HashSet<_>>().len()
                != wire.consumed_roll_ids.len()
            || wire.version == 0
        {
            return Err(TrpgError::InvalidConfiguration("combat_persisted_state"));
        }
        Ok(Self {
            combat_id: wire.combat_id,
            participants,
            initiative_order: wire.initiative_order,
            round: wire.round,
            current_turn_index: wire.current_turn_index,
            turn_action_consumed: wire.turn_action_consumed,
            consumed_roll_ids: wire.consumed_roll_ids,
            status: wire.status,
            version: wire.version,
            last_transition: wire.last_transition,
        })
    }
}

pub fn resolve_attack(
    action: CombatActionKind,
    attacker_roll: &ServerDiceRoll,
    defense: CombatDefense,
    defender_roll: Option<&ServerDiceRoll>,
) -> KernelResult<AttackResolution> {
    if (defense != CombatDefense::None) != defender_roll.is_some() {
        return Err(TrpgError::InvalidConfiguration("combat_defense_roll"));
    }
    if defense == CombatDefense::FightBack && action != CombatActionKind::Melee {
        return Err(TrpgError::InvalidConfiguration("combat_fight_back_action"));
    }
    let attacker_success = attacker_roll.outcome().success_level;
    let defender_success = defender_roll.map(|roll| roll.outcome().success_level);
    let outcome = exchange_outcome(defense, attacker_success, defender_success)?;
    Ok(AttackResolution {
        action,
        defense,
        attacker_success,
        defender_success,
        hit: outcome == Some(CombatExchangeOutcome::AttackerHit),
        counterattack: outcome == Some(CombatExchangeOutcome::DefenderFoughtBack),
        outcome,
    })
}

pub fn apply_damage(
    current_hp: u8,
    max_hp: u8,
    damage: u8,
    prior_condition: CombatCondition,
) -> KernelResult<CombatTransition> {
    apply_damage_with_armor(current_hp, max_hp, damage, 0, prior_condition)
}

pub fn apply_damage_with_armor(
    current_hp: u8,
    max_hp: u8,
    raw_damage: u8,
    armor: u8,
    prior_condition: CombatCondition,
) -> KernelResult<CombatTransition> {
    if max_hp == 0
        || current_hp > max_hp
        || armor > 30
        || matches!(
            prior_condition,
            CombatCondition::Dying | CombatCondition::Dead
        ) != (current_hp == 0)
        || prior_condition == CombatCondition::Dead
    {
        return Err(TrpgError::InvalidConfiguration("hit_point_range"));
    }

    let damage = raw_damage.saturating_sub(armor);
    let armor_absorbed = raw_damage - damage;
    let after_hp = current_hp.saturating_sub(damage);
    let major_wound_threshold = max_hp.div_ceil(2);
    let condition = if after_hp == 0 && damage >= max_hp {
        CombatCondition::Dead
    } else if after_hp == 0 {
        CombatCondition::Dying
    } else if prior_condition == CombatCondition::MajorWound || damage >= major_wound_threshold {
        CombatCondition::MajorWound
    } else {
        CombatCondition::Able
    };

    Ok(CombatTransition {
        before_hp: current_hp,
        after_hp,
        raw_damage,
        armor_absorbed,
        damage,
        prior_condition,
        condition,
    })
}

pub fn recover_major_wound(
    current_hp: u8,
    prior_condition: CombatCondition,
    medical_recovery_event: bool,
) -> KernelResult<CombatCondition> {
    if prior_condition != CombatCondition::MajorWound || current_hp == 0 || !medical_recovery_event
    {
        return Err(TrpgError::InvalidConfiguration("major_wound_recovery"));
    }
    Ok(CombatCondition::Able)
}

pub fn record_combat_transition<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    transition: &CombatTransition,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        EventType::CombatStateUpdated.name(),
        "combat_state_machine",
        format!(
            "hp {}->{} raw_damage={} armor={} damage={} condition={:?}->{:?}",
            transition.before_hp,
            transition.after_hp,
            transition.raw_damage,
            transition.armor_absorbed,
            transition.damage,
            transition.prior_condition,
            transition.condition
        ),
    )
}

fn success_rank(level: SuccessLevel) -> u8 {
    match level {
        SuccessLevel::Critical => 4,
        SuccessLevel::Extreme => 3,
        SuccessLevel::Hard => 2,
        SuccessLevel::Regular => 1,
        SuccessLevel::Failure | SuccessLevel::Fumble => 0,
    }
}

fn exchange_outcome(
    defense: CombatDefense,
    attacker_success: SuccessLevel,
    defender_success: Option<SuccessLevel>,
) -> KernelResult<Option<CombatExchangeOutcome>> {
    if (defense != CombatDefense::None) != defender_success.is_some() {
        return Err(TrpgError::InvalidConfiguration("combat_defense_roll"));
    }
    let attacker_rank = success_rank(attacker_success);
    let defender_rank = defender_success.map(success_rank).unwrap_or(0);
    Ok(match defense {
        CombatDefense::None if attacker_rank > 0 => Some(CombatExchangeOutcome::AttackerHit),
        CombatDefense::None => None,
        CombatDefense::Dodge if attacker_rank > defender_rank && attacker_rank > 0 => {
            Some(CombatExchangeOutcome::AttackerHit)
        }
        CombatDefense::Dodge => None,
        CombatDefense::FightBack if attacker_rank >= defender_rank && attacker_rank > 0 => {
            Some(CombatExchangeOutcome::AttackerHit)
        }
        CombatDefense::FightBack if defender_rank > attacker_rank => {
            Some(CombatExchangeOutcome::DefenderFoughtBack)
        }
        CombatDefense::FightBack => None,
    })
}

fn valid_combat_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
