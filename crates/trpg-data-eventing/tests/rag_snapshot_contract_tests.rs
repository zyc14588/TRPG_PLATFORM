// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

mod support;

include!("rag_snapshot_contract_tests/01_module_prelude.rs");
include!("rag_snapshot_contract_tests/02_pgvector_snapshot_is_rebuildable_and_filters_visibility_before_materialization.rs");
include!("rag_snapshot_contract_tests/03_raw_snapshot_chunk_insert.rs");
