
impl PostgresProjectionWorker {
    pub fn new(
        pool: PgPool,
        projection_name: impl Into<String>,
        page_size: i64,
    ) -> Result<Self, EventWorkerError> {
        let projection_name = projection_name.into();
        if projection_name.trim().is_empty()
            || projection_name.len() > 128
            || !(1..=10_000).contains(&page_size)
        {
            return Err(EventWorkerError::Configuration(
                "invalid_projection_worker_configuration",
            ));
        }
        Ok(Self {
            pool,
            projection_name,
            page_size,
        })
    }

    pub async fn check_readiness(&self) -> Result<(), EventWorkerError> {
        let ready: bool = sqlx::query_scalar(
            "SELECT to_regclass('public.canonical_event_projection') IS NOT NULL AND to_regclass('public.projection_checkpoint') IS NOT NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("check_projection_readiness"))?;
        if ready {
            Ok(())
        } else {
            Err(EventWorkerError::Database("projection_schema_missing"))
        }
    }

    pub async fn checkpoint(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<ProjectionCheckpointState, EventWorkerError> {
        validate_stream_scope(campaign_id, stream_id)?;
        let checkpoint = sqlx::query_as::<_, ProjectionCheckpointState>(
            r#"
            SELECT projection_name, campaign_id, stream_id, version,
                   last_event_sequence, projection_hash, rebuilt_at
              FROM public.projection_checkpoint
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
            "#,
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("load_projection_checkpoint"))?;
        Ok(checkpoint.unwrap_or_else(|| {
            ProjectionCheckpointState::genesis(&self.projection_name, campaign_id, stream_id)
        }))
    }

    /// Delete only this rebuildable projection stream and its cursor while
    /// holding the same transaction-scoped lock used by page application.
    /// Canonical Event Store rows are deliberately outside this operation.
    pub async fn reset_stream_to_genesis(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<ProjectionCheckpointState, EventWorkerError> {
        validate_stream_scope(campaign_id, stream_id)?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| EventWorkerError::Database("begin_projection_reset"))?;
        self.lock_stream(&mut transaction, campaign_id, stream_id)
            .await?;
        self.delete_materialization(&mut transaction, campaign_id, stream_id)
            .await?;
        transaction
            .commit()
            .await
            .map_err(|_| EventWorkerError::Database("commit_projection_reset"))?;
        Ok(ProjectionCheckpointState::genesis(
            &self.projection_name,
            campaign_id,
            stream_id,
        ))
    }

    /// Force a complete reconstruction of one read-model stream from
    /// canonical Event Store history.
    pub async fn rebuild_from_genesis(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<ProjectionCheckpointState, EventWorkerError> {
        self.reset_stream_to_genesis(campaign_id, stream_id).await?;
        self.rebuild_to_tip(campaign_id, stream_id).await
    }

    /// Compute a bounded projection page without mutating either the read
    /// model or its durable cursor. `advance_checkpoint` applies this page and
    /// advances the checkpoint in one PostgreSQL transaction.
    pub async fn prepare_page(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<ProjectionPage, EventWorkerError> {
        let start = self.checkpoint(campaign_id, stream_id).await?;
        let rows = load_projection_events(
            &self.pool,
            campaign_id,
            stream_id,
            start.version,
            self.page_size,
        )
        .await?;
        let mut hasher = CanonicalProjectionHasher::resume(start.projection_hash.clone())?;
        let mut expected_version = start.version.saturating_add(1);
        for event in &rows {
            if event.stream_version != expected_version {
                return Err(EventWorkerError::ProjectionStreamGap {
                    expected: expected_version,
                    actual: event.stream_version,
                });
            }
            hasher.apply(event)?;
            expected_version = expected_version.saturating_add(1);
        }
        let target = if let Some(last) = rows.last() {
            ProjectionCheckpointState {
                projection_name: self.projection_name.clone(),
                campaign_id: campaign_id.to_owned(),
                stream_id: stream_id.to_owned(),
                version: last.stream_version,
                last_event_sequence: last.sequence,
                projection_hash: hasher.projection_hash().to_owned(),
                rebuilt_at: Utc::now(),
            }
        } else {
            start.clone()
        };
        Ok(ProjectionPage {
            start,
            target,
            events: rows,
        })
    }

    pub async fn advance_checkpoint(
        &self,
        page: &ProjectionPage,
    ) -> Result<CheckpointAdvance, EventWorkerError> {
        if page.events.is_empty() {
            return Ok(CheckpointAdvance::NoEvents(page.start.clone()));
        }
        let projection_hashes = validate_prepared_page(&self.projection_name, page)?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| EventWorkerError::Database("begin_checkpoint_transaction"))?;
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended($1 || ':' || $2 || ':' || $3, 0))",
        )
        .bind(&self.projection_name)
        .bind(&page.start.campaign_id)
        .bind(&page.start.stream_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| EventWorkerError::Database("lock_projection_checkpoint"))?;

        let current_row = sqlx::query_as::<_, ProjectionCheckpointState>(
            r#"
            SELECT projection_name, campaign_id, stream_id, version,
                   last_event_sequence, projection_hash, rebuilt_at
              FROM public.projection_checkpoint
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
             FOR UPDATE
            "#,
        )
        .bind(&self.projection_name)
        .bind(&page.start.campaign_id)
        .bind(&page.start.stream_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| EventWorkerError::Database("lock_current_checkpoint"))?;
        let checkpoint_exists = current_row.is_some();
        let current = current_row.unwrap_or_else(|| {
            ProjectionCheckpointState::genesis(
                &self.projection_name,
                &page.start.campaign_id,
                &page.start.stream_id,
            )
        });

        if current.version == page.target.version
            && current.last_event_sequence == page.target.last_event_sequence
            && current.projection_hash == page.target.projection_hash
        {
            let projected_rows: i64 = sqlx::query_scalar(
                r#"
                SELECT count(*)
                  FROM public.canonical_event_projection
                 WHERE projection_name = $1
                   AND campaign_id = $2
                   AND stream_id = $3
                   AND stream_version > $4
                   AND stream_version <= $5
                "#,
            )
            .bind(&self.projection_name)
            .bind(&page.start.campaign_id)
            .bind(&page.start.stream_id)
            .bind(page.start.version)
            .bind(page.target.version)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| EventWorkerError::Database("verify_applied_projection_page"))?;
            if projected_rows != page.events.len() as i64 {
                return Err(EventWorkerError::ProjectionReadModelConflict);
            }
            transaction
                .commit()
                .await
                .map_err(|_| EventWorkerError::Database("commit_checkpoint_noop"))?;
            return Ok(CheckpointAdvance::AlreadyApplied(current));
        }
        if current.version != page.start.version
            || current.last_event_sequence != page.start.last_event_sequence
            || current.projection_hash != page.start.projection_hash
        {
            return Err(EventWorkerError::CheckpointConflict {
                expected_version: page.start.version,
                actual_version: current.version,
            });
        }

        for (event, projection_hash) in page.events.iter().zip(projection_hashes) {
            let event_document = serde_json::to_value(event)
                .map_err(|_| EventWorkerError::ProjectionSerialization)?;
            sqlx::query(
                r#"
                INSERT INTO public.canonical_event_projection (
                    projection_name, campaign_id, stream_id, stream_version,
                    event_sequence, projection_hash, event_document
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(&self.projection_name)
            .bind(&event.campaign_id)
            .bind(&event.stream_id)
            .bind(event.stream_version)
            .bind(event.sequence)
            .bind(projection_hash)
            .bind(Json(event_document))
            .execute(&mut *transaction)
            .await
            .map_err(|_| EventWorkerError::Database("apply_projection_page"))?;
        }

        let advanced = if !checkpoint_exists {
            sqlx::query_as::<_, ProjectionCheckpointState>(
                r#"
                INSERT INTO public.projection_checkpoint (
                    projection_name, campaign_id, stream_id, version,
                    last_event_sequence, projection_hash, rebuilt_at
                ) VALUES ($1, $2, $3, $4, $5, $6, now())
                RETURNING projection_name, campaign_id, stream_id, version,
                          last_event_sequence, projection_hash, rebuilt_at
                "#,
            )
            .bind(&self.projection_name)
            .bind(&page.start.campaign_id)
            .bind(&page.start.stream_id)
            .bind(page.target.version)
            .bind(page.target.last_event_sequence)
            .bind(&page.target.projection_hash)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| EventWorkerError::Database("insert_projection_checkpoint"))?
        } else {
            sqlx::query_as::<_, ProjectionCheckpointState>(
                r#"
                UPDATE public.projection_checkpoint
                   SET version = $4,
                       last_event_sequence = $5,
                       projection_hash = $6,
                       rebuilt_at = now()
                 WHERE projection_name = $1
                   AND campaign_id = $2
                   AND stream_id = $3
                   AND version = $7
                   AND last_event_sequence = $8
                   AND projection_hash = $9
                RETURNING projection_name, campaign_id, stream_id, version,
                          last_event_sequence, projection_hash, rebuilt_at
                "#,
            )
            .bind(&self.projection_name)
            .bind(&page.start.campaign_id)
            .bind(&page.start.stream_id)
            .bind(page.target.version)
            .bind(page.target.last_event_sequence)
            .bind(&page.target.projection_hash)
            .bind(page.start.version)
            .bind(page.start.last_event_sequence)
            .bind(&page.start.projection_hash)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| EventWorkerError::Database("update_projection_checkpoint"))?
        };
        transaction
            .commit()
            .await
            .map_err(|_| EventWorkerError::Database("commit_projection_checkpoint"))?;
        Ok(CheckpointAdvance::Advanced(advanced))
    }

    pub async fn run_page(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<CheckpointAdvance, EventWorkerError> {
        let page = self.prepare_page(campaign_id, stream_id).await?;
        self.advance_checkpoint(&page).await
    }

    pub async fn rebuild_to_tip(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<ProjectionCheckpointState, EventWorkerError> {
        self.repair_materialization_if_inconsistent(campaign_id, stream_id)
            .await?;
        loop {
            match self.run_page(campaign_id, stream_id).await? {
                CheckpointAdvance::Advanced(_) | CheckpointAdvance::AlreadyApplied(_) => {}
                CheckpointAdvance::NoEvents(checkpoint) => {
                    // The empty-page path must not become a false pass when a
                    // read model was deleted while its checkpoint survived.
                    if self
                        .repair_materialization_if_inconsistent(campaign_id, stream_id)
                        .await?
                    {
                        continue;
                    }
                    return Ok(checkpoint);
                }
            }
        }
    }
}
