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
        target_id: String,
        raw_damage: u8,
    },
    MajorWoundRecovered {
        target_id: String,
        medical_roll_id: String,
    },
    TurnAdvanced,
    Ended,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Combatant {
    participant_id: String,
    dexterity: u8,
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
            target_id,
            raw_damage,
        } => {
            let target = state
                .participants
                .iter_mut()
                .find(|participant| participant.participant_id == *target_id)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            apply_validated_damage(target, *raw_damage)?;
        }
        CombatMutation::MajorWoundRecovered {
            target_id,
            medical_roll_id,
        } => {
            if !valid_id(medical_roll_id) {
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
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
enum ChaseMutation {
    Started,
    Advanced {
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
                 "max_hp":10,"armor":0,"condition":"ABLE"},
                {"participant_id":"two","dexterity":50,"current_hp":8,
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
