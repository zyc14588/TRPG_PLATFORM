// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("auth_http_integration/01_module_prelude.rs");
include!("auth_http_integration/02_refresh_rotates_session_and_logout_revokes_it.rs");
