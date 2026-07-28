
#[tokio::test]
async fn migration_upgrade_covers_empty_b24_repeat_drift_and_constraints() {
    // Ordered phases preserve shared integration fixtures in lexical scope.
    include!("03_migration_upgrade_phases/01_schema_baseline_and_drift_guards.rs");
}
