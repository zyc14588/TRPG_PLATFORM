use crate::runtime_state_machines::{create_pending_decision, PendingDecision, RuntimeDecision};
use trpg_shared_kernel::AuthorityMode;

pub const PROMPT_ID: &str = "CODEX-0033-03-RUNTIME-ORCHESTRATION-0d6882e9c6";

pub fn open_pending_decision(
    authority_mode: &AuthorityMode,
    decision: RuntimeDecision,
) -> PendingDecision {
    create_pending_decision(authority_mode, decision)
}
