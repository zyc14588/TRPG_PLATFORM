// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("session_scene_state_machine/01_module_prelude.rs");
include!("session_scene_state_machine/02_concurrent_start_transitions_and_restart_recovery_are_durable.rs");
