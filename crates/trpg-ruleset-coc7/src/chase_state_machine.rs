use crate::{append_coc7_event, Coc7EventPayload};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChaseStatus {
    Ongoing,
    Escaped,
    Caught,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChaseTransition {
    pub before_range: i8,
    pub after_range: i8,
    pub obstacle_cost: u8,
    pub status: ChaseStatus,
}

pub fn advance_chase(
    current_range: i8,
    quarry_success: bool,
    pursuer_success: bool,
    obstacle_cost: u8,
) -> KernelResult<ChaseTransition> {
    if !(0..=5).contains(&current_range) || obstacle_cost > 2 {
        return Err(TrpgError::InvalidConfiguration("chase_range"));
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
        "coc7.chase_transition_recorded",
        "chase_state_machine",
        format!(
            "range {}->{} status={:?}",
            transition.before_range, transition.after_range, transition.status
        ),
    )
}
