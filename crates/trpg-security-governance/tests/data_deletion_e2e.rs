// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("data_deletion_e2e/01_module_prelude.rs");
include!(
    "data_deletion_e2e/02_data_deletion_persists_blocks_on_hold_and_verifies_every_real_surface.rs"
);
include!("data_deletion_e2e/03_deletion_cannot_complete_when_a_required_surface_is_missing.rs");
include!("data_deletion_e2e/04_exhausted_lease_recovery_remains_terminal_and_is_not_requeued.rs");
