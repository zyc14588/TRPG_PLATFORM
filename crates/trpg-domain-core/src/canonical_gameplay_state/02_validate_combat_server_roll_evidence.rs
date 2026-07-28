
pub fn validate_combat_server_roll_evidence(
    state_json: &str,
    attacker_roll: Option<&ServerPercentileRoll>,
    defender_roll: Option<&ServerPercentileRoll>,
    damage_roll: Option<&ServerDamageRoll>,
    medical_roll: Option<&ServerPercentileRoll>,
) -> Result<(), CanonicalGameplayStateError> {
    let state = parse_combat(state_json)?;
    match &state.last_transition {
        CombatMutation::AttackMissed {
            attacker_roll: recorded_attacker,
            defender_roll: recorded_defender,
            ..
        } => {
            let attacker_roll =
                attacker_roll.ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            if damage_roll.is_some()
                || medical_roll.is_some()
                || !percentile_evidence_matches_server(recorded_attacker, attacker_roll)
                || !match (recorded_defender, defender_roll) {
                    (Some(recorded), Some(server)) => {
                        percentile_evidence_matches_server(recorded, server)
                    }
                    (None, None) => true,
                    _ => false,
                }
            {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
        }
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
        CombatMutation::MajorWoundRecoveryAttempted {
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
        || state
            .consumed_roll_ids
            .iter()
            .any(|roll_id| !valid_id(roll_id))
        || state.consumed_roll_ids.iter().collect::<HashSet<_>>().len()
            != state.consumed_roll_ids.len()
        || initiative != state.initiative_order
        || state.round == 0
        || state.current_turn_index >= state.participants.len()
        || state.version == 0
    {
        return Err(CanonicalGameplayStateError::InvalidShape);
    }
    Ok(state)
}
