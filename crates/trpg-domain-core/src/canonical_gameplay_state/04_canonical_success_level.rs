
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
    consumed_roll_ids: Vec<String>,
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
            || !next.consumed_roll_ids.is_empty()
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
        || state
            .consumed_roll_ids
            .iter()
            .any(|roll_id| !valid_id(roll_id))
        || state.consumed_roll_ids.iter().collect::<HashSet<_>>().len()
            != state.consumed_roll_ids.len()
        || state.version == 0
    {
        return Err(CanonicalGameplayStateError::InvalidShape);
    }
    Ok(state)
}
