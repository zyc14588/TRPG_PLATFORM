
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
    let roll_ids = rolls
        .iter()
        .map(|roll| roll.roll_id.as_str())
        .collect::<Vec<_>>();
    if roll_ids.iter().collect::<HashSet<_>>().len() != roll_ids.len()
        || state
            .consumed_roll_ids
            .iter()
            .any(|consumed| roll_ids.contains(&consumed.as_str()))
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
    state
        .consumed_roll_ids
        .extend(roll_ids.into_iter().map(str::to_owned));
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
    if evidence.selected_tens_digit > 9 || evidence.ones_digit > 9 {
        return Err(CanonicalGameplayStateError::InvalidTransition);
    }
    let reconstructed = if evidence.selected_tens_digit == 0 && evidence.ones_digit == 0 {
        100
    } else {
        evidence.selected_tens_digit * 10 + evidence.ones_digit
    };
    if evidence.participant_id != participant.participant_id
        || !valid_id(&evidence.roll_id)
        || evidence.target != target
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
