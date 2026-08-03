#[cfg(test)]
mod tests {
    use super::*;

    fn claimed(attempt_count: i16) -> ClaimedExport {
        ClaimedExport {
            export_id: "export_retry_contract".to_owned(),
            attempt_count,
        }
    }

    fn built(bytes: &[u8]) -> BuiltArtifact {
        BuiltArtifact {
            bytes: bytes.to_vec(),
            artifact_hash: artifact_sha256(bytes),
            manifest_hash: artifact_sha256(b"manifest"),
            first_event_sequence: 1,
            last_event_sequence: 1,
            event_count: 1,
            subjects: BTreeSet::new(),
            artifact_key: "artifacts/campaign_retry/export_retry_contract.json".to_owned(),
            retention_expires_at: Utc::now() + chrono::Duration::minutes(5),
        }
    }

    fn temporary_root(test_name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "trpg-campaign-export-{test_name}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn failed_export_attempts_retry_then_become_terminal() {
        assert_eq!(
            failed_claim_outcome(claimed(1), "CAMPAIGN_EXPORT_WRITE_FAILED"),
            CampaignExportOutcome::RetryScheduled {
                export_id: "export_retry_contract".to_owned(),
                error_code: "CAMPAIGN_EXPORT_WRITE_FAILED",
            }
        );
        assert_eq!(
            failed_claim_outcome(claimed(MAX_ATTEMPTS), "CAMPAIGN_EXPORT_WRITE_FAILED"),
            CampaignExportOutcome::TerminalFailure {
                export_id: "export_retry_contract".to_owned(),
                error_code: "CAMPAIGN_EXPORT_WRITE_FAILED",
            }
        );
    }

    #[test]
    fn persisted_artifact_is_idempotent_after_restart_and_hash_conflicts_fail_closed() {
        let root = temporary_root("restart");
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove stale campaign export test root");
        }
        fs::create_dir_all(&root).expect("create campaign export test root");
        let artifact = built(br#"{"manifest":{"export_id":"export_retry_contract"}}"#);
        let destination = checked_artifact_path(&root, &artifact.artifact_key)
            .expect("artifact destination is valid");

        persist_artifact_at(&root, "worker_before_crash", &claimed(1), &artifact)
            .expect("first worker persists the artifact before crashing");
        persist_artifact_at(&root, "worker_after_restart", &claimed(2), &artifact)
            .expect("restarted worker accepts the identical durable artifact");
        assert_eq!(
            fs::read(&destination).expect("read persisted artifact"),
            artifact.bytes
        );
        assert_eq!(
            fs::read_dir(destination.parent().expect("artifact parent"))
                .expect("read artifact directory")
                .count(),
            1,
            "restart must not create a duplicate artifact or temporary file"
        );

        fs::write(&destination, b"tampered").expect("inject artifact hash conflict");
        let error = persist_artifact_at(
            &root,
            "worker_after_tamper",
            &claimed(3),
            &artifact,
        )
        .expect_err("conflicting artifact bytes must fail closed");
        assert_eq!(error.code(), "CAMPAIGN_EXPORT_ARTIFACT_HASH_CONFLICT");
        assert_eq!(
            fs::read(&destination).expect("read conflicting artifact"),
            b"tampered"
        );

        fs::remove_dir_all(root).expect("remove campaign export test root");
    }
}
