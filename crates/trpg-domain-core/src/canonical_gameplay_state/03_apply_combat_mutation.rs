
fn apply_combat_mutation(
    state: &mut CombatSnapshot,
    mutation: &CombatMutation,
) -> Result<(), CanonicalGameplayStateError> {
    if state.status != CombatStatus::Ongoing {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    match mutation {
        CombatMutation::Started => return Err(CanonicalGameplayStateError::InvalidTransition),
        CombatMutation::AttackMissed {
            attacker_id,
            target_id,
            action,
            defense,
            attacker_roll,
            defender_roll,
        } => {
            let current_actor = state
                .initiative_order
                .get(state.current_turn_index)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            if current_actor != attacker_id
                || attacker_id == target_id
                || state.turn_action_consumed
            {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            let attacker = state
                .participants
                .iter()
                .find(|participant| participant.participant_id == *attacker_id)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            if !attacker.condition.can_act() {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            let attacker_target = attacker.skill_targets.attack_target(*action);
            let defender = state
                .participants
                .iter()
                .find(|participant| participant.participant_id == *target_id)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            if defender.condition == CombatCondition::Dead {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            if *defense != CombatDefense::None && !defender.condition.can_act() {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
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
            let mut roll_ids = vec![attacker_roll.roll_id.as_str()];
            if let Some(defender_roll) = defender_roll {
                roll_ids.push(defender_roll.roll_id.as_str());
            }
            if roll_ids.iter().collect::<HashSet<_>>().len() != roll_ids.len()
                || state
                    .consumed_roll_ids
                    .iter()
                    .any(|consumed| roll_ids.contains(&consumed.as_str()))
                || canonical_exchange_outcome(
                    *defense,
                    attacker_roll.success_level,
                    defender_roll.as_ref().map(|roll| roll.success_level),
                )?
                .is_some()
            {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            state
                .consumed_roll_ids
                .extend(roll_ids.into_iter().map(str::to_owned));
            state.turn_action_consumed = true;
        }
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
            if current_actor != attacker_id
                || attacker_id == target_id
                || state.turn_action_consumed
            {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            let attacker = state
                .participants
                .iter()
                .find(|participant| participant.participant_id == *attacker_id)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            if !attacker.condition.can_act() {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            let attacker_target = attacker.skill_targets.attack_target(*action);
            let defender = state
                .participants
                .iter()
                .find(|participant| participant.participant_id == *target_id)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            if defender.condition == CombatCondition::Dead {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            if *defense != CombatDefense::None && !defender.condition.can_act() {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
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
            let mut roll_ids = vec![attacker_roll.roll_id.as_str()];
            if let Some(defender_roll) = defender_roll {
                roll_ids.push(defender_roll.roll_id.as_str());
            }
            roll_ids.push(damage_roll.roll_id.as_str());
            if roll_ids.iter().collect::<HashSet<_>>().len() != roll_ids.len()
                || state
                    .consumed_roll_ids
                    .iter()
                    .any(|consumed| roll_ids.contains(&consumed.as_str()))
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
            let expected_damage_formula = match outcome {
                CombatExchangeOutcome::AttackerHit => {
                    attacker.weapon_loadout.damage_formula(*action)
                }
                CombatExchangeOutcome::DefenderFoughtBack => defender
                    .weapon_loadout
                    .damage_formula(CombatActionKind::Melee),
            };
            validate_damage_evidence(damage_roll, expected_damage_formula)?;
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
            state
                .consumed_roll_ids
                .extend(roll_ids.into_iter().map(str::to_owned));
            state.turn_action_consumed = true;
        }
        CombatMutation::MajorWoundRecoveryAttempted {
            healer_id,
            target_id,
            medical_skill,
            medical_roll,
            recovered,
        } => {
            let current_actor = state
                .initiative_order
                .get(state.current_turn_index)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            let healer = state
                .participants
                .iter()
                .find(|participant| participant.participant_id == *healer_id)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            if current_actor != healer_id
                || !healer.condition.can_act()
                || state.turn_action_consumed
            {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            validate_percentile_evidence(
                medical_roll,
                healer.skill_targets.medical_target(*medical_skill),
            )?;
            if state
                .consumed_roll_ids
                .iter()
                .any(|consumed| consumed == &medical_roll.roll_id)
            {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            let target = state
                .participants
                .iter_mut()
                .find(|participant| participant.participant_id == *target_id)
                .ok_or(CanonicalGameplayStateError::InvalidTransition)?;
            let stabilizes_dying = target.current_hp == 0
                && target.condition == CombatCondition::Dying
                && *medical_skill == CombatMedicalSkill::FirstAid;
            let treats_major_wound =
                target.current_hp > 0 && target.condition == CombatCondition::MajorWound;
            if !stabilizes_dying && !treats_major_wound {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            let derived_recovered = success_rank(medical_roll.success_level) > 0;
            if derived_recovered != *recovered {
                return Err(CanonicalGameplayStateError::InvalidTransition);
            }
            if derived_recovered {
                if stabilizes_dying {
                    target.current_hp = 1;
                    target.condition = CombatCondition::MajorWound;
                } else {
                    target.condition = CombatCondition::Able;
                }
            }
            state.consumed_roll_ids.push(medical_roll.roll_id.clone());
            state.turn_action_consumed = true;
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
                    state.turn_action_consumed = false;
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
    if evidence.selected_tens_digit > 9 || evidence.ones_digit > 9 {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    let reconstructed = if evidence.selected_tens_digit == 0 && evidence.ones_digit == 0 {
        100
    } else {
        evidence.selected_tens_digit * 10 + evidence.ones_digit
    };
    if !valid_id(&evidence.roll_id)
        || evidence.target != expected_target
        || reconstructed != evidence.roll
        || canonical_success_level(evidence.roll, evidence.target)? != evidence.success_level
    {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    Ok(())
}

fn validate_damage_evidence(
    evidence: &DamageRollEvidence,
    expected_formula: CombatDamageFormula,
) -> Result<(), CanonicalGameplayStateError> {
    let total = evidence
        .dice_values
        .iter()
        .try_fold(i16::from(evidence.flat_bonus), |sum, value| {
            sum.checked_add(i16::from(*value))
        })
        .and_then(|value| u8::try_from(value).ok());
    if !valid_id(&evidence.roll_id)
        || (evidence.dice_count, evidence.die_sides, evidence.flat_bonus)
            != (
                expected_formula.dice_count,
                expected_formula.die_sides,
                expected_formula.flat_bonus,
            )
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
