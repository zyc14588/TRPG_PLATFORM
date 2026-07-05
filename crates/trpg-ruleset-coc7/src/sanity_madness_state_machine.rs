use crate::{append_coc7_event, Coc7EventPayload};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MadnessState {
    Stable,
    TemporaryInsanity,
    IndefiniteInsanity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SanityTransition {
    pub before: u8,
    pub after: u8,
    pub loss: u8,
    pub day_loss: u8,
    pub state: MadnessState,
}

pub fn apply_sanity_loss(
    current: u8,
    loss: u8,
    prior_day_loss: u8,
) -> KernelResult<SanityTransition> {
    if current > 99 {
        return Err(TrpgError::InvalidConfiguration("sanity_range"));
    }

    let after = current.saturating_sub(loss);
    let day_loss = prior_day_loss.saturating_add(loss);
    let indefinite_threshold = (current / 5).max(1);
    let state = if day_loss >= indefinite_threshold {
        MadnessState::IndefiniteInsanity
    } else if loss >= 5 {
        MadnessState::TemporaryInsanity
    } else {
        MadnessState::Stable
    };

    Ok(SanityTransition {
        before: current,
        after,
        loss,
        day_loss,
        state,
    })
}

pub fn record_sanity_madness_transition<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    transition: &SanityTransition,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        "coc7.sanity_transition_recorded",
        "sanity_madness_state_machine",
        format!(
            "sanity {}->{} loss={} state={:?}",
            transition.before, transition.after, transition.loss, transition.state
        ),
    )
}
