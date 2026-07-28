// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("security_privacy/01_module_prelude.rs");
include!("security_privacy/02_postgres_cloud_egress_ledger_new.rs");
include!("security_privacy/03_deletion_request_port_request_deletion.rs");
include!("security_privacy/04_postgres_deletion_repository_connect.rs");
include!("security_privacy/05_postgres_deletion_repository_refresh_execution_lease.rs");
include!("security_privacy/06_postgres_deletion_repository_claim_execution.rs");
include!("security_privacy/07_deletion_batch_progress_complete.rs");
include!("security_privacy/08_deletion_surface_target.rs");
include!("security_privacy/09_nats_queue_deletion_surface_connect.rs");
include!("security_privacy/10_deletion_surface_target.rs");
include!("security_privacy/11_deletion_surface_target.rs");
include!("security_privacy/12_deletion_worker_new.rs");
