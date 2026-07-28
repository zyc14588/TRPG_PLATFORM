// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

const API_WS_NATS_FIXTURE: &str =
    include_str!("../../../fixtures/api/api_ws_nats_contract_cases.v1.json.md");
const RAG_SNAPSHOT_FIXTURE: &str =
    include_str!("../../../fixtures/rag/rag_snapshot_cases.v1.json.md");
const S03_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S03_event_store_projection_hash.current.json.md"
);

include!("batch_026_data_eventing_contract_tests/01_module_prelude.rs");
include!("batch_026_data_eventing_contract_tests/02_b026_appends_only_through_governed_event_store_path.rs");
