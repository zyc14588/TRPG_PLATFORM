// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

fn schema_assertion_sql() -> String {
    include_str!("../../../scripts/ci/assert-schema.sql")
        .lines()
        .filter(|line| !line.trim_start().starts_with("\\set"))
        .collect::<Vec<_>>()
        .join("\n")
}

include!("migration_upgrade/01_module_prelude.rs");
include!("migration_upgrade/02_try_insert_historical_probe_event.rs");
include!("migration_upgrade/03_migration_upgrade_covers_empty_b24_repeat_drift_and_constraints.rs");
