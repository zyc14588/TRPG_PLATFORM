use crate::chase_state_machine::{advance_chase, ChaseObstacle, ChaseStatus};
use crate::combat_state_machine::{
    apply_damage_with_armor, resolve_attack, CombatActionKind, CombatCondition,
    CombatDamageFormula, CombatDefense,
};
use crate::dice_roll_contract::{
    server_roll_skill_check, DiceAdjustment, ServerDiceRoll, SuccessLevel,
};
use crate::{append_coc7_event, Coc7EventPayload};
use serde::Serialize;
use serde_json::Value;
use trpg_contracts::EventType;
use trpg_shared_kernel::{
    server_damage_roll, AuthorityContract, CommandEnvelope, EntityId, EventEnvelope, EventStore,
    KernelResult, ServerDamageRoll, TrpgError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coc7EngineDecision {
    SkillCheck,
    OpposedRoll,
    SanityCheck,
    CombatRound,
    ChaseRound,
    InvestigationStep,
}

pub fn engine_decision_route(decision: Coc7EngineDecision) -> &'static str {
    match decision {
        Coc7EngineDecision::SkillCheck => "skill_check",
        Coc7EngineDecision::OpposedRoll => "opposed_roll",
        Coc7EngineDecision::SanityCheck => "sanity_check",
        Coc7EngineDecision::CombatRound => "combat_round",
        Coc7EngineDecision::ChaseRound => "chase_round",
        Coc7EngineDecision::InvestigationStep => "investigation_step",
    }
}

pub const fn canonical_event_for_decision(decision: Coc7EngineDecision) -> EventType {
    match decision {
        Coc7EngineDecision::SkillCheck | Coc7EngineDecision::OpposedRoll => {
            EventType::SkillCheckResolved
        }
        Coc7EngineDecision::SanityCheck => EventType::SanityLossApplied,
        Coc7EngineDecision::CombatRound => EventType::CombatStateUpdated,
        Coc7EngineDecision::ChaseRound => EventType::ChaseSegmentResolved,
        Coc7EngineDecision::InvestigationStep => EventType::ClueRevealed,
    }
}

pub fn record_coc7_rules_engine_decision<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    decision: Coc7EngineDecision,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        canonical_event_for_decision(decision).name(),
        "coc7_rules_engine",
        format!("decision={}", engine_decision_route(decision)),
    )
}

