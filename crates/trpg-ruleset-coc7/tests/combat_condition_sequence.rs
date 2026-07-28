// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("combat_condition_sequence/01_module_prelude.rs");
include!("combat_condition_sequence/02_armor_and_multi_character_turns_are_persistent_aggregate_state.rs");
include!("combat_condition_sequence/03_defenses_medical_targets_and_roll_ids_are_derived_and_single_use.rs");
