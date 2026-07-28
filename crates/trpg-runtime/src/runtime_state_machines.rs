// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("runtime_state_machines/01_module_prelude.rs");
include!("runtime_state_machines/02_runtime_decision_new.rs");
include!("runtime_state_machines/03_confirm_pending_decision.rs");
include!("runtime_state_machines/04_commit_decision.rs");
include!("runtime_state_machines/05_internal_generic_append_rejects_formal_decision_payloads.rs");
