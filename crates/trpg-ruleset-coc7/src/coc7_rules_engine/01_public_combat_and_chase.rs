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
