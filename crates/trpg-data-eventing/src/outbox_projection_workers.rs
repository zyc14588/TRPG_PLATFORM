// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("outbox_projection_workers/01_module_prelude.rs");
include!("outbox_projection_workers/02_postgres_outbox_lease_repository_new.rs");
include!("outbox_projection_workers/03_postgres_projection_worker_new.rs");
include!("outbox_projection_workers/04_postgres_projection_worker_rebuild_all_to_tip.rs");
include!("outbox_projection_workers/05_load_projection_events.rs");
