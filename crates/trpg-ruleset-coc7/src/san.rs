use crate::dice_roll_contract::{success_level, SuccessLevel};
use crate::sanity_madness_state_machine::{apply_sanity_loss, SanityTransition};
use crate::{append_coc7_event, Coc7EventPayload};
use trpg_contracts::EventType;
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult,
};

pub fn san_check_succeeds(roll: u8, current_sanity: u8) -> KernelResult<bool> {
    Ok(!matches!(
        success_level(roll, current_sanity)?,
        SuccessLevel::Failure | SuccessLevel::Fumble
    ))
}

pub fn resolve_san_check(
    roll: u8,
    current_sanity: u8,
    success_loss: u8,
    failure_loss: u8,
    prior_day_loss: u8,
) -> KernelResult<SanityTransition> {
    let loss = if san_check_succeeds(roll, current_sanity)? {
        success_loss
    } else {
        failure_loss
    };

    apply_sanity_loss(current_sanity, loss, prior_day_loss)
}

pub fn record_san_decision<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    transition: &SanityTransition,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        EventType::SanityLossApplied.name(),
        "san",
        format!(
            "san decision after={} state={:?}",
            transition.after, transition.state
        ),
    )
}
