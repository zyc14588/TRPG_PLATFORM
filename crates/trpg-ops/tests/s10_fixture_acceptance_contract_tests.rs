use trpg_ops::{
    verify_projection_rebuild, verify_restore_hash, BackupManifest, OpsRunbookError,
    RunbookExecutionRecord, S10_BACKUP_EVENT_HASH, S10_PROJECTION_HASH, S10_RESTORE_EVENT_HASH,
};

const S10_STAGE_FIXTURE: &str =
    include_str!("../../../fixtures/stages/S10_stage_acceptance_fixture.v1.json.md");
const S10_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S10_ops_migration_runbooks_expected.current.json.md"
);
const OPS_FIXTURE: &str =
    include_str!("../../../fixtures/ops/backup_restore_projection_rebuild.v1.json.md");

#[test]
fn s10_fixtures_are_bound_to_ops_runbook_contracts() {
    assert!(S10_STAGE_FIXTURE.contains("\"stage\": \"S10\""));
    assert!(S10_STAGE_FIXTURE.contains("\"stage_dir\": \"s10-ops-migration-runbooks\""));
    assert!(S10_DETAILED_FIXTURE.contains("\"BackupManifest\""));
    assert!(S10_DETAILED_FIXTURE.contains("\"RunbookExecutionRecord\""));
    assert!(OPS_FIXTURE.contains(S10_BACKUP_EVENT_HASH));
    assert!(OPS_FIXTURE.contains(S10_PROJECTION_HASH));

    for criterion in [
        "migrations_idempotent",
        "backup_restore_hash_verified",
        "projection_rebuild_deterministic",
        "rollback_runbook_exists",
    ] {
        assert!(S10_DETAILED_FIXTURE.contains(criterion));
    }
}

#[test]
fn backup_restore_and_projection_checks_are_executable() {
    let manifest = BackupManifest::fixture();
    assert!(manifest.has_required_fields());
    assert_eq!(manifest.sha256, S10_BACKUP_EVENT_HASH);

    verify_restore_hash(S10_BACKUP_EVENT_HASH, S10_RESTORE_EVENT_HASH)
        .expect("fixture restore hash matches");
    let restore_error = verify_restore_hash(S10_BACKUP_EVENT_HASH, "sha256:wrong")
        .expect_err("hash mismatch is rejected");
    assert_eq!(restore_error.code(), "RESTORE_HASH_MISMATCH");

    let report = verify_projection_rebuild(3, 3, S10_PROJECTION_HASH, S10_PROJECTION_HASH)
        .expect("projection rebuild is deterministic");
    assert_eq!(report.new_canon_events, 0);

    let projection_error = verify_projection_rebuild(3, 4, "sha256:changed", S10_PROJECTION_HASH)
        .expect_err("projection rebuild must not append canon events");
    assert_eq!(
        projection_error,
        OpsRunbookError::ProjectionRebuildHashMismatch
    );

    let execution = RunbookExecutionRecord::succeeded(
        "pwsh ./scripts/ops/backup-restore-smoke.ps1",
        "evidence/stages/S10/backup-restore.txt",
    );
    assert!(execution.has_required_fields());
    assert_eq!(execution.exit_code, 0);
}
