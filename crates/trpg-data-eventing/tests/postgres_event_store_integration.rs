// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

mod support;

include!("postgres_event_store_integration/01_module_prelude.rs");
include!("postgres_event_store_integration/02_restore_and_hash.rs");
