// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

const BASE_MIGRATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../migrations/20260705000100_create_data_eventing_event_store.up.sql"
));
const DURABLE_WORKFLOW_MIGRATION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../migrations/20260715000400_create_canonical_commit_protocol.up.sql"
));
include!("durable_workflow/01_module_prelude.rs");
include!("durable_workflow/02_durable_workflow_store_connect.rs");
include!("durable_workflow/03_validate_transition.rs");
include!("durable_workflow/04_agent_job_store.rs");
