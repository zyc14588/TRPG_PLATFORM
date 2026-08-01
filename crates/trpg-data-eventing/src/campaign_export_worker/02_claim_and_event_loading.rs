impl CampaignExportWorker {
    pub fn new(
        pool: PgPool,
        canonical: PostgresCanonicalStore,
        root: impl Into<PathBuf>,
        worker_id: impl Into<String>,
        retention: Duration,
    ) -> Result<Self, CampaignExportWorkerError> {
        let root = root.into();
        let worker_id = worker_id.into();
        if !safe_id(&worker_id)
            || retention < Duration::from_secs(60)
            || retention > Duration::from_secs(366 * 24 * 60 * 60)
            || root.as_os_str().is_empty()
        {
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_CONFIGURATION_INVALID",
            ));
        }
        if fs::symlink_metadata(&root).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_ROOT_SYMLINK_FORBIDDEN",
            ));
        }
        fs::create_dir_all(&root)
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_ROOT_UNAVAILABLE"))?;
        Ok(Self {
            pool,
            canonical,
            root,
            worker_id,
            retention,
        })
    }

    pub async fn check_readiness(&self) -> Result<(), CampaignExportWorkerError> {
        let ready: bool =
            sqlx::query_scalar("SELECT to_regclass('public.campaign_export_jobs') IS NOT NULL")
                .fetch_one(&self.pool)
                .await
                .map_err(|_| {
                    CampaignExportWorkerError::new("CAMPAIGN_EXPORT_DATABASE_UNAVAILABLE")
                })?;
        if !ready {
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_SCHEMA_UNAVAILABLE",
            ));
        }
        let probe = self.root.join(".readiness");
        write_private_file(&probe, b"ready")?;
        fs::remove_file(probe)
            .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_ROOT_UNAVAILABLE"))
    }

    pub async fn run_once(
        &self,
        now_unix_ms: i64,
    ) -> Result<CampaignExportOutcome, CampaignExportWorkerError> {
        let now = utc_from_unix_ms(now_unix_ms)?;
        if let Some(expired) = self.expire_one(now).await? {
            return Ok(CampaignExportOutcome::Expired { export_id: expired });
        }
        let Some(claim) = self.claim(now).await? else {
            return Ok(CampaignExportOutcome::Idle);
        };
        match self.process_claim(&claim, now).await {
            Ok(()) => Ok(CampaignExportOutcome::Completed {
                export_id: claim.export_id,
            }),
            Err(error) => {
                self.mark_failed(&claim, error.code(), now).await?;
                Ok(failed_claim_outcome(claim, error.code()))
            }
        }
    }

    async fn claim(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Option<ClaimedExport>, CampaignExportWorkerError> {
        let row = sqlx::query(
            r#"
            WITH candidate AS (
                SELECT export_id
                  FROM public.campaign_export_jobs
                 WHERE attempt_count < $4
                   AND (
                        state IN ('REQUESTED', 'FAILED') AND next_attempt_at <= $1
                        OR state = 'RUNNING' AND lease_expires_at <= $1
                   )
                 ORDER BY created_at, export_id
                 FOR UPDATE SKIP LOCKED
                 LIMIT 1
            )
            UPDATE public.campaign_export_jobs AS job
               SET state = 'RUNNING',
                   attempt_count = job.attempt_count + 1,
                   lease_owner = $2,
                   lease_expires_at = $1 + make_interval(secs => $3),
                   failure_code = NULL,
                   updated_at = $1
              FROM candidate
             WHERE job.export_id = candidate.export_id
            RETURNING job.export_id, job.attempt_count
            "#,
        )
        .bind(now)
        .bind(&self.worker_id)
        .bind(CLAIM_LEASE_SECONDS)
        .bind(MAX_ATTEMPTS)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_CLAIM_FAILED"))?;
        Ok(row.map(|row| ClaimedExport {
            export_id: row.get("export_id"),
            attempt_count: row.get("attempt_count"),
        }))
    }

    async fn process_claim(
        &self,
        claim: &ClaimedExport,
        _now: DateTime<Utc>,
    ) -> Result<(), CampaignExportWorkerError> {
        let request = self.load_request(&claim.export_id).await?;
        let events = self
            .load_events(&request.campaign_id, request.requested_event_sequence)
            .await?;
        let groups = self.player_groups(&request).await?;
        let subjects = self.campaign_subjects(&request.campaign_id).await?;
        let built = self.build_artifact(&request, &events, &groups, subjects)?;
        self.persist_artifact(claim, &built)?;
        self.mark_ready(claim, built).await
    }

    async fn load_request(
        &self,
        export_id: &str,
    ) -> Result<ExportRequest, CampaignExportWorkerError> {
        let row = sqlx::query(
            r#"
            SELECT export.export_id, export.campaign_id, export.requested_by,
                   export.audience, export.requested_at,
                   job.requested_event_sequence,
                   authority.contract_id AS authority_contract_id,
                   authority.authority_mode, authority.authority_owner,
                   authority.contract_version, authority.ruleset_version,
                   authority.model_route_snapshot,
                   fork.fork_id, fork.parent_campaign_id, fork.source_session_id,
                   fork.source_snapshot_hash, fork.child_snapshot_hash
              FROM public.campaign_exports AS export
              JOIN public.campaign_export_jobs AS job
                ON job.export_id = export.export_id
              JOIN public.authority_contracts AS authority
                ON authority.campaign_id = export.campaign_id
               AND authority.locked
              LEFT JOIN public.campaign_forks AS fork
                ON fork.child_campaign_id = export.campaign_id
             WHERE export.export_id = $1
            "#,
        )
        .bind(export_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_REQUEST_LOAD_FAILED"))?
        .ok_or_else(|| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_REQUEST_MISSING"))?;
        Ok(ExportRequest {
            export_id: row.get("export_id"),
            campaign_id: row.get("campaign_id"),
            requested_by: row.get("requested_by"),
            audience: row.get("audience"),
            requested_at: row.get("requested_at"),
            requested_event_sequence: row.get("requested_event_sequence"),
            authority_contract_id: row.get("authority_contract_id"),
            authority_mode: row.get("authority_mode"),
            authority_owner: row.get("authority_owner"),
            authority_contract_version: row.get("contract_version"),
            ruleset_version: row.get("ruleset_version"),
            model_route_snapshot: row.get("model_route_snapshot"),
            fork_id: row.get("fork_id"),
            parent_campaign_id: row.get("parent_campaign_id"),
            source_session_id: row.get("source_session_id"),
            source_snapshot_hash: row.get("source_snapshot_hash"),
            child_snapshot_hash: row.get("child_snapshot_hash"),
        })
    }

    async fn load_events(
        &self,
        campaign_id: &str,
        through_sequence: i64,
    ) -> Result<Vec<CanonicalReplayEvent>, CampaignExportWorkerError> {
        let mut after = 0_i64;
        let mut events = Vec::new();
        loop {
            let page = self
                .canonical
                .load_replay_page(campaign_id, after, 500)
                .await
                .map_err(|_| {
                    CampaignExportWorkerError::new("CAMPAIGN_EXPORT_CANONICAL_REPLAY_FAILED")
                })?;
            if page.is_empty() {
                break;
            }
            let mut reached_boundary = false;
            for event in page {
                if event.sequence > through_sequence {
                    reached_boundary = true;
                    break;
                }
                after = event.sequence;
                events.push(event);
            }
            if reached_boundary
                || events
                    .last()
                    .is_some_and(|event| event.sequence >= through_sequence)
            {
                break;
            }
        }
        if events.is_empty() || events.last().map(|event| event.sequence) != Some(through_sequence)
        {
            return Err(CampaignExportWorkerError::new(
                "CAMPAIGN_EXPORT_EVENT_RANGE_INCOMPLETE",
            ));
        }
        Ok(events)
    }

    async fn player_groups(
        &self,
        request: &ExportRequest,
    ) -> Result<BTreeSet<String>, CampaignExportWorkerError> {
        if request.audience != "PLAYER" {
            return Ok(BTreeSet::new());
        }
        let rows = sqlx::query(
            r#"
            SELECT group_id
              FROM public.campaign_group_memberships
             WHERE campaign_id = $1 AND user_id = $2 AND revoked_at IS NULL
            "#,
        )
        .bind(&request.campaign_id)
        .bind(&request.requested_by)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| CampaignExportWorkerError::new("CAMPAIGN_EXPORT_GROUPS_LOAD_FAILED"))?;
        Ok(rows.into_iter().map(|row| row.get("group_id")).collect())
    }

    async fn campaign_subjects(
        &self,
        campaign_id: &str,
    ) -> Result<BTreeSet<String>, CampaignExportWorkerError> {
        let rows =
            sqlx::query("SELECT user_id FROM public.campaign_memberships WHERE campaign_id = $1")
                .bind(campaign_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|_| {
                    CampaignExportWorkerError::new("CAMPAIGN_EXPORT_SUBJECTS_LOAD_FAILED")
                })?;
        Ok(rows.into_iter().map(|row| row.get("user_id")).collect())
    }

}

fn failed_claim_outcome(
    claim: ClaimedExport,
    error_code: &'static str,
) -> CampaignExportOutcome {
    if claim.attempt_count >= MAX_ATTEMPTS {
        CampaignExportOutcome::TerminalFailure {
            export_id: claim.export_id,
            error_code,
        }
    } else {
        CampaignExportOutcome::RetryScheduled {
            export_id: claim.export_id,
            error_code,
        }
    }
}
