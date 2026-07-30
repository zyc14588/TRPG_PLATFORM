// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

pub mod core_domain;
pub mod middleware;
pub mod player_action;

#[cfg(test)]
mod production_custody_tests;

include!("lib_sections/01_module_prelude.rs");
include!("lib_sections/02_api_application_new.rs");
include!("lib_sections/03_api_application_authorized_player_action_context.rs");
include!("lib_sections/04_api_application_request_deletion.rs");
include!("lib_sections/05_api_application_get_authority.rs");
include!("lib_sections/06_deletion_status_response.rs");
include!("lib_sections/08_agent_job_gateway.rs");
