// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

const NORMALIZED_PROMPT_MAP: &str =
    include_str!("../../../../docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md");

include!("lib_sections/01_module_prelude.rs");
include!("lib_sections/02_governed_command_for_contract.rs");
include!("lib_sections/03_spawn_test_policy_server.rs");
