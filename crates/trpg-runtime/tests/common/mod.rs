use std::sync::atomic::{AtomicU64, Ordering};

use trpg_runtime::runtime_state_machines::RuntimeEventPayload;
use trpg_runtime::{EventStore, FormalCommitAudit};

static NEXT_AUDIT_ID: AtomicU64 = AtomicU64::new(1);

pub fn audited_store() -> EventStore<RuntimeEventPayload> {
    let audit_id = NEXT_AUDIT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "p02-runtime-integration-audit-{}-{audit_id}.jsonl",
        std::process::id()
    ));
    let audit = FormalCommitAudit::open(path, "runtime-integration-test-v1", &[0x84; 32]).unwrap();
    EventStore::with_formal_audit(audit)
}
