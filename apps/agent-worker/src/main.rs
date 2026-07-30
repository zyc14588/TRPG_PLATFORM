// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("main_sections/01_module_prelude.rs");
include!("main_sections/02_agent_worker_process_from_environment.rs");
include!("main_sections/03_background_cycle_error.rs");
include!("main_sections/04_eventing_worker_rollout_flag_is_explicit_and_fail_closed.rs");
include!("main_sections/05_model_provider_construction.rs");
include!("main_sections/06_agent_job_construction.rs");
