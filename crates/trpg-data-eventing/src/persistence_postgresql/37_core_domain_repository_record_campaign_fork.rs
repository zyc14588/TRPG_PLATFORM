
impl CoreDomainRepository {
    pub async fn record_campaign_fork(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordCampaignForkRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let (persisted, replay_events) = self
            .commit_campaign_fork_events(metadata, request, None)
            .await?;
        self.project_campaign_fork_replay(metadata, request, &replay_events)
            .await?;
        Ok(persisted)
    }

    async fn commit_campaign_fork_events(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordCampaignForkRequest,
        pending_child_room_id: Option<&str>,
    ) -> Result<(PersistedCommit, Vec<CanonicalReplayEvent>), CoreDomainRepositoryError> {
        if metadata.expected_version != 0
            || request.parent_campaign_id == request.child_campaign_id
            || request.reason.trim().is_empty()
            || request.reason.len() > 512
            || !valid_sha256(&request.snapshot_hash)
            || request.copy_scopes != DEFAULT_PUBLIC_COPY_SCOPES
            || metadata.visibility_label != "keeper_only"
            || metadata.visibility_subject != "not_applicable"
        {
            return Err(CoreDomainRepositoryError::InvalidInput("campaign_fork"));
        }
        self.ensure_campaign_admin(&request.parent_campaign_id, &metadata.requesting_actor_id)
            .await?;
        if pending_child_room_id.is_none() {
            self.ensure_campaign_admin(&request.child_campaign_id, &metadata.requesting_actor_id)
                .await?;
            self.validate_campaign_fork_authority(request).await?;
        }

        let child_campaign_events = self
            .load_campaign_events(&request.child_campaign_id)
            .await?;
        let mut canonical_lineage = None;
        let mut canonical_lineage_sequence = None;
        for replay in child_campaign_events
            .iter()
            .filter(|event| event.event_type == "CampaignForkRecorded")
        {
            let event: CoreDomainEvent =
                serde_json::from_value(replay.payload.clone()).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("campaign_fork_lineage_payload")
                })?;
            event.validate_schema_version()?;
            let CoreDomainEvent::CampaignForkRecorded {
                fork_id,
                parent_campaign_id,
                child_campaign_id,
                source_session_id,
                snapshot_hash,
                copy_scopes,
                reason,
                ..
            } = event
            else {
                return Err(CoreDomainRepositoryError::Integrity(
                    "campaign_fork_lineage_event_type",
                ));
            };
            if replay.resource_id != fork_id
                || replay.campaign_id != child_campaign_id
                || child_campaign_id != request.child_campaign_id
                || canonical_lineage.is_some()
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "campaign_fork_multiple_child_lineages",
                ));
            }
            canonical_lineage = Some((
                fork_id,
                parent_campaign_id,
                child_campaign_id,
                source_session_id,
                snapshot_hash,
                copy_scopes,
                reason,
            ));
            canonical_lineage_sequence = Some(replay.sequence);
        }
        let retrying_canonical = if let Some(existing) = canonical_lineage {
            if existing
                != (
                    request.fork_id.clone(),
                    request.parent_campaign_id.clone(),
                    request.child_campaign_id.clone(),
                    request.source_session_id.clone(),
                    request.snapshot_hash.clone(),
                    request.copy_scopes.clone(),
                    request.reason.clone(),
                )
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "campaign_fork_child_lineage_conflict",
                ));
            }
            true
        } else {
            false
        };
        let include_child_lineage_marker =
            if let Some(lineage_sequence) = canonical_lineage_sequence {
                sqlx::query_scalar(
                    r#"
                    SELECT EXISTS(
                        SELECT 1
                          FROM jsonb_array_elements(projection_targets) AS target
                         WHERE target ->> 'relation' = $2
                           AND target ->> 'row_id' = $3
                    )
                      FROM public.event_store
                     WHERE sequence = $1
                       AND event_type = 'CampaignForkRecorded'
                       AND integrity_status = 'verified_hmac'
                       AND request_hash_source = 'formal_commit'
                    "#,
                )
                .bind(lineage_sequence)
                .bind(FORK_CHILD_LINEAGE_MARKER_RELATION)
                .bind(&request.fork_id)
                .fetch_one(&self.primary)
                .await
                .map_err(database_error("load_campaign_fork_lineage_marker"))?
            } else {
                true
            };

        let existing_fork: Option<(String, String, String, String, String)> = sqlx::query_as(
            r#"
            SELECT fork_id, parent_campaign_id, child_campaign_id,
                   source_session_id, source_snapshot_hash
              FROM public.campaign_forks
             WHERE fork_id = $1 OR child_campaign_id = $2
            "#,
        )
        .bind(&request.fork_id)
        .bind(&request.child_campaign_id)
        .fetch_optional(&self.primary)
        .await
        .map_err(database_error("load_existing_campaign_fork_identity"))?;
        let retrying_projection = if let Some(existing) = existing_fork {
            if existing
                != (
                    request.fork_id.clone(),
                    request.parent_campaign_id.clone(),
                    request.child_campaign_id.clone(),
                    request.source_session_id.clone(),
                    request.snapshot_hash.clone(),
                )
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "campaign_fork_identity_conflict",
                ));
            }
            true
        } else {
            false
        };
        let materialization = if retrying_canonical {
            campaign_fork_materialization_from_replay(&child_campaign_events, request)?
        } else {
            let references_exist: bool = if pending_child_room_id.is_some() {
                sqlx::query_scalar(
                    r#"
                    SELECT EXISTS(
                        SELECT 1
                          FROM core_domain.sessions AS source_session
                         WHERE source_session.session_id = $1
                           AND source_session.campaign_id = $2
                    )
                    "#,
                )
                .bind(&request.source_session_id)
                .bind(&request.parent_campaign_id)
                .fetch_one(&self.primary)
                .await
            } else {
                sqlx::query_scalar(
                    r#"
                    SELECT EXISTS(
                        SELECT 1
                          FROM core_domain.sessions AS source_session
                          JOIN public.campaigns AS child
                            ON child.campaign_id = $1
                         WHERE source_session.session_id = $2
                           AND source_session.campaign_id = $3
                    )
                    "#,
                )
                .bind(&request.child_campaign_id)
                .bind(&request.source_session_id)
                .bind(&request.parent_campaign_id)
                .fetch_one(&self.primary)
                .await
            }
            .map_err(database_error("load_campaign_fork_references"))?;
            if !references_exist {
                return Err(CoreDomainRepositoryError::NotFound(
                    "fork_campaign_or_session",
                ));
            }
            if pending_child_room_id.is_none() && !retrying_projection {
                let child_has_gameplay_state: bool = sqlx::query_scalar(
                    r#"
                    SELECT EXISTS(
                        SELECT 1 FROM public.scenarios WHERE campaign_id = $1
                        UNION ALL
                        SELECT 1 FROM public.characters WHERE campaign_id = $1
                        UNION ALL
                        SELECT 1 FROM core_domain.sessions WHERE campaign_id = $1
                        UNION ALL
                        SELECT 1 FROM public.campaign_forks WHERE child_campaign_id = $1
                        UNION ALL
                        SELECT 1 FROM public.campaign_fork_public_events WHERE campaign_id = $1
                        UNION ALL
                        SELECT 1 FROM public.campaign_fork_clues WHERE campaign_id = $1
                        UNION ALL
                        SELECT 1 FROM public.campaign_fork_npc_states WHERE campaign_id = $1
                        UNION ALL
                        SELECT 1 FROM public.combat_states WHERE campaign_id = $1
                        UNION ALL
                        SELECT 1 FROM public.chase_states WHERE campaign_id = $1
                        UNION ALL
                        SELECT 1 FROM public.ending_events WHERE campaign_id = $1
                    )
                    "#,
                )
                .bind(&request.child_campaign_id)
                .fetch_one(&self.primary)
                .await
                .map_err(database_error("check_fork_child_empty"))?;
                if child_has_gameplay_state {
                    return Err(CoreDomainRepositoryError::Integrity("fork_child_not_empty"));
                }
            }
            let snapshot = self
                .load_public_campaign_fork_snapshot(
                    &request.parent_campaign_id,
                    &request.source_session_id,
                )
                .await?;
            if snapshot.snapshot_hash != request.snapshot_hash {
                return Err(CoreDomainRepositoryError::Integrity(
                    "campaign_fork_snapshot_hash_mismatch",
                ));
            }
            self.build_campaign_fork_materialization(
                request,
                &snapshot,
                pending_child_room_id,
            )
            .await?
        };
        let batch_count = u64::try_from(materialization.batches.len())
            .map_err(|_| CoreDomainRepositoryError::Integrity("fork_batch_count"))?;
        let materialized_row_count = u64::try_from(materialization.rows.len())
            .map_err(|_| CoreDomainRepositoryError::Integrity("fork_row_count"))?;
        if materialization.batches.len() + 2 > 256 {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_event_batch_limit",
            ));
        }
        let snapshot_reference_json = fork_snapshot_reference_json(&request.snapshot_hash)?;
        let recorded = CoreDomainEvent::CampaignForkRecorded {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            fork_id: request.fork_id.clone(),
            parent_campaign_id: request.parent_campaign_id.clone(),
            child_campaign_id: request.child_campaign_id.clone(),
            source_session_id: request.source_session_id.clone(),
            snapshot_hash: request.snapshot_hash.clone(),
            child_snapshot_hash: materialization.child_snapshot_hash.clone(),
            copy_scopes: request.copy_scopes.clone(),
            canonical_snapshot_json: snapshot_reference_json,
            reason: request.reason.clone(),
        };
        let manifest = CoreDomainEvent::CampaignForkMaterializationRecorded {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            fork_id: request.fork_id.clone(),
            child_campaign_id: request.child_campaign_id.clone(),
            child_session_id: materialization.child_session_id.clone(),
            child_scenario_id: materialization.child_scenario_id.clone(),
            child_snapshot_hash: materialization.child_snapshot_hash.clone(),
            child_state_json: materialization.child_state_json.clone(),
            materialized_row_count,
            batch_count,
        };
        let mut events = vec![
            (
                recorded,
                campaign_fork_recorded_projection_targets(
                    &request.fork_id,
                    include_child_lineage_marker,
                ),
            ),
            (
                manifest,
                vec![projection_target(
                    "public.campaign_fork_materializations",
                    &request.fork_id,
                )],
            ),
        ];
        for (index, batch) in materialization.batches.iter().enumerate() {
            let projection_targets = batch
                .rows
                .iter()
                .flat_map(fork_row_projection_targets)
                .collect::<Vec<_>>();
            if projection_targets.len() > 32 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "fork_projection_target_limit",
                ));
            }
            events.push((
                CoreDomainEvent::CampaignForkMaterialized {
                    schema_version: CORE_EVENT_SCHEMA_VERSION,
                    fork_id: request.fork_id.clone(),
                    child_campaign_id: request.child_campaign_id.clone(),
                    batch_index: u64::try_from(index + 1)
                        .map_err(|_| CoreDomainRepositoryError::Integrity("fork_batch_index"))?,
                    batch_count,
                    rows: batch.rows.clone(),
                },
                projection_targets,
            ));
        }
        let mut draft = metadata.to_multi_event_draft(
            &request.child_campaign_id,
            &request.fork_id,
            "campaign_fork",
            "campaign.fork.record",
            events,
        )?;
        for (event, batch) in draft
            .events
            .iter_mut()
            .skip(2)
            .zip(&materialization.batches)
        {
            event.visibility = Some(CanonicalEventVisibility {
                label: batch.visibility_label.clone(),
                subject: batch.visibility_subject.clone(),
                data_subject_id: batch.data_subject_id.clone(),
            });
        }
        let persisted = match self.canonical.commit(&draft).await {
            Ok(persisted) => persisted,
            Err(error) => {
                // A concurrent candidate may have won the canonical partial
                // unique index after this request's preflight read. Resolve
                // that database conflict to the stable domain error without
                // keeping a pool connection reserved across the commit.
                if self
                    .load_campaign_events(&request.child_campaign_id)
                    .await?
                    .iter()
                    .any(|event| event.event_type == "CampaignForkRecorded")
                {
                    return Err(CoreDomainRepositoryError::Integrity(
                        "campaign_fork_child_lineage_conflict",
                    ));
                }
                return Err(CoreDomainRepositoryError::Canonical(error));
            }
        };
        let replay_events = self
            .load_campaign_events(&request.child_campaign_id)
            .await?
            .into_iter()
            .filter(|event| {
                event.sequence >= persisted.first_event_sequence
                    && event.sequence <= persisted.last_event_sequence
                    && event.stream_id == request.fork_id
            })
            .collect::<Vec<_>>();
        if replay_events.len() != materialization.batches.len() + 2 {
            return Err(CoreDomainRepositoryError::Integrity(
                "campaign_fork_event_batch_mismatch",
            ));
        }
        Ok((persisted, replay_events))
    }
}
