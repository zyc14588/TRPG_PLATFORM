// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

const S06_STAGE_FIXTURE: &str =
    include_str!("../../../fixtures/stages/S06_stage_acceptance_fixture.v1.json.md");
const S06_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S06_decision_pipeline_commit_expected.current.json.md"
);

include!("batch_012_runtime_contract_tests/01_module_prelude.rs");
include!("batch_012_runtime_contract_tests/02_human_kp_ai_formal_tool_becomes_draft_only_pending_decision.rs");
include!("batch_012_runtime_contract_tests/03_p04_runtime_rejects_corrupt_batch_receipt_without_partial_local_events.rs");
