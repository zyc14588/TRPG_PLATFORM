impl CampaignExportWorker {
    fn build_artifact(
        &self,
        request: &ExportRequest,
        events: &[CanonicalReplayEvent],
        groups: &BTreeSet<String>,
        mut subjects: BTreeSet<String>,
    ) -> Result<BuiltArtifact, CampaignExportWorkerError> {
        subjects.insert(request.requested_by.clone());
        let first_event_sequence = events[0].sequence;
        let last_event_sequence = events[events.len() - 1].sequence;
        let event_count = i64::try_from(events.len())
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_EVENT_COUNT_OVERFLOW"))?;
        let view = normalized_view(&request.audience)?;
        let records = events
            .iter()
            .filter_map(|event| export_record(view, request, groups, event))
            .collect::<Vec<_>>();
        let sections = export_sections(view, &records, request);
        let content = json!({
            "records": records,
            "sections": sections,
        });
        let content_bytes = serde_json::to_vec(&content)
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_SERIALIZATION_FAILED"))?;
        let manifest_hash = sha256_prefixed(&content_bytes);
        let retention_expires_at = request.requested_at
            + chrono::Duration::from_std(self.retention)
                .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_RETENTION_INVALID"))?;
        let artifact = json!({
            "content": content,
            "manifest": {
                "artifact_schema": CAMPAIGN_EXPORT_SCHEMA,
                "authority": {
                    "contract_id": request.authority_contract_id,
                    "contract_version": request.authority_contract_version,
                    "mode": request.authority_mode,
                    "owner": request.authority_owner,
                    "ruleset_version": request.ruleset_version,
                },
                "campaign_id": request.campaign_id,
                "event_range": {
                    "count": event_count,
                    "first_sequence": first_event_sequence,
                    "last_sequence": last_event_sequence,
                },
                "export_id": request.export_id,
                "fork_provenance": {
                    "child_snapshot_hash": request.child_snapshot_hash,
                    "fork_id": request.fork_id,
                    "parent_campaign_id": request.parent_campaign_id,
                    "source_session_id": request.source_session_id,
                    "source_snapshot_hash": request.source_snapshot_hash,
                },
                "generated_from_request_at": request.requested_at.to_rfc3339(),
                "hash_algorithm": "sha256",
                "manifest_hash": manifest_hash,
                "manifest_hash_scope": "canonical-json:content",
                "requested_by": request.requested_by,
                "retention_expires_at": retention_expires_at.to_rfc3339(),
                "view": view,
                "visibility_policy_version": VISIBILITY_POLICY_VERSION,
            },
        });
        let bytes = serde_json::to_vec(&artifact)
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_SERIALIZATION_FAILED"))?;
        let artifact_hash = sha256_prefixed(&bytes);
        let artifact_key = format!(
            "artifacts/{}/{}.json",
            request.campaign_id, request.export_id
        );
        if !safe_relative_key(&artifact_key) {
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_ARTIFACT_KEY_INVALID",
            ));
        }
        Ok(BuiltArtifact {
            bytes,
            artifact_hash,
            manifest_hash,
            first_event_sequence,
            last_event_sequence,
            event_count,
            subjects,
            artifact_key,
            retention_expires_at,
        })
    }

    fn persist_artifact(
        &self,
        claim: &ClaimedExport,
        built: &BuiltArtifact,
    ) -> Result<(), CampaignExportWorkerError> {
        persist_artifact_at(&self.root, &self.worker_id, claim, built)
    }

    async fn mark_ready(
        &self,
        claim: &ClaimedExport,
        built: BuiltArtifact,
    ) -> Result<(), CampaignExportWorkerError> {
        let artifact_size = i64::try_from(built.bytes.len()).map_err(|_| {
            CampaignExportWorkerError::new("CAMPAIGN_EXPORT_ARTIFACT_SIZE_OVERFLOW")
        })?;
        let mut transaction =
            self.pool.begin().await.map_err(|_| {
                CampaignExportWorkerError::new("CAMPAIGN_EXPORT_READY_BEGIN_FAILED")
            })?;
        sqlx::query("DELETE FROM public.campaign_export_subjects WHERE export_id = $1")
            .bind(&claim.export_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_SUBJECT_RESET_FAILED"))?;
        for subject in built.subjects {
            sqlx::query(
                "INSERT INTO public.campaign_export_subjects (export_id, subject_id) \
                 VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(&claim.export_id)
            .bind(subject)
            .execute(&mut *transaction)
            .await
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_SUBJECT_BIND_FAILED"))?;
        }
        let updated = sqlx::query(
            r#"
            UPDATE public.campaign_export_jobs
               SET state = 'READY', lease_owner = NULL, lease_expires_at = NULL,
                   first_event_sequence = $3, last_event_sequence = $4,
                   event_count = $5,
                   visibility_policy_version = $6, artifact_schema = $7,
                   artifact_key = $8, artifact_hash = $9, manifest_hash = $10,
                   artifact_size = $11, ready_at = now(),
                   retention_expires_at = $12, failure_code = NULL,
                   updated_at = now()
             WHERE export_id = $1 AND state = 'RUNNING' AND lease_owner = $2
            "#,
        )
        .bind(&claim.export_id)
        .bind(&self.worker_id)
        .bind(built.first_event_sequence)
        .bind(built.last_event_sequence)
        .bind(built.event_count)
        .bind(VISIBILITY_POLICY_VERSION)
        .bind(CAMPAIGN_EXPORT_SCHEMA)
        .bind(built.artifact_key)
        .bind(built.artifact_hash)
        .bind(built.manifest_hash)
        .bind(artifact_size)
        .bind(built.retention_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_READY_UPDATE_FAILED"))?;
        if updated.rows_affected() != 1 {
            return Err(CampaignExportWorkerError::new("CAMPAIGN_EXPORT_LEASE_LOST"));
        }
        transaction
            .commit()
            .await
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_READY_COMMIT_FAILED"))
    }

    async fn mark_failed(
        &self,
        claim: &ClaimedExport,
        error_code: &'static str,
        now: DateTime<Utc>,
    ) -> Result<(), CampaignExportWorkerError> {
        let retry_seconds = i64::from(claim.attempt_count).saturating_mul(2).max(1);
        let updated = sqlx::query(
            r#"
            UPDATE public.campaign_export_jobs
               SET state = 'FAILED', lease_owner = NULL, lease_expires_at = NULL,
                   failure_code = $3,
                   next_attempt_at = $4 + make_interval(secs => $5),
                   updated_at = $4
             WHERE export_id = $1 AND state = 'RUNNING' AND lease_owner = $2
            "#,
        )
        .bind(&claim.export_id)
        .bind(&self.worker_id)
        .bind(error_code)
        .bind(now)
        .bind(retry_seconds)
        .execute(&self.pool)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_FAILURE_UPDATE_FAILED"))?;
        if updated.rows_affected() != 1 {
            return Err(CampaignExportWorkerError::new("CAMPAIGN_EXPORT_LEASE_LOST"));
        }
        Ok(())
    }
}

fn persist_artifact_at(
    root: &Path,
    worker_id: &str,
    claim: &ClaimedExport,
    built: &BuiltArtifact,
) -> Result<(), CampaignExportWorkerError> {
        let destination = checked_artifact_path(root, &built.artifact_key)?;
        let parent = destination.parent().ok_or_else(|| {
            CampaignExportWorkerError::new("CAMPAIGN_EXPORT_ARTIFACT_KEY_INVALID")
        })?;
        fs::create_dir_all(parent)
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_WRITE_FAILED"))?;
        if destination.exists() {
            let existing = fs::read(&destination)
                .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_READ_FAILED"))?;
            if sha256_prefixed(&existing) == built.artifact_hash {
                return Ok(());
            }
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_ARTIFACT_HASH_CONFLICT",
            ));
        }
        let temporary = destination.with_extension(format!(
            "json.tmp.{}.{}",
            worker_id, claim.attempt_count
        ));
        write_private_file(&temporary, &built.bytes)?;
        fs::rename(&temporary, &destination).map_err(|_| {
            let _ = fs::remove_file(&temporary);
            CampaignExportWorkerError::new("CAMPAIGN_EXPORT_ATOMIC_RENAME_FAILED")
        })
}
