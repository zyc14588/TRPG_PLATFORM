// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("service/01_module_prelude.rs");
include!("service/02_read_http_request.rs");
#[cfg(test)]
include!("service/03_tests.rs");