/// Server-loaded, public-safe gameplay profiles. Transport adapters must
/// obtain these values from the approved character sheet and active scenario;
/// callers never supply rules statistics directly.
#[derive(Clone, Debug, PartialEq)]
pub struct PublicGameplayContext {
    pub npc_public_identity: String,
    pub character_combat_profile: Value,
    pub npc_combat_profile: Value,
    pub character_chase_profile: Value,
    pub npc_chase_profile: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicGameplayAction {
    NpcInteraction {
        character_id: String,
        npc_id: String,
        approach: String,
        public_response: String,
    },
    CombatRound {
        character_id: String,
        npc_id: String,
        action_kind: String,
        defense: String,
    },
    ChaseSegment {
        character_id: String,
        npc_id: String,
        initial_range: i8,
        obstacle_id: Option<String>,
        obstacle_cost: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublicPercentileRoll {
    pub roll_id: String,
    pub target: u8,
    pub roll: u8,
    pub selected_tens_digit: u8,
    pub ones_digit: u8,
    pub success_level: String,
    pub adjustment: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PublicDamageRoll {
    pub roll_id: String,
    pub dice_count: u8,
    pub die_sides: u8,
    pub flat_bonus: i8,
    pub dice_values: Vec<u8>,
    pub value: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicGameplayResolution {
    NpcInteraction {
        summary: String,
        character_id: String,
        npc_id: String,
        npc_public_identity: String,
        approach: String,
        public_response: String,
    },
    CombatRound {
        summary: String,
        character_id: String,
        npc_id: String,
        action_kind: String,
        defense: String,
        hit: bool,
        counterattack: bool,
        before_hp: u8,
        after_hp: u8,
        damage: u8,
        armor_absorbed: u8,
        condition: String,
        attacker_roll: PublicPercentileRoll,
        defender_roll: Option<PublicPercentileRoll>,
        damage_roll: Option<PublicDamageRoll>,
        random_source: &'static str,
    },
    ChaseSegment {
        summary: String,
        character_id: String,
        npc_id: String,
        before_range: i8,
        after_range: i8,
        status: String,
        obstacle_id: Option<String>,
        obstacle_cost: u8,
        quarry_roll: PublicPercentileRoll,
        pursuer_roll: PublicPercentileRoll,
        random_source: &'static str,
    },
}

impl PublicGameplayResolution {
    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::NpcInteraction { .. } => EventType::Coc7NpcDecisionRecorded.name(),
            Self::CombatRound { .. } => EventType::CombatStateUpdated.name(),
            Self::ChaseSegment { .. } => EventType::ChaseSegmentResolved.name(),
        }
    }
}

/// Resolves the RF04 public product slice entirely inside the COC7 rules
/// boundary. IDs and narrative choices are commands; all statistics come from
/// `PublicGameplayContext`, and every random value comes from the server OS
/// CSPRNG.
pub fn resolve_public_gameplay(
    context: &PublicGameplayContext,
    action: &PublicGameplayAction,
) -> KernelResult<PublicGameplayResolution> {
    match action {
        PublicGameplayAction::NpcInteraction {
            character_id,
            npc_id,
            approach,
            public_response,
        } => {
            validate_participant_ids(character_id, npc_id)?;
            if context.npc_public_identity.trim().is_empty()
                || context.npc_public_identity.len() > 256
                || approach.trim().is_empty()
                || approach.len() > 500
                || public_response.trim().is_empty()
                || public_response.len() > 2_000
            {
                return Err(TrpgError::InvalidConfiguration("npc_interaction"));
            }
            Ok(PublicGameplayResolution::NpcInteraction {
                summary: format!("与{}的互动已形成正式记录", context.npc_public_identity),
                character_id: character_id.clone(),
                npc_id: npc_id.clone(),
                npc_public_identity: context.npc_public_identity.clone(),
                approach: approach.clone(),
                public_response: public_response.clone(),
            })
        }
        PublicGameplayAction::CombatRound {
            character_id,
            npc_id,
            action_kind,
            defense,
        } => resolve_public_combat_round(context, character_id, npc_id, action_kind, defense),
        PublicGameplayAction::ChaseSegment {
            character_id,
            npc_id,
            initial_range,
            obstacle_id,
            obstacle_cost,
        } => resolve_public_chase_segment(
            context,
            character_id,
            npc_id,
            *initial_range,
            obstacle_id.as_deref(),
            *obstacle_cost,
        ),
    }
}

fn resolve_public_combat_round(
    context: &PublicGameplayContext,
    character_id: &str,
    npc_id: &str,
    action_kind: &str,
    defense: &str,
) -> KernelResult<PublicGameplayResolution> {
    validate_participant_ids(character_id, npc_id)?;
    let attacker = CombatProfile::parse(&context.character_combat_profile)?;
    let defender = CombatProfile::parse(&context.npc_combat_profile)?;
    let (action, attack_target, damage_formula) = match action_kind {
        "MELEE" => (
            CombatActionKind::Melee,
            attacker.melee,
            attacker.melee_damage,
        ),
        "FIREARM" => (
            CombatActionKind::Firearm,
            attacker.firearm,
            attacker.firearm_damage,
        ),
        _ => return Err(TrpgError::InvalidConfiguration("public_combat_action")),
    };
    let (defense_kind, defense_target) = match defense {
        "NONE" => (CombatDefense::None, None),
        "DODGE" => (CombatDefense::Dodge, Some(defender.dodge)),
        _ => return Err(TrpgError::InvalidConfiguration("public_combat_defense")),
    };
    let attacker_roll = server_roll_skill_check(attack_target, DiceAdjustment::None)?;
    let defender_roll = defense_target
        .map(|target| server_roll_skill_check(target, DiceAdjustment::None))
        .transpose()?;
    let attack = resolve_attack(action, &attacker_roll, defense_kind, defender_roll.as_ref())?;
    let damage_roll = attack
        .hit
        .then(|| {
            server_damage_roll(
                damage_formula.dice_count(),
                damage_formula.die_sides(),
                damage_formula.flat_bonus(),
            )
        })
        .transpose()?;
    let transition = damage_roll
        .as_ref()
        .map(|roll| {
            apply_damage_with_armor(
                defender.current_hp,
                defender.max_hp,
                roll.value(),
                defender.armor,
                defender.condition,
            )
        })
        .transpose()?;
    let after_hp = transition
        .as_ref()
        .map_or(defender.current_hp, |value| value.after_hp);
    let damage = transition.as_ref().map_or(0, |value| value.damage);
    let armor_absorbed = transition.as_ref().map_or(0, |value| value.armor_absorbed);
    let condition = transition
        .as_ref()
        .map_or(defender.condition, |value| value.condition);
    Ok(PublicGameplayResolution::CombatRound {
        summary: format!(
            "基础战斗轮已结算：{}，目标 HP {}→{}",
            if attack.hit { "命中" } else { "未命中" },
            defender.current_hp,
            after_hp
        ),
        character_id: character_id.to_owned(),
        npc_id: npc_id.to_owned(),
        action_kind: action_kind.to_owned(),
        defense: defense.to_owned(),
        hit: attack.hit,
        counterattack: attack.counterattack,
        before_hp: defender.current_hp,
        after_hp,
        damage,
        armor_absorbed,
        condition: combat_condition_name(condition).to_owned(),
        attacker_roll: public_percentile_roll(&attacker_roll),
        defender_roll: defender_roll.as_ref().map(public_percentile_roll),
        damage_roll: damage_roll.as_ref().map(public_damage_roll),
        random_source: "SERVER_OS_CSPRNG",
    })
}

fn resolve_public_chase_segment(
    context: &PublicGameplayContext,
    character_id: &str,
    npc_id: &str,
    initial_range: i8,
    obstacle_id: Option<&str>,
    obstacle_cost: u8,
) -> KernelResult<PublicGameplayResolution> {
    validate_participant_ids(character_id, npc_id)?;
    let quarry = ChaseProfile::parse(&context.character_chase_profile, "QUARRY")?;
    let pursuer = ChaseProfile::parse(&context.npc_chase_profile, "PURSUER")?;
    if obstacle_id.is_none() && obstacle_cost != 0 {
        return Err(TrpgError::InvalidConfiguration("public_chase_obstacle"));
    }
    let obstacle = obstacle_id
        .map(|id| ChaseObstacle::new(id, obstacle_cost))
        .transpose()?;
    let quarry_roll = server_roll_skill_check(
        quarry
            .movement_rate
            .checked_mul(5)
            .ok_or(TrpgError::InvalidConfiguration("public_chase_target"))?,
        DiceAdjustment::None,
    )?;
    let pursuer_roll = server_roll_skill_check(
        pursuer
            .movement_rate
            .checked_mul(5)
            .ok_or(TrpgError::InvalidConfiguration("public_chase_target"))?,
        DiceAdjustment::None,
    )?;
    let transition = advance_chase(
        initial_range,
        ChaseStatus::Ongoing,
        roll_succeeded(&quarry_roll),
        roll_succeeded(&pursuer_roll),
        obstacle.as_ref().map_or(0, |value| value.cost),
    )?;
    Ok(PublicGameplayResolution::ChaseSegment {
        summary: format!(
            "基础追逐段已结算：距离 {}→{}，状态 {}",
            transition.before_range,
            transition.after_range,
            chase_status_name(transition.status)
        ),
        character_id: character_id.to_owned(),
        npc_id: npc_id.to_owned(),
        before_range: transition.before_range,
        after_range: transition.after_range,
        status: chase_status_name(transition.status).to_owned(),
        obstacle_id: obstacle_id.map(str::to_owned),
        obstacle_cost: transition.obstacle_cost,
        quarry_roll: public_percentile_roll(&quarry_roll),
        pursuer_roll: public_percentile_roll(&pursuer_roll),
        random_source: "SERVER_OS_CSPRNG",
    })
}

#[derive(Clone, Copy)]
struct CombatProfile {
    melee: u8,
    firearm: u8,
    dodge: u8,
    melee_damage: CombatDamageFormula,
    firearm_damage: CombatDamageFormula,
    current_hp: u8,
    max_hp: u8,
    armor: u8,
    condition: CombatCondition,
}

impl CombatProfile {
    fn parse(value: &Value) -> KernelResult<Self> {
        let melee = json_u8(value, "/skill_targets/melee")?;
        let firearm = json_u8(value, "/skill_targets/firearm")?;
        let dodge = json_u8(value, "/skill_targets/dodge")?;
        if !(1..=100).contains(&melee)
            || !(1..=100).contains(&firearm)
            || !(1..=100).contains(&dodge)
        {
            return Err(TrpgError::InvalidConfiguration("public_combat_profile"));
        }
        let current_hp = json_u8(value, "/current_hp")?;
        let max_hp = json_u8(value, "/max_hp")?;
        let armor = json_u8(value, "/armor")?;
        let condition = match json_str(value, "/condition")? {
            "ABLE" => CombatCondition::Able,
            "MAJOR_WOUND" => CombatCondition::MajorWound,
            "DYING" => CombatCondition::Dying,
            "DEAD" => CombatCondition::Dead,
            _ => return Err(TrpgError::InvalidConfiguration("public_combat_profile")),
        };
        // Reuse the state-machine health invariants instead of accepting a
        // transport-defined notion of valid hit points.
        crate::combat_state_machine::CombatHealth::new(current_hp, max_hp, condition)?;
        if armor > 30 {
            return Err(TrpgError::InvalidConfiguration("public_combat_profile"));
        }
        Ok(Self {
            melee,
            firearm,
            dodge,
            melee_damage: damage_formula(value, "/weapon_loadout/melee/damage_formula")?,
            firearm_damage: damage_formula(value, "/weapon_loadout/firearm/damage_formula")?,
            current_hp,
            max_hp,
            armor,
            condition,
        })
    }
}

#[derive(Clone, Copy)]
struct ChaseProfile {
    movement_rate: u8,
}

impl ChaseProfile {
    fn parse(value: &Value, expected_role: &str) -> KernelResult<Self> {
        let movement_rate = json_u8(value, "/movement_rate")?;
        if json_str(value, "/role")? != expected_role || !(1..=20).contains(&movement_rate) {
            return Err(TrpgError::InvalidConfiguration("public_chase_profile"));
        }
        Ok(Self { movement_rate })
    }
}

fn damage_formula(value: &Value, pointer: &str) -> KernelResult<CombatDamageFormula> {
    let formula = value
        .pointer(pointer)
        .ok_or(TrpgError::InvalidConfiguration("public_combat_profile"))?;
    CombatDamageFormula::new(
        json_u8(formula, "/dice_count")?,
        json_u8(formula, "/die_sides")?,
        formula
            .pointer("/flat_bonus")
            .and_then(Value::as_i64)
            .and_then(|number| i8::try_from(number).ok())
            .ok_or(TrpgError::InvalidConfiguration("public_combat_profile"))?,
    )
}

fn json_u8(value: &Value, pointer: &str) -> KernelResult<u8> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .and_then(|number| u8::try_from(number).ok())
        .ok_or(TrpgError::InvalidConfiguration("public_gameplay_profile"))
}

fn json_str<'a>(value: &'a Value, pointer: &str) -> KernelResult<&'a str> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or(TrpgError::InvalidConfiguration("public_gameplay_profile"))
}

