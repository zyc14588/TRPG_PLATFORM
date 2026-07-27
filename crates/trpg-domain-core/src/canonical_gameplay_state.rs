//! Independent validation for serialized formal combat and chase states.
//!
//! The COC7 ruleset produces these snapshots. Domain persistence validates the
//! complete shape and exact predecessor transition again before an event can
//! become canonical, without creating an outward dependency from adapters to
//! the concrete ruleset crate.

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
    MajorWoundRecovered {
        target_id: String,
        medical_roll: PercentileRollEvidence,
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
            || next.status != CombatStatus::Ongoing
            || !matches!(next.last_transition, CombatMutation::Started)
            || next.participants.iter().any(|participant| {
                participant.current_hp != participant.max_hp
                    || participant.condition != CombatCondition::Able
            })
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

pub fn validate_combat_server_roll_evidence(
    state_json: &str,
    attacker_roll: Option<&ServerPercentileRoll>,
    defender_roll: Option<&ServerPercentileRoll>,
    damage_roll: Option<&ServerDamageRoll>,
    medical_roll: Option<&ServerPercentileRoll>,
) -> Result<(), CanonicalGameplayStateError> {
    let state = parse_combat(state_json)?;
    match &state.last_transition {
        CombatMutation::DamageApplied {
            attacker_roll: recorded_attacker,
            defender_roll: recorded_defender,
            damage_roll: recorded_damage,
            ..
        } => {
            let attacker_roll =
                attacker_roll.ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            let damage_roll = damage_roll.ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            if medical_roll.is_some()
                || !percentile_evidence_matches_server(recorded_attacker, attacker_roll)
                || !match (recorded_defender, defender_roll) {
                    (Some(recorded), Some(server)) => {
                        percentile_evidence_matches_server(recorded, server)
                    }
                    (None, None) => true,
                    _ => false,
                }
                || !damage_evidence_matches_server(recorded_damage, damage_roll)
            {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
        }
        CombatMutation::MajorWoundRecovered {
            medical_roll: recorded_medical,
            ..
        } => {
            if attacker_roll.is_some()
                || defender_roll.is_some()
                || damage_roll.is_some()
                || !medical_roll.is_some_and(|server| {
                    percentile_evidence_matches_server(recorded_medical, server)
                })
            {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
        }
        _ if attacker_roll.is_none()
            && defender_roll.is_none()
            && damage_roll.is_none()
            && medical_roll.is_none() => {}
        _ => return Err(CanonicalGameplayStateError::InvalidTransition),
    }
    Ok(())
}

fn combat_summary(state: CombatSnapshot) -> ValidatedCombatState {
    ValidatedCombatState {
        combat_id: state.combat_id,
        status: state.status.as_str(),
        round: state.round,
        current_turn_index: state.current_turn_index,
        version: state.version,
    }
}

fn parse_combat(value: &str) -> Result<CombatSnapshot, CanonicalGameplayStateError> {
    let state: CombatSnapshot =
        serde_json::from_str(value).map_err(|_| CanonicalGameplayStateError::InvalidJson)?;
    let unique = state
        .participants
        .iter()
        .map(|participant| participant.participant_id.as_str())
        .collect::<HashSet<_>>();
    let mut initiative = state
        .participants
        .iter()
        .map(|participant| (participant.dexterity, participant.participant_id.clone()))
        .collect::<Vec<_>>();
    initiative.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let initiative = initiative
        .into_iter()
        .map(|(_, participant_id)| participant_id)
        .collect::<Vec<_>>();
    if !valid_id(&state.combat_id)
        || state.participants.len() < 2
        || unique.len() != state.participants.len()
        || state.participants.iter().any(|participant| {
            !valid_id(&participant.participant_id)
                || !(1..=100).contains(&participant.dexterity)
                || !(1..=100).contains(&participant.skill_targets.melee)
                || !(1..=100).contains(&participant.skill_targets.firearm)
                || !(1..=100).contains(&participant.skill_targets.dodge)
                || participant.max_hp == 0
                || participant.current_hp > participant.max_hp
                || participant.armor > 30
                || (participant.current_hp == 0)
                    != matches!(
                        participant.condition,
                        CombatCondition::Dying | CombatCondition::Dead
                    )
        })
        || initiative != state.initiative_order
        || state.round == 0
        || state.current_turn_index >= state.participants.len()
        || state.version == 0
    {
        return Err(CanonicalGameplayStateError::InvalidShape);
    }
    Ok(state)
}

fn apply_combat_mutation(
    state: &mut CombatSnapshot,
    mutation: &CombatMutation,
) -> Result<(), CanonicalGameplayStateError> {
    if state.status != CombatStatus::Ongoing {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    match mutation {
        CombatMutation::Started => return Err(CanonicalGameplayStateError::InvalidTransition),
        CombatMutation::DamageApplied {
            attacker_id,
            target_id,
            action,
            defense,
            outcome,
            attacker_roll,
            defender_roll,
            damage_roll,
            raw_damage,
        } => {
            let current_actor = state
                .initiative_order
                .get(state.current_turn_index)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            if current_actor != attacker_id || attacker_id == target_id {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            let attacker_target = state
                .participants
                .iter()
                .find(|participant| participant.participant_id == *attacker_id)
                .map(|participant| participant.skill_targets.attack_target(*action))
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            let defender = state
                .participants
                .iter()
                .find(|participant| participant.participant_id == *target_id)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            let defender_target = defender.skill_targets.defense_target(*defense);
            if *defense == CombatDefense::FightBack && *action != CombatActionKind::Melee {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            validate_percentile_evidence(attacker_roll, attacker_target)?;
            if (*defense != CombatDefense::None) != defender_roll.is_some() {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            if let (Some(defender_roll), Some(defender_target)) = (defender_roll, defender_target) {
                validate_percentile_evidence(defender_roll, defender_target)?;
            }
            if defender_roll
                .as_ref()
                .is_some_and(|roll| roll.roll_id == attacker_roll.roll_id)
                || damage_roll.roll_id == attacker_roll.roll_id
                || defender_roll
                    .as_ref()
                    .is_some_and(|roll| roll.roll_id == damage_roll.roll_id)
            {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            let derived_outcome = canonical_exchange_outcome(
                *defense,
                attacker_roll.success_level,
                defender_roll.as_ref().map(|roll| roll.success_level),
            )?
            .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            if derived_outcome != *outcome {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            validate_damage_evidence(damage_roll, *action)?;
            if *raw_damage != damage_roll.raw_damage {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            let damaged_participant_id = match outcome {
                CombatExchangeOutcome::AttackerHit => target_id,
                CombatExchangeOutcome::DefenderFoughtBack => attacker_id,
            };
            let target = state
                .participants
                .iter_mut()
                .find(|participant| {
                    participant.participant_id.as_str() == damaged_participant_id.as_str()
                })
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            apply_validated_damage(target, *raw_damage)?;
        }
        CombatMutation::MajorWoundRecovered {
            target_id,
            medical_roll,
        } => {
            validate_percentile_evidence(medical_roll, medical_roll.target)?;
            if medical_roll.target == 0 || success_rank(medical_roll.success_level) == 0 {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            let target = state
                .participants
                .iter_mut()
                .find(|participant| participant.participant_id == *target_id)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            if target.current_hp == 0 || target.condition != CombatCondition::MajorWound {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            target.condition = CombatCondition::Able;
        }
        CombatMutation::TurnAdvanced => {
            let participant_count = state.initiative_order.len();
            let mut found = false;
            for _ in 0..participant_count {
                state.current_turn_index += 1;
                if state.current_turn_index == participant_count {
                    state.current_turn_index = 0;
                    state.round = state
                        .round
                        .checked_add(1)
                        .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
                }
                let candidate = &state.initiative_order[state.current_turn_index];
                if state
                    .participants
                    .iter()
                    .find(|participant| participant.participant_id == *candidate)
                    .is_some_and(|participant| participant.condition.can_act())
                {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
        }
        CombatMutation::Ended => state.status = CombatStatus::Ended,
    }
    state.last_transition = mutation.clone();
    state.version = state
        .version
        .checked_add(1)
        .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
    Ok(())
}

fn validate_percentile_evidence(
    evidence: &PercentileRollEvidence,
    expected_target: u8,
) -> Result<(), CanonicalGameplayStateError> {
    let reconstructed = if evidence.selected_tens_digit == 0 && evidence.ones_digit == 0 {
        100
    } else {
        evidence.selected_tens_digit * 10 + evidence.ones_digit
    };
    if !valid_id(&evidence.roll_id)
        || evidence.target != expected_target
        || evidence.selected_tens_digit > 9
        || evidence.ones_digit > 9
        || reconstructed != evidence.roll
        || canonical_success_level(evidence.roll, evidence.target)? != evidence.success_level
    {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    Ok(())
}

fn validate_damage_evidence(
    evidence: &DamageRollEvidence,
    action: CombatActionKind,
) -> Result<(), CanonicalGameplayStateError> {
    let expected_formula = match action {
        CombatActionKind::Melee => (1, 6, 0),
        CombatActionKind::Firearm => (1, 6, 5),
    };
    let total = evidence
        .dice_values
        .iter()
        .try_fold(i16::from(evidence.flat_bonus), |sum, value| {
            sum.checked_add(i16::from(*value))
        })
        .and_then(|value| u8::try_from(value).ok());
    if !valid_id(&evidence.roll_id)
        || (evidence.dice_count, evidence.die_sides, evidence.flat_bonus) != expected_formula
        || evidence.dice_values.len() != usize::from(evidence.dice_count)
        || evidence
            .dice_values
            .iter()
            .any(|value| *value == 0 || *value > evidence.die_sides)
        || total != Some(evidence.raw_damage)
    {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    Ok(())
}

fn percentile_evidence_matches_server(
    evidence: &PercentileRollEvidence,
    server: &ServerPercentileRoll,
) -> bool {
    evidence.roll_id == server.roll_id()
        && evidence.roll == server.value()
        && evidence.selected_tens_digit == server.selected_tens_digit()
        && evidence.ones_digit == server.ones_digit()
}

fn damage_evidence_matches_server(
    evidence: &DamageRollEvidence,
    server: &ServerDamageRoll,
) -> bool {
    evidence.roll_id == server.roll_id()
        && evidence.dice_count == server.dice_count()
        && evidence.die_sides == server.die_sides()
        && evidence.flat_bonus == server.flat_bonus()
        && evidence.dice_values == server.dice_values()
        && evidence.raw_damage == server.value()
}

fn canonical_success_level(
    roll: u8,
    target: u8,
) -> Result<SuccessLevel, CanonicalGameplayStateError> {
    if !(1..=100).contains(&roll) || target > 100 {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    Ok(if roll == 1 {
        SuccessLevel::Critical
    } else if (target < 50 && roll >= 96) || (target >= 50 && roll == 100) {
        SuccessLevel::Fumble
    } else if roll <= target / 5 {
        SuccessLevel::Extreme
    } else if roll <= target / 2 {
        SuccessLevel::Hard
    } else if roll <= target {
        SuccessLevel::Regular
    } else {
        SuccessLevel::Failure
    })
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

fn canonical_exchange_outcome(
    defense: CombatDefense,
    attacker_success: SuccessLevel,
    defender_success: Option<SuccessLevel>,
) -> Result<Option<CombatExchangeOutcome>, CanonicalGameplayStateError> {
    if (defense != CombatDefense::None) != defender_success.is_some() {
        return Err(CanonicalGameplayStateError::InvalidTransition);
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

fn apply_validated_damage(
    target: &mut Combatant,
    raw_damage: u8,
) -> Result<(), CanonicalGameplayStateError> {
    if target.condition == CombatCondition::Dead {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    let damage = raw_damage.saturating_sub(target.armor);
    let after_hp = target.current_hp.saturating_sub(damage);
    let threshold = target.max_hp.div_ceil(2);
    let condition = if after_hp == 0 && damage >= target.max_hp {
        CombatCondition::Dead
    } else if after_hp == 0 {
        CombatCondition::Dying
    } else if target.condition == CombatCondition::MajorWound || damage >= threshold {
        CombatCondition::MajorWound
    } else {
        CombatCondition::Able
    };
    target.current_hp = after_hp;
    target.condition = condition;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ChaseStatus {
    Ongoing,
    Escaped,
    Caught,
}

impl ChaseStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ongoing => "ONGOING",
            Self::Escaped => "ESCAPED",
            Self::Caught => "CAUGHT",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ChaseRole {
    Quarry,
    Pursuer,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ChaseParticipant {
    participant_id: String,
    role: ChaseRole,
    movement_rate: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ChaseParticipantRollEvidence {
    participant_id: String,
    roll_id: String,
    target: u8,
    roll: u8,
    selected_tens_digit: u8,
    ones_digit: u8,
    success_level: SuccessLevel,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
enum ChaseMutation {
    Started,
    Advanced {
        rolls: Vec<ChaseParticipantRollEvidence>,
        quarry_success: bool,
        pursuer_success: bool,
        obstacle_id: Option<String>,
        obstacle_cost: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ChaseSnapshot {
    chase_id: String,
    participants: Vec<ChaseParticipant>,
    range: i8,
    segment: u32,
    status: ChaseStatus,
    version: u64,
    last_transition: ChaseMutation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedChaseState {
    chase_id: String,
    status: &'static str,
    range: i8,
    segment: u32,
    version: u64,
}

impl ValidatedChaseState {
    pub fn chase_id(&self) -> &str {
        &self.chase_id
    }

    pub const fn status(&self) -> &'static str {
        self.status
    }

    pub const fn range(&self) -> i8 {
        self.range
    }

    pub const fn segment(&self) -> u32 {
        self.segment
    }

    pub const fn version(&self) -> u64 {
        self.version
    }
}

pub fn validate_chase_state_transition(
    previous_state_json: Option<&str>,
    next_state_json: &str,
) -> Result<ValidatedChaseState, CanonicalGameplayStateError> {
    let next = parse_chase(next_state_json)?;
    let expected = if let Some(previous_state_json) = previous_state_json {
        let mut previous = parse_chase(previous_state_json)?;
        apply_chase_mutation(&mut previous, &next.last_transition)?;
        previous
    } else {
        if next.version != 1
            || next.segment != 1
            || next.status != ChaseStatus::Ongoing
            || !(1..=4).contains(&next.range)
            || !matches!(next.last_transition, ChaseMutation::Started)
        {
            return Err(CanonicalGameplayStateError::InvalidTransition);
        }
        next.clone()
    };
    if expected != next {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    Ok(chase_summary(next))
}

pub fn inspect_chase_state(
    state_json: &str,
) -> Result<ValidatedChaseState, CanonicalGameplayStateError> {
    parse_chase(state_json).map(chase_summary)
}

pub fn validate_chase_server_roll_evidence(
    state_json: &str,
    server_rolls: &[ServerPercentileRoll],
) -> Result<(), CanonicalGameplayStateError> {
    let state = parse_chase(state_json)?;
    match &state.last_transition {
        ChaseMutation::Advanced { rolls, .. } => {
            if rolls.len() != server_rolls.len()
                || !rolls.iter().zip(server_rolls).all(|(recorded, server)| {
                    recorded.roll_id == server.roll_id()
                        && recorded.roll == server.value()
                        && recorded.selected_tens_digit == server.selected_tens_digit()
                        && recorded.ones_digit == server.ones_digit()
                })
            {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
        }
        ChaseMutation::Started if server_rolls.is_empty() => {}
        _ => return Err(CanonicalGameplayStateError::InvalidTransition),
    }
    Ok(())
}

fn chase_summary(state: ChaseSnapshot) -> ValidatedChaseState {
    ValidatedChaseState {
        chase_id: state.chase_id,
        status: state.status.as_str(),
        range: state.range,
        segment: state.segment,
        version: state.version,
    }
}

fn parse_chase(value: &str) -> Result<ChaseSnapshot, CanonicalGameplayStateError> {
    let state: ChaseSnapshot =
        serde_json::from_str(value).map_err(|_| CanonicalGameplayStateError::InvalidJson)?;
    let unique = state
        .participants
        .iter()
        .map(|participant| participant.participant_id.as_str())
        .collect::<HashSet<_>>();
    let quarry_count = state
        .participants
        .iter()
        .filter(|participant| participant.role == ChaseRole::Quarry)
        .count();
    let pursuer_count = state
        .participants
        .iter()
        .filter(|participant| participant.role == ChaseRole::Pursuer)
        .count();
    if !valid_id(&state.chase_id)
        || unique.len() != state.participants.len()
        || quarry_count == 0
        || pursuer_count == 0
        || state.participants.iter().any(|participant| {
            !valid_id(&participant.participant_id) || !(1..=20).contains(&participant.movement_rate)
        })
        || !(0..=5).contains(&state.range)
        || state.segment == 0
        || state.version == 0
    {
        return Err(CanonicalGameplayStateError::InvalidShape);
    }
    Ok(state)
}

fn apply_chase_mutation(
    state: &mut ChaseSnapshot,
    mutation: &ChaseMutation,
) -> Result<(), CanonicalGameplayStateError> {
    if state.status != ChaseStatus::Ongoing {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    let ChaseMutation::Advanced {
        rolls,
        quarry_success,
        pursuer_success,
        obstacle_id,
        obstacle_cost,
    } = mutation
    else {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    };
    if *obstacle_cost > 2
        || obstacle_id.as_deref().is_some_and(|id| !valid_id(id))
        || (obstacle_id.is_none() && *obstacle_cost != 0)
    {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    if rolls.len() != state.participants.len() {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    for (participant, roll) in state.participants.iter().zip(rolls) {
        let target = participant
            .movement_rate
            .checked_mul(5)
            .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
        validate_chase_roll_evidence(roll, participant, target)?;
    }
    if rolls
        .iter()
        .map(|roll| roll.roll_id.as_str())
        .collect::<HashSet<_>>()
        .len()
        != rolls.len()
    {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    let derived_quarry_success = state
        .participants
        .iter()
        .zip(rolls)
        .filter(|(participant, _)| participant.role == ChaseRole::Quarry)
        .any(|(_, roll)| success_rank(roll.success_level) > 0);
    let derived_pursuer_success = state
        .participants
        .iter()
        .zip(rolls)
        .filter(|(participant, _)| participant.role == ChaseRole::Pursuer)
        .any(|(_, roll)| success_rank(roll.success_level) > 0);
    if derived_quarry_success != *quarry_success || derived_pursuer_success != *pursuer_success {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    let contest_delta = match (*quarry_success, *pursuer_success) {
        (true, false) => 1,
        (false, true) => -1,
        _ => 0,
    };
    let obstacle_delta = if *quarry_success {
        0
    } else {
        -(*obstacle_cost as i8)
    };
    state.range = (state.range + contest_delta + obstacle_delta).clamp(0, 5);
    state.status = if state.range >= 5 {
        ChaseStatus::Escaped
    } else if state.range <= 0 {
        ChaseStatus::Caught
    } else {
        ChaseStatus::Ongoing
    };
    state.segment = state
        .segment
        .checked_add(1)
        .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
    state.version = state
        .version
        .checked_add(1)
        .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
    state.last_transition = mutation.clone();
    Ok(())
}

fn validate_chase_roll_evidence(
    evidence: &ChaseParticipantRollEvidence,
    participant: &ChaseParticipant,
    target: u8,
) -> Result<(), CanonicalGameplayStateError> {
    let reconstructed = if evidence.selected_tens_digit == 0 && evidence.ones_digit == 0 {
        100
    } else {
        evidence.selected_tens_digit * 10 + evidence.ones_digit
    };
    if evidence.participant_id != participant.participant_id
        || !valid_id(&evidence.roll_id)
        || evidence.target != target
        || evidence.selected_tens_digit > 9
        || evidence.ones_digit > 9
        || reconstructed != evidence.roll
        || canonical_success_level(evidence.roll, target)? != evidence.success_level
    {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_same_id_combat_from_an_unrelated_lineage() {
        let initial = r#"{
            "combat_id":"combat_a",
            "participants":[
                {"participant_id":"one","dexterity":70,"current_hp":10,
                 "skill_targets":{"melee":60,"firearm":55,"dodge":40},
                 "max_hp":10,"armor":0,"condition":"ABLE"},
                {"participant_id":"two","dexterity":50,"current_hp":8,
                 "skill_targets":{"melee":45,"firearm":35,"dodge":25},
                 "max_hp":8,"armor":0,"condition":"ABLE"}
            ],
            "initiative_order":["one","two"],"round":1,
            "current_turn_index":0,"status":"ONGOING","version":1,
            "last_transition":{"kind":"STARTED"}
        }"#;
        let unrelated = initial
            .replace("\"current_hp\":10", "\"current_hp\":4")
            .replace("\"version\":1", "\"version\":2")
            .replace(
                "{\"kind\":\"STARTED\"}",
                "{\"kind\":\"DAMAGE_APPLIED\",\"target_id\":\"one\",\"raw_damage\":1}",
            );
        validate_combat_state_transition(None, initial).unwrap();
        assert!(validate_combat_state_transition(Some(initial), &unrelated).is_err());
    }
}
