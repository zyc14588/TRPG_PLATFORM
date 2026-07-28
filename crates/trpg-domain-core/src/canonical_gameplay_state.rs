//! Independent validation for serialized formal combat and chase states.
//!
//! The COC7 ruleset produces these snapshots. Domain persistence validates the
//! complete shape and exact predecessor transition again before an event can
//! become canonical, without creating an outward dependency from adapters to
//! the concrete ruleset crate.

// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("canonical_gameplay_state/01_module_prelude.rs");
include!("canonical_gameplay_state/02_validate_combat_server_roll_evidence.rs");
include!("canonical_gameplay_state/03_apply_combat_mutation.rs");
include!("canonical_gameplay_state/04_canonical_success_level.rs");
include!("canonical_gameplay_state/05_apply_chase_mutation.rs");
include!("canonical_gameplay_state/06_weapon_loadout.rs");