fn validate_participant_ids(character_id: &str, npc_id: &str) -> KernelResult<()> {
    if character_id == npc_id
        || EntityId::new(character_id).is_err()
        || EntityId::new(npc_id).is_err()
    {
        return Err(TrpgError::InvalidConfiguration(
            "public_gameplay_participants",
        ));
    }
    Ok(())
}

fn public_percentile_roll(roll: &ServerDiceRoll) -> PublicPercentileRoll {
    PublicPercentileRoll {
        roll_id: roll.roll_id().to_owned(),
        target: roll.outcome().target,
        roll: roll.outcome().roll,
        selected_tens_digit: roll.outcome().selected_tens_digit,
        ones_digit: roll.outcome().ones_digit,
        success_level: success_level_name(roll.outcome().success_level).to_owned(),
        adjustment: "NONE".to_owned(),
    }
}

fn public_damage_roll(roll: &ServerDamageRoll) -> PublicDamageRoll {
    PublicDamageRoll {
        roll_id: roll.roll_id().to_owned(),
        dice_count: roll.dice_count(),
        die_sides: roll.die_sides(),
        flat_bonus: roll.flat_bonus(),
        dice_values: roll.dice_values().to_vec(),
        value: roll.value(),
    }
}

fn roll_succeeded(roll: &ServerDiceRoll) -> bool {
    matches!(
        roll.outcome().success_level,
        SuccessLevel::Critical | SuccessLevel::Extreme | SuccessLevel::Hard | SuccessLevel::Regular
    )
}

const fn success_level_name(value: SuccessLevel) -> &'static str {
    match value {
        SuccessLevel::Critical => "CRITICAL",
        SuccessLevel::Extreme => "EXTREME",
        SuccessLevel::Hard => "HARD",
        SuccessLevel::Regular => "REGULAR",
        SuccessLevel::Failure => "FAILURE",
        SuccessLevel::Fumble => "FUMBLE",
    }
}

const fn combat_condition_name(value: CombatCondition) -> &'static str {
    match value {
        CombatCondition::Able => "ABLE",
        CombatCondition::MajorWound => "MAJOR_WOUND",
        CombatCondition::Dying => "DYING",
        CombatCondition::Dead => "DEAD",
    }
}

const fn chase_status_name(value: ChaseStatus) -> &'static str {
    match value {
        ChaseStatus::Ongoing => "ONGOING",
        ChaseStatus::Escaped => "ESCAPED",
        ChaseStatus::Caught => "CAUGHT",
    }
}
