// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("agent_runtime/01_module_prelude.rs");
include!("agent_runtime/02_agent_decision.rs");
include!("agent_runtime/03_agent_decision_committer_new.rs");
include!("agent_runtime/04_persist_agent_formal_batch.rs");
