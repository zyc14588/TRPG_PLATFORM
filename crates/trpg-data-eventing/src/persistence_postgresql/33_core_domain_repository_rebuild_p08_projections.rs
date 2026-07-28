
impl CoreDomainRepository {

    pub async fn rebuild_p08_projections(
        &self,
        campaign_id: &str,
    ) -> Result<P08ProjectionRebuildReport, CoreDomainRepositoryError> {
        EntityId::new(campaign_id)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("campaign_id"))?;
        // Loading and decrypting replay pages uses the canonical-store pool.
        // Do that before reserving a primary connection, then prove under the
        // campaign rebuild lock that the loaded P08 prefix is still current.
        // A writer that commits after this proof is forced to wait on the same
        // lock before projecting, so its projection cannot be lost.
        let (mut transaction, replay_events) = loop {
            let replay_events = self
                .load_campaign_events(campaign_id)
                .await?
                .into_iter()
                .filter(|event| {
                    matches!(
                        event.event_type.as_str(),
                        "CombatStateRecorded"
                            | "ChaseStateRecorded"
                            | "ReconsiderationRequested"
                            | "ReconsiderationReviewed"
                            | "ReconsiderationUpheld"
                            | "ReconsiderationCorrected"
                            | "CampaignForkRecorded"
                            | "CampaignForkMaterializationRecorded"
                            | "CampaignForkMaterialized"
                            | "EndingRecorded"
                            | "CharacterGrowthApplied"
                    )
                })
                .collect::<Vec<_>>();
            let mut transaction = self
                .primary
                .begin()
                .await
                .map_err(database_error("begin_p08_projection_rebuild"))?;
            self.lock_p08_projection_rebuild_scope(&mut transaction, campaign_id)
                .await?;
            let canonical_prefix: (i64, Option<i64>) = sqlx::query_as(
                r#"
                SELECT count(*), max(event.sequence)
                  FROM public.event_store AS event
                 WHERE event.campaign_id = $1
                   AND event.event_type IN (
                        'CombatStateRecorded',
                        'ChaseStateRecorded',
                        'ReconsiderationRequested',
                        'ReconsiderationReviewed',
                        'ReconsiderationUpheld',
                        'ReconsiderationCorrected',
                        'CampaignForkRecorded',
                        'CampaignForkMaterializationRecorded',
                        'CampaignForkMaterialized',
                        'EndingRecorded',
                        'CharacterGrowthApplied'
                   )
                   AND event.integrity_status = 'verified_hmac'
                   AND event.request_hash_source = 'formal_commit'
                   AND event.event_integrity_hash IS NOT NULL
                   AND event.payload_json ? 'protected_payload'
                   AND (
                        event.data_subject_id = 'not_applicable'
                        OR EXISTS (
                            SELECT 1
                              FROM public.privacy_subject_keys AS subject_key
                             WHERE subject_key.subject_id = event.data_subject_id
                               AND subject_key.key_reference =
                                   event.payload_key_reference
                               AND subject_key.wrapped_key IS NOT NULL
                               AND subject_key.destroyed_at IS NULL
                        )
                   )
                "#,
            )
            .bind(campaign_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error("verify_p08_replay_prefix"))?;
            let loaded_last_sequence = replay_events.last().map(|event| event.sequence);
            if canonical_prefix.0
                == i64::try_from(replay_events.len())
                    .map_err(|_| CoreDomainRepositoryError::Integrity("p08_replay_count"))?
                && canonical_prefix.1 == loaded_last_sequence
            {
                break (transaction, replay_events);
            }
            transaction
                .rollback()
                .await
                .map_err(database_error("retry_p08_projection_rebuild"))?;
        };
        let last_event_sequence = replay_events
            .last()
            .map(|event| event.sequence)
            .unwrap_or(0);
        let mut sheet_projected_character_ids = BTreeSet::<String>::new();
        for replay in &replay_events {
            let event: CoreDomainEvent = serde_json::from_value(replay.payload.clone())
                .map_err(|_| CoreDomainRepositoryError::Integrity("p08_sheet_replay_payload"))?;
            event.validate_schema_version()?;
            let (event_campaign_id, character_ids) = match event {
                CoreDomainEvent::CharacterGrowthApplied {
                    campaign_id,
                    character_id,
                    ..
                } => (campaign_id, vec![character_id]),
                CoreDomainEvent::CombatStateRecorded {
                    campaign_id,
                    character_health_updates,
                    ..
                } => (
                    campaign_id,
                    character_health_updates
                        .into_iter()
                        .map(|update| update.character_id)
                        .collect(),
                ),
                _ => continue,
            };
            if event_campaign_id != campaign_id {
                return Err(CoreDomainRepositoryError::Integrity(
                    "p08_sheet_replay_campaign",
                ));
            }
            sheet_projected_character_ids.extend(character_ids);
        }
        sqlx::query("SET CONSTRAINTS ALL DEFERRED")
            .execute(&mut *transaction)
            .await
            .map_err(database_error("defer_p08_rebuild_constraints"))?;

