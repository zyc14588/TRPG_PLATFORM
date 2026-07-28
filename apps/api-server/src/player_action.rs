//! Production P07 player-action adapter.
//!
//! The API layer supplies only authenticated intent. This adapter composes the
//! runtime confirmation workflow, the COC7 rules executor, and the canonical
//! PostgreSQL repository. No transport/model supplied dice value reaches the
//! executor.

// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("player_action/01_module_prelude.rs");
include!("player_action/02_player_action_tool_executor_execute.rs");
include!("player_action/03_player_action_command_port_submit_player_action.rs");
