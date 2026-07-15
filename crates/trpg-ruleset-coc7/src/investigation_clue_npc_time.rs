use crate::{append_coc7_event, Coc7EventPayload};
use trpg_contracts::EventType;
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClueImportance {
    Core,
    Optional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClueOutcome {
    Revealed,
    RevealedWithCost,
    NotFound,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClueResolution {
    pub importance: ClueImportance,
    pub outcome: ClueOutcome,
    pub cost: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeAdvance {
    pub before_minutes: u32,
    pub after_minutes: u32,
    pub reason: &'static str,
}

pub fn resolve_clue_check(importance: ClueImportance, succeeded: bool) -> ClueResolution {
    let outcome = match (importance, succeeded) {
        (_, true) => ClueOutcome::Revealed,
        (ClueImportance::Core, false) => ClueOutcome::RevealedWithCost,
        (ClueImportance::Optional, false) => ClueOutcome::NotFound,
    };
    let cost = if outcome == ClueOutcome::RevealedWithCost {
        Some("time_or_complication")
    } else {
        None
    };

    ClueResolution {
        importance,
        outcome,
        cost,
    }
}

pub fn advance_time(
    before_minutes: u32,
    delta_minutes: u32,
    reason: &'static str,
) -> KernelResult<TimeAdvance> {
    if delta_minutes == 0 || reason.trim().is_empty() {
        return Err(TrpgError::InvalidConfiguration("time_advance"));
    }

    Ok(TimeAdvance {
        before_minutes,
        after_minutes: before_minutes.saturating_add(delta_minutes),
        reason,
    })
}

pub fn record_investigation_clue_npc_time_decision<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    resolution: &ClueResolution,
) -> KernelResult<EventEnvelope<Coc7EventPayload>> {
    append_coc7_event(
        contract,
        store,
        command,
        EventType::ClueRevealed.name(),
        "investigation_clue_npc_time",
        format!("clue={:?} cost={:?}", resolution.outcome, resolution.cost),
    )
}
