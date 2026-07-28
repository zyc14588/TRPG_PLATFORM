// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

const TUTORIAL: &str =
    include_str!("../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml");

include!("tutorial_complete_e2e/01_module_prelude.rs");
include!("tutorial_complete_e2e/02_create_campaign.rs");
include!("tutorial_complete_e2e/03_tutorial_runs_through_real_repository_event_store_outbox_and_witness.rs");
include!("tutorial_complete_e2e/04_tutorial_rejects_early_ending_and_private_fork_scope.rs");
