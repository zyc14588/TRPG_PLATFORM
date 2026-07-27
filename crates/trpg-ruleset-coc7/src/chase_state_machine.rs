use crate::dice_roll_contract::{success_level, SuccessLevel};
use crate::{append_coc7_event, Coc7EventPayload};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use trpg_contracts::EventType;
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult,
    ServerPercentileRoll, TrpgError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChaseStatus {
    Ongoing,
    Escaped,
    Caught,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChaseRole {
    Quarry,
    Pursuer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChaseParticipant {
    participant_id: String,
    role: ChaseRole,
    movement_rate: u8,
}

impl ChaseParticipant {
    pub fn new(
        participant_id: impl Into<String>,
        role: ChaseRole,
        movement_rate: u8,
    ) -> KernelResult<Self> {
        let participant_id = participant_id.into();
        if !valid_chase_id(&participant_id) || !(1..=20).contains(&movement_rate) {
            return Err(TrpgError::InvalidConfiguration("chase_participant"));
        }
        Ok(Self {
            participant_id,
            role,
            movement_rate,
        })
    }

    pub fn participant_id(&self) -> &str {
        &self.participant_id
    }

    pub const fn role(&self) -> ChaseRole {
        self.role
    }

    pub const fn movement_rate(&self) -> u8 {
        self.movement_rate
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ChaseObstacle {
    pub obstacle_id: String,
    pub cost: u8,
}

impl ChaseObstacle {
    pub fn new(obstacle_id: impl Into<String>, cost: u8) -> KernelResult<Self> {
        let obstacle_id = obstacle_id.into();
        if !valid_chase_id(&obstacle_id) || cost > 2 {
            return Err(TrpgError::InvalidConfiguration("chase_obstacle"));
        }
        Ok(Self { obstacle_id, cost })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ChaseTransition {
    pub before_range: i8,
    pub after_range: i8,
    pub obstacle_cost: u8,
    pub before_status: ChaseStatus,
    pub status: ChaseStatus,
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

impl ChaseParticipantRollEvidence {
    fn from_server_roll(
        participant: &ChaseParticipant,
        roll: &ServerPercentileRoll,
    ) -> KernelResult<Self> {
        let target = participant
            .movement_rate
            .checked_mul(5)
            .ok_or(TrpgError::InvalidConfiguration("chase_roll_target"))?;
        Ok(Self {
            participant_id: participant.participant_id.clone(),
            roll_id: roll.roll_id().to_owned(),
            target,
            roll: roll.value(),
            selected_tens_digit: roll.selected_tens_digit(),
            ones_digit: roll.ones_digit(),
            success_level: success_level(roll.value(), target)?,
        })
    }

    fn validate(&self, participant: &ChaseParticipant) -> KernelResult<()> {
        let target = participant
            .movement_rate
            .checked_mul(5)
            .ok_or(TrpgError::InvalidConfiguration("chase_roll_target"))?;
        let reconstructed = if self.selected_tens_digit == 0 && self.ones_digit == 0 {
            100
        } else {
            self.selected_tens_digit * 10 + self.ones_digit
        };
        if self.participant_id != participant.participant_id
            || !valid_chase_id(&self.roll_id)
            || self.target != target
            || self.selected_tens_digit > 9
            || self.ones_digit > 9
            || reconstructed != self.roll
            || success_level(self.roll, target)? != self.success_level
        {
            return Err(TrpgError::InvalidConfiguration("chase_roll_evidence"));
        }
        Ok(())
    }

    fn succeeded(&self) -> bool {
        matches!(
            self.success_level,
            SuccessLevel::Critical
                | SuccessLevel::Extreme
                | SuccessLevel::Hard
                | SuccessLevel::Regular
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChaseState {
    chase_id: String,
    participants: Vec<ChaseParticipant>,
    range: i8,
    segment: u32,
    status: ChaseStatus,
    version: u64,
    last_transition: ChaseMutation,
}

#[derive(Deserialize)]
struct ChaseParticipantWire {
    participant_id: String,
    role: ChaseRole,
    movement_rate: u8,
}

#[derive(Deserialize)]
struct ChaseStateWire {
    chase_id: String,
    participants: Vec<ChaseParticipantWire>,
    range: i8,
    segment: u32,
    status: ChaseStatus,
    version: u64,
    last_transition: ChaseMutation,
}

impl ChaseState {
    pub fn start(
        chase_id: impl Into<String>,
        participants: Vec<ChaseParticipant>,
        initial_range: i8,
    ) -> KernelResult<Self> {
        let chase_id = chase_id.into();
        let unique: HashSet<&str> = participants
            .iter()
            .map(|participant| participant.participant_id.as_str())
            .collect();
        let quarry_count = participants
            .iter()
            .filter(|participant| participant.role == ChaseRole::Quarry)
            .count();
        let pursuer_count = participants
            .iter()
            .filter(|participant| participant.role == ChaseRole::Pursuer)
            .count();
        if !valid_chase_id(&chase_id)
            || !(1..=4).contains(&initial_range)
            || quarry_count == 0
            || pursuer_count == 0
            || unique.len() != participants.len()
        {
            return Err(TrpgError::InvalidConfiguration("chase_state"));
        }
        Ok(Self {
            chase_id,
            participants,
            range: initial_range,
            segment: 1,
            status: ChaseStatus::Ongoing,
            version: 1,
            last_transition: ChaseMutation::Started,
        })
    }

    pub fn chase_id(&self) -> &str {
        &self.chase_id
    }

    pub fn participants(&self) -> &[ChaseParticipant] {
        &self.participants
    }

    pub const fn range(&self) -> i8 {
        self.range
    }

    pub const fn segment(&self) -> u32 {
        self.segment
    }

    pub const fn status(&self) -> ChaseStatus {
        self.status
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn advance(
        &mut self,
        participant_rolls: &[ServerPercentileRoll],
        obstacle: Option<&ChaseObstacle>,
    ) -> KernelResult<ChaseTransition> {
        if participant_rolls.len() != self.participants.len() {
            return Err(TrpgError::InvalidConfiguration("chase_roll_evidence"));
        }
        let rolls = self
            .participants
            .iter()
            .zip(participant_rolls)
            .map(|(participant, roll)| {
                ChaseParticipantRollEvidence::from_server_roll(participant, roll)
            })
            .collect::<KernelResult<Vec<_>>>()?;
        self.advance_verified(&rolls, obstacle)
    }

    fn advance_verified(
        &mut self,
        rolls: &[ChaseParticipantRollEvidence],
        obstacle: Option<&ChaseObstacle>,
    ) -> KernelResult<ChaseTransition> {
        if self.status != ChaseStatus::Ongoing || rolls.len() != self.participants.len() {
            return Err(TrpgError::InvalidConfiguration(
                if self.status != ChaseStatus::Ongoing {
                    "chase_terminal"
                } else {
                    "chase_roll_evidence"
                },
            ));
        }
        for (participant, roll) in self.participants.iter().zip(rolls) {
            roll.validate(participant)?;
        }
        if rolls
            .iter()
            .map(|roll| roll.roll_id.as_str())
            .collect::<HashSet<_>>()
            .len()
            != rolls.len()
        {
            return Err(TrpgError::InvalidConfiguration("chase_roll_reuse"));
        }
        let quarry_success = self
            .participants
            .iter()
            .zip(rolls)
            .filter(|(participant, _)| participant.role == ChaseRole::Quarry)
            .any(|(_, roll)| roll.succeeded());
        let pursuer_success = self
            .participants
            .iter()
            .zip(rolls)
            .filter(|(participant, _)| participant.role == ChaseRole::Pursuer)
            .any(|(_, roll)| roll.succeeded());
        let transition = advance_chase(
            self.range,
            self.status,
            quarry_success,
            pursuer_success,
            obstacle.map_or(0, |obstacle| obstacle.cost),
        )?;
        self.range = transition.after_range;
        self.status = transition.status;
        self.last_transition = ChaseMutation::Advanced {
            rolls: rolls.to_vec(),
            quarry_success,
            pursuer_success,
            obstacle_id: obstacle.map(|obstacle| obstacle.obstacle_id.clone()),
            obstacle_cost: obstacle.map_or(0, |obstacle| obstacle.cost),
        };
        self.segment = self
            .segment
            .checked_add(1)
            .ok_or(TrpgError::InvalidConfiguration("chase_segment"))?;
        self.version = self
            .version
            .checked_add(1)
            .ok_or(TrpgError::InvalidConfiguration("chase_version"))?;
        Ok(transition)
    }

    /// Serializes only a state-machine-produced aggregate. `ChaseState` does
    /// not implement `Deserialize`, preventing arbitrary caller JSON from
    /// becoming a formal chase state.
    pub fn persistence_json(&self) -> KernelResult<String> {
        serde_json::to_string(self)
            .map_err(|_| TrpgError::InvalidConfiguration("chase_state_serialization"))
    }

    pub fn validate_persistence_transition(
        &self,
        previous_state_json: Option<&str>,
    ) -> KernelResult<()> {
        let Some(previous_state_json) = previous_state_json else {
            if self.version == 1
                && self.segment == 1
                && self.status == ChaseStatus::Ongoing
                && (1..=4).contains(&self.range)
                && matches!(self.last_transition, ChaseMutation::Started)
            {
                return Ok(());
            }
            return Err(TrpgError::InvalidConfiguration("chase_initial_state"));
        };
        let wire: ChaseStateWire = serde_json::from_str(previous_state_json)
            .map_err(|_| TrpgError::InvalidConfiguration("chase_persisted_state"))?;
        let mut expected = Self::try_from_wire(wire)?;
        match &self.last_transition {
            ChaseMutation::Started => {
                return Err(TrpgError::InvalidConfiguration(
                    "chase_transition_restarted",
                ));
            }
            ChaseMutation::Advanced {
                rolls,
                quarry_success,
                pursuer_success,
                obstacle_id,
                obstacle_cost,
            } => {
                let obstacle = obstacle_id
                    .as_ref()
                    .map(|id| ChaseObstacle::new(id.clone(), *obstacle_cost))
                    .transpose()?;
                if obstacle.is_none() && *obstacle_cost != 0 {
                    return Err(TrpgError::InvalidConfiguration("chase_obstacle_evidence"));
                }
                let transition = expected.advance_verified(rolls, obstacle.as_ref())?;
                let ChaseMutation::Advanced {
                    quarry_success: derived_quarry_success,
                    pursuer_success: derived_pursuer_success,
                    ..
                } = &expected.last_transition
                else {
                    return Err(TrpgError::InvalidConfiguration("chase_roll_evidence"));
                };
                if derived_quarry_success != quarry_success
                    || derived_pursuer_success != pursuer_success
                    || transition.obstacle_cost != *obstacle_cost
                {
                    return Err(TrpgError::InvalidConfiguration("chase_roll_evidence"));
                }
            }
        }
        if expected == *self {
            Ok(())
        } else {
            Err(TrpgError::InvalidConfiguration("chase_transition_mismatch"))
        }
    }

    /// Validates a canonical replay transition while keeping `ChaseState`
    /// non-deserializable at command boundaries.
    pub fn validate_serialized_persistence_transition(
        previous_state_json: Option<&str>,
        next_state_json: &str,
    ) -> KernelResult<()> {
        let wire: ChaseStateWire = serde_json::from_str(next_state_json)
            .map_err(|_| TrpgError::InvalidConfiguration("chase_persisted_state"))?;
        let next = Self::try_from_wire(wire)?;
        next.validate_persistence_transition(previous_state_json)
    }

    fn try_from_wire(wire: ChaseStateWire) -> KernelResult<Self> {
        let participants = wire
            .participants
            .into_iter()
            .map(|participant| ChaseParticipant {
                participant_id: participant.participant_id,
                role: participant.role,
                movement_rate: participant.movement_rate,
            })
            .collect::<Vec<_>>();
        let unique = participants
            .iter()
            .map(|participant| participant.participant_id.as_str())
            .collect::<HashSet<_>>();
        let quarry_count = participants
            .iter()
            .filter(|participant| participant.role == ChaseRole::Quarry)
            .count();
        let pursuer_count = participants
            .iter()
            .filter(|participant| participant.role == ChaseRole::Pursuer)
            .count();
        if !valid_chase_id(&wire.chase_id)
            || unique.len() != participants.len()
            || quarry_count == 0
            || pursuer_count == 0
            || participants.iter().any(|participant| {
                !valid_chase_id(&participant.participant_id)
                    || !(1..=20).contains(&participant.movement_rate)
            })
            || !(0..=5).contains(&wire.range)
            || wire.segment == 0
            || wire.version == 0
        {
            return Err(TrpgError::InvalidConfiguration("chase_persisted_state"));
        }
        Ok(Self {
            chase_id: wire.chase_id,
            participants,
            range: wire.range,
            segment: wire.segment,
            status: wire.status,
            version: wire.version,
            last_transition: wire.last_transition,
        })
    }
}

pub fn advance_chase(
    current_range: i8,
    current_status: ChaseStatus,
    quarry_success: bool,
    pursuer_success: bool,
    obstacle_cost: u8,
) -> KernelResult<ChaseTransition> {
    if !(0..=5).contains(&current_range) || obstacle_cost > 2 {
        return Err(TrpgError::InvalidConfiguration("chase_range"));
    }
    if current_status != ChaseStatus::Ongoing {
        return Err(TrpgError::InvalidConfiguration("chase_terminal"));
    }

    let contest_delta = match (quarry_success, pursuer_success) {
        (true, false) => 1,
        (false, true) => -1,
        _ => 0,
    };
    let obstacle_delta = if quarry_success {
        0
    } else {
        -(obstacle_cost as i8)
    };
    let after_range = (current_range + contest_delta + obstacle_delta).clamp(0, 5);
    let status = if after_range >= 5 {
        ChaseStatus::Escaped
    } else if after_range <= 0 {
        ChaseStatus::Caught
    } else {
        ChaseStatus::Ongoing
    };

    Ok(ChaseTransition {
        before_range: current_range,
        after_range,
        obstacle_cost,
        before_status: current_status,
        status,
    })
}

pub fn record_chase_transition<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    transition: &ChaseTransition,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        EventType::ChaseSegmentResolved.name(),
        "chase_state_machine",
        format!(
            "range {}->{} status={:?}->{:?}",
            transition.before_range,
            transition.after_range,
            transition.before_status,
            transition.status
        ),
    )
}

fn valid_chase_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
