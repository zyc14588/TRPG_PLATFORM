// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("production_custody_tests/01_module_prelude.rs");
include!(
    "production_custody_tests/02_production_runtime_without_a_bound_tool_executor_fails_closed.rs"
);
