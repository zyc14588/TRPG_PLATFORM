use crate::runtime_state_machines::{create_pending_decision, PendingDecision, RuntimeDecision};
use trpg_shared_kernel::AuthorityMode;

pub fn open_pending_decision(
    authority_mode: &AuthorityMode,
    decision: RuntimeDecision,
) -> PendingDecision {
    create_pending_decision(authority_mode, decision)
}
