// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

include!("combat_state_machine/01_module_prelude.rs");
include!("combat_state_machine/02_combat_weapon_loadout.rs");
include!("combat_state_machine/03_combat_state_start.rs");
include!("combat_state_machine/04_combat_state_apply_verified_recovery.rs");
include!("combat_state_machine/05_apply_damage_with_armor.rs");