        // Growth-owned rows are rebuilt from the canonical events below. The
        // character itself is not rewound up front: replay advances a
        // projection that is behind, repairs one exactly at the Growth event,
        // and preserves a character already advanced by later canonical
        // gameplay. This keeps P08 repair from discarding P09-era mutations.
        if let Some(authorizing_sequence) = replay_events.last().map(|event| event.sequence) {
            let authorizing_commit_id: String = sqlx::query_scalar(
                r#"
                SELECT commit_id
                  FROM public.formal_commits
                 WHERE $1 BETWEEN first_event_sequence AND last_event_sequence
                   AND campaign_id = $2
                   AND status = 'committed'
                "#,
            )
            .bind(authorizing_sequence)
            .bind(campaign_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error("load_p08_rebuild_authorization"))?;
            self.set_projection_capability(
                &mut transaction,
                &authorizing_commit_id,
                "set_p08_rebuild_cleanup_capability",
            )
            .await?;
            sqlx::query("SELECT core_domain.clear_p08_rebuildable_projections($1, $2)")
                .bind(campaign_id)
                .bind(&authorizing_commit_id)
                .execute(&mut *transaction)
                .await
                .map_err(database_error("clear_p08_rebuildable_projections"))?;
            sqlx::query("SELECT core_domain.clear_combat_health_sheet_projections($1, $2)")
                .bind(campaign_id)
                .bind(authorizing_commit_id)
                .execute(&mut *transaction)
                .await
                .map_err(database_error("clear_combat_health_sheet_projections"))?;
        } else {
            let authorizing_commit_id: String = sqlx::query_scalar(
                r#"
                SELECT formal.commit_id
                  FROM public.event_store AS event
                  JOIN public.formal_commits AS formal
                    ON event.sequence BETWEEN
                       formal.first_event_sequence AND formal.last_event_sequence
                   AND formal.campaign_id = event.campaign_id
                   AND formal.status = 'committed'
                 WHERE event.campaign_id = $1
                   AND event.event_integrity_version = 3
                   AND event.integrity_status = 'verified_hmac'
                   AND event.request_hash_source = 'formal_commit'
                   AND event.event_integrity_hash IS NOT NULL
                   AND jsonb_array_length(event.projection_targets) > 0
                 ORDER BY event.sequence DESC
                 LIMIT 1
                "#,
            )
            .bind(campaign_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(database_error("load_empty_p08_rebuild_authorization"))?
            .ok_or(CoreDomainRepositoryError::NotFound("campaign_event"))?;
            self.set_projection_capability(
                &mut transaction,
                &authorizing_commit_id,
                "set_empty_p08_rebuild_cleanup_capability",
            )
            .await?;
            sqlx::query("SELECT core_domain.clear_empty_p08_rebuildable_projections($1, $2)")
                .bind(campaign_id)
                .bind(authorizing_commit_id)
                .execute(&mut *transaction)
                .await
                .map_err(database_error("clear_empty_p08_rebuildable_projections"))?;
        }
        for replay_event in &replay_events {
            let commit_id: String = sqlx::query_scalar(
                r#"
                SELECT commit_id
                  FROM public.formal_commits
                 WHERE $1 BETWEEN first_event_sequence AND last_event_sequence
                   AND campaign_id = $2
                   AND status = 'committed'
                "#,
            )
            .bind(replay_event.sequence)
            .bind(campaign_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error("load_p08_rebuild_commit"))?;
            self.set_projection_capability(
                &mut transaction,
                &commit_id,
                "set_p08_rebuild_projection_capability",
            )
            .await?;
            apply_p08_replay_event(&mut transaction, replay_event).await?;
        }
        for character_id in &sheet_projected_character_ids {
            let character_matches_canonical_tip: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1
                      FROM public.characters AS character
                      JOIN public.character_sheet_versions AS current_sheet
                        ON current_sheet.character_id =
                           character.character_id
                       AND current_sheet.version =
                           character.current_sheet_version
                      JOIN public.event_store AS current_event
                        ON current_event.sequence =
                           character.last_event_sequence
                       AND current_event.campaign_id =
                           character.campaign_id
                     WHERE character.character_id = $1
                       AND character.campaign_id = $2
                       AND current_event.integrity_status = 'verified_hmac'
                       AND current_event.request_hash_source = 'formal_commit'
                       AND current_event.event_integrity_hash IS NOT NULL
                       AND (
                            current_event.event_type =
                                'CombatStateRecorded'
                            OR current_event.visibility_label =
                               character.visibility_label::TEXT
                               AND current_event.visibility_subject =
                                   character.visibility_subject
                       )
                       AND current_event.fact_provenance_kind =
                           character.provenance_kind::TEXT
                       AND current_event.fact_provenance_reference =
                           character.provenance_reference
                       AND current_event.fact_recorded_by =
                           character.provenance_recorded_by
                       AND EXISTS (
                            SELECT 1
                              FROM jsonb_array_elements(
                                   current_event.projection_targets
                              ) AS projection_target
                             WHERE projection_target ->> 'relation' =
                                   'public.characters'
                               AND projection_target ->> 'row_id' = $1
                       )
                       AND character.last_event_sequence = (
                            SELECT max(event.sequence)
                              FROM public.event_store AS event
                             WHERE event.campaign_id = $2
                               AND event.integrity_status = 'verified_hmac'
                               AND event.request_hash_source =
                                   'formal_commit'
                               AND event.event_integrity_hash IS NOT NULL
                               AND EXISTS (
                                    SELECT 1
                                      FROM jsonb_array_elements(
                                           event.projection_targets
                                      ) AS projection_target
                                     WHERE projection_target ->> 'relation' =
                                           'public.characters'
                                       AND projection_target ->> 'row_id' = $1
                               )
                       )
                       AND character.version = (
                            SELECT count(*)
                              FROM public.event_store AS event
                             WHERE event.campaign_id = $2
                               AND event.sequence <=
                                   character.last_event_sequence
                               AND event.integrity_status = 'verified_hmac'
                               AND event.request_hash_source =
                                   'formal_commit'
                               AND event.event_integrity_hash IS NOT NULL
                               AND EXISTS (
                                    SELECT 1
                                      FROM jsonb_array_elements(
                                           event.projection_targets
                                      ) AS projection_target
                                     WHERE projection_target ->> 'relation' =
                                           'public.characters'
                                       AND projection_target ->> 'row_id' = $1
                               )
                       )
                )
                "#,
            )
            .bind(character_id)
            .bind(campaign_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error("verify_rebuilt_growth_character_tip"))?;
            if !character_matches_canonical_tip {
                return Err(CoreDomainRepositoryError::Integrity(
                    "growth_replay_character_tip_mismatch",
                ));
            }
        }
        let counts: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                (SELECT count(*) FROM public.combat_states WHERE campaign_id = $1),
                (SELECT count(*) FROM public.chase_states WHERE campaign_id = $1),
                (SELECT count(*) FROM public.reconsiderations WHERE campaign_id = $1),
                (SELECT count(*) FROM public.campaign_forks WHERE campaign_id = $1),
                (SELECT count(*) FROM public.campaign_fork_materializations
                  WHERE campaign_id = $1),
                (SELECT count(*) FROM public.campaign_fork_public_events
                  WHERE campaign_id = $1),
                (SELECT count(*) FROM public.campaign_fork_clues
                  WHERE campaign_id = $1),
                (SELECT count(*) FROM public.campaign_fork_npc_states
                  WHERE campaign_id = $1),
                (SELECT count(*) FROM public.gameplay_roll_consumptions
                  WHERE campaign_id = $1),
                (SELECT count(*) FROM public.ending_events WHERE campaign_id = $1),
                (SELECT count(*) FROM public.growth_events WHERE campaign_id = $1)
            "#,
        )
        .bind(campaign_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(database_error("count_p08_rebuilt_projections"))?;
        transaction
            .commit()
            .await
            .map_err(database_error("commit_p08_projection_rebuild"))?;
        Ok(P08ProjectionRebuildReport {
            campaign_id: campaign_id.to_owned(),
            replayed_events: replay_events.len(),
            combat_states: counts.0,
            chase_states: counts.1,
            reconsiderations: counts.2,
            campaign_forks: counts.3,
            fork_materializations: counts.4,
            fork_public_events: counts.5,
            fork_clues: counts.6,
            fork_npc_states: counts.7,
            gameplay_roll_consumptions: counts.8,
            ending_events: counts.9,
            growth_events: counts.10,
            last_event_sequence,
        })
    }
}
