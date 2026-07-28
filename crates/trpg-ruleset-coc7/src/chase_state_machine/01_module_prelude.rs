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
        if self.selected_tens_digit > 9 || self.ones_digit > 9 {
            return Err(TrpgError::InvalidConfiguration("chase_roll_evidence"));
        }
        let reconstructed = if self.selected_tens_digit == 0 && self.ones_digit == 0 {
            100
        } else {
            self.selected_tens_digit * 10 + self.ones_digit
        };
        if self.participant_id != participant.participant_id
            || !valid_chase_id(&self.roll_id)
            || self.target != target
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
    consumed_roll_ids: Vec<String>,
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
    consumed_roll_ids: Vec<String>,
    status: ChaseStatus,
    version: u64,
    last_transition: ChaseMutation,
}
