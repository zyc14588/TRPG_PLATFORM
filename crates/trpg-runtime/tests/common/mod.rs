use std::sync::atomic::{AtomicU64, Ordering};

use trpg_runtime::runtime_state_machines::RuntimeEventPayload;
use trpg_runtime::{AuthorityContract, EventStore, FormalCommitAudit, FormalCommitAuthorizer};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};

static NEXT_AUDIT_ID: AtomicU64 = AtomicU64::new(1);

pub fn audited_store(contract: &AuthorityContract) -> EventStore<RuntimeEventPayload> {
    let audit_id = NEXT_AUDIT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "p02-runtime-integration-audit-{}-{audit_id}.jsonl",
        std::process::id()
    ));
    let audit = FormalCommitAudit::open(path, "runtime-integration-test-v1", &[0x84; 32]).unwrap();
    let endpoints = trpg_test_support::formal_commit_policy_endpoints();
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            endpoints.openfga,
            "/stores/test/check",
            PolicyBackend::OpenFga,
            endpoints.openfga_model,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            endpoints.opa,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            endpoints.opa_revision,
        )
        .unwrap(),
    )
    .unwrap();
    let (identity_verifier, _) = trpg_test_support::formal_commit_identity_for_contract(contract);
    EventStore::with_formal_custody(
        FormalCommitAuthorizer::new(identity_verifier, policy, audit),
        trpg_test_support::test_canonical_commit_port(),
    )
}
