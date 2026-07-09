use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEvent, TestingQualityEventEnvelope, TestingQualityModuleContract,
    TestingQualityRepository,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const PROMPT_ID: &str = "CODEX-0092-10-TESTING-QUALITY-d6a006e0a1";
pub const MODULE: &str = "testing_quality::replay_consistency_tests";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayProjection {
    pub event_count: usize,
    pub final_state_hash: u64,
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/replay_consistency_tests.rs",
        "crates/trpg-testing/tests/replay_consistency_tests_contract_tests.rs",
        TestingQualityAction::VerifyReplayConsistency,
        &[
            "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
            "test-data/export_expected_snapshots.md",
        ],
        &[
            "event_store_is_canon",
            "projection_is_rebuildable",
            "replay_hash_is_deterministic",
        ],
    )
}

pub fn replay_digest(events: &[TestingQualityEventEnvelope]) -> u64 {
    events.iter().fold(0xcbf29ce484222325, |hash, event| {
        let hash = mix_u64(hash, event.sequence);
        let hash = mix_str(hash, event.event_type);
        match &event.payload {
            TestingQualityEvent::ContractValidated { module, action, .. } => {
                mix_str(mix_str(hash, module), action.as_str())
            }
        }
    })
}

pub fn rebuild_projection(events: &[TestingQualityEventEnvelope]) -> ReplayProjection {
    ReplayProjection {
        event_count: events.len(),
        final_state_hash: replay_digest(events),
    }
}

fn mix_str(mut hash: u64, value: &str) -> u64 {
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn mix_u64(hash: u64, value: u64) -> u64 {
    mix_str(hash, &value.to_string())
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
