use crate::dice_roll_contract::{ServerDiceRoll, SuccessLevel};
use crate::{append_coc7_event, Coc7EventPayload};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use trpg_contracts::EventType;
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttackResolution {
    pub action: CombatActionKind,
    pub defense: CombatDefense,
    pub attacker_success: SuccessLevel,
    pub defender_success: Option<SuccessLevel>,
    pub hit: bool,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CombatantState {
    participant_id: String,
    dexterity: u8,
    current_hp: u8,
    max_hp: u8,
    armor: u8,
    condition: CombatCondition,
}

impl CombatantState {
    pub fn new(
        participant_id: impl Into<String>,
        dexterity: u8,
        max_hp: u8,
        armor: u8,
    ) -> KernelResult<Self> {
        let participant_id = participant_id.into();
        if !valid_combat_id(&participant_id)
            || dexterity == 0
            || dexterity > 100
            || max_hp == 0
            || armor > 30
        {
            return Err(TrpgError::InvalidConfiguration("combat_participant"));
        }
        Ok(Self {
            participant_id,
            dexterity,
            current_hp: max_hp,
            max_hp,
            armor,
            condition: CombatCondition::Able,
        })
    }

    pub fn participant_id(&self) -> &str {
        &self.participant_id
    }

    pub const fn dexterity(&self) -> u8 {
        self.dexterity
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
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CombatState {
    combat_id: String,
    participants: Vec<CombatantState>,
    initiative_order: Vec<String>,
    round: u32,
    current_turn_index: usize,
    status: CombatStatus,
    version: u64,
    last_transition: CombatMutation,
}

#[derive(Deserialize)]
struct CombatantStateWire {
    participant_id: String,
    dexterity: u8,
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
        raw_damage: u8,
    ) -> KernelResult<CombatTransition> {
        if self.status != CombatStatus::Ongoing {
            return Err(TrpgError::InvalidConfiguration("combat_terminal"));
        }
        let target = self
            .participants
            .iter_mut()
            .find(|participant| participant.participant_id == target_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
        let transition = apply_damage_with_armor(
            target.current_hp,
            target.max_hp,
            raw_damage,
            target.armor,
            target.condition,
        )?;
        target.current_hp = transition.after_hp;
        target.condition = transition.condition;
        self.last_transition = CombatMutation::DamageApplied {
            target_id: target_id.to_owned(),
            raw_damage,
        };
        self.version = self
            .version
            .checked_add(1)
            .ok_or(TrpgError::InvalidConfiguration("combat_version"))?;
        Ok(transition)
    }

    pub fn recover_major_wound(
        &mut self,
        target_id: &str,
        medical_roll: &ServerDiceRoll,
    ) -> KernelResult<CombatCondition> {
        if self.status != CombatStatus::Ongoing {
            return Err(TrpgError::InvalidConfiguration("combat_terminal"));
        }
        if success_rank(medical_roll.outcome().success_level) == 0 {
            return Err(TrpgError::InvalidConfiguration("major_wound_recovery"));
        }
        self.apply_verified_recovery(target_id, medical_roll.roll_id())
    }

    fn apply_verified_recovery(
        &mut self,
        target_id: &str,
        medical_roll_id: &str,
    ) -> KernelResult<CombatCondition> {
        let target = self
            .participants
            .iter_mut()
            .find(|participant| participant.participant_id == target_id)
            .ok_or(TrpgError::InvalidConfiguration("combat_target"))?;
        target.condition = recover_major_wound(target.current_hp, target.condition, true)?;
        self.last_transition = CombatMutation::MajorWoundRecovered {
            target_id: target_id.to_owned(),
            medical_roll_id: medical_roll_id.to_owned(),
        };
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
                && self.status == CombatStatus::Ongoing
                && matches!(self.last_transition, CombatMutation::Started)
                && self.participants.iter().all(|participant| {
                    participant.current_hp == participant.max_hp
                        && participant.condition == CombatCondition::Able
                })
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
            CombatMutation::DamageApplied {
                target_id,
                raw_damage,
            } => {
                expected.apply_damage(target_id, *raw_damage)?;
            }
            CombatMutation::MajorWoundRecovered {
                target_id,
                medical_roll_id,
            } => {
                if medical_roll_id.trim().is_empty() {
                    return Err(TrpgError::InvalidConfiguration(
                        "major_wound_recovery_evidence",
                    ));
                }
                expected.apply_verified_recovery(target_id, medical_roll_id)?;
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
    if matches!(defense, CombatDefense::Dodge) != defender_roll.is_some() {
        return Err(TrpgError::InvalidConfiguration("combat_defense_roll"));
    }
    let attacker_success = attacker_roll.outcome().success_level;
    let defender_success = defender_roll.map(|roll| roll.outcome().success_level);
    let attacker_rank = success_rank(attacker_success);
    let hit = if attacker_rank == 0 {
        false
    } else if let Some(defender_success) = defender_success {
        attacker_rank > success_rank(defender_success)
    } else {
        true
    };
    Ok(AttackResolution {
        action,
        defense,
        attacker_success,
        defender_success,
        hit,
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

fn valid_combat_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
