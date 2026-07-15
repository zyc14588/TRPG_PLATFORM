use std::sync::atomic::{AtomicU64, Ordering};

use trpg_agent_runtime::{AgentEventPayload, AgentEventStore, FormalCommitAudit};

static NEXT_AUDIT_ID: AtomicU64 = AtomicU64::new(1);

pub fn audited_store() -> AgentEventStore<AgentEventPayload> {
    audited_store_with_handle().0
}

pub fn audited_store_with_handle() -> (AgentEventStore<AgentEventPayload>, FormalCommitAudit) {
    let audit_id = NEXT_AUDIT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "p02-agent-integration-audit-{}-{audit_id}.jsonl",
        std::process::id()
    ));
    let audit = FormalCommitAudit::open(path, "agent-integration-test-v1", &[0x85; 32]).unwrap();
    (AgentEventStore::with_formal_audit(audit.clone()), audit)
}
