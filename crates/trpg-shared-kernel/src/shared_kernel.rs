// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("shared_kernel/01_module_prelude.rs");
include!("shared_kernel/02_visibility_new.rs");
include!("shared_kernel/03_fact_provenance_new.rs");
include!("shared_kernel/04_authority_version_snapshot_from_draft.rs");
include!("shared_kernel/05_validate_command_envelope.rs");
include!("shared_kernel/06_event_store_validate_append.rs");
include!("shared_kernel/07_event_store_replay_visible.rs");
