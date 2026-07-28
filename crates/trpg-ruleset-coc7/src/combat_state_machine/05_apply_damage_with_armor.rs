
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
