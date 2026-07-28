
impl CoreDomainRepository {

    async fn record_combat_state_inner(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordCombatStateRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        let next_version = metadata
            .expected_version
            .checked_add(1)
            .filter(|version| *version > 0)
            .ok_or(CoreDomainRepositoryError::InvalidInput("combat_version"))?;
        if request.state_json.is_empty() || request.state_json.len() > 1_048_576 {
            return Err(CoreDomainRepositoryError::InvalidInput("combat_state"));
        }
        let inspected = inspect_combat_state(&request.state_json)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_state"))?;
        validate_combat_server_roll_evidence(
            &request.state_json,
            request.attacker_roll.as_ref(),
            request.defender_roll.as_ref(),
            request.damage_roll.as_ref(),
            request.medical_roll.as_ref(),
        )
        .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_roll_evidence"))?;
        let roll_consumptions = combat_gameplay_roll_consumptions(&request.state_json)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_roll_evidence"))?;
        let combat_id = inspected.combat_id().to_owned();
        let status = inspected.status();
        let round = i64::from(inspected.round());
        let turn_index = i64::try_from(inspected.current_turn_index())
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_turn"))?;
        if i64::try_from(inspected.version()).ok() != Some(next_version) {
            return Err(CoreDomainRepositoryError::InvalidInput("combat_state"));
        }
        self.ensure_campaign_admin(&request.campaign_id, &metadata.requesting_actor_id)
            .await?;
        let mut transaction = self
            .begin_projection_transaction(&metadata.commit_id, "begin_combat_state")
            .await?;
        self.lock_p08_projection_rebuild_scope(&mut transaction, &request.campaign_id)
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("p08-combat:{}:{}", request.campaign_id, combat_id))
            .execute(&mut *transaction)
            .await
            .map_err(database_error("lock_combat_state"))?;
        let mut event = CoreDomainEvent::CombatStateRecorded {
            schema_version: CORE_EVENT_SCHEMA_VERSION,
            combat_id: combat_id.clone(),
            campaign_id: request.campaign_id.clone(),
            session_id: request.session_id.clone(),
            status: status.to_owned(),
            round: u64::try_from(round)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_round"))?,
            turn_index: u64::try_from(turn_index)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_turn"))?,
            version: u64::try_from(next_version)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_version"))?,
            state_json: request.state_json.clone(),
            character_health_updates: Vec::new(),
        };
        let canonical_retry = if let Some((sequence, existing_event)) = self
            .load_idempotent_core_event_record(
                &request.campaign_id,
                &combat_id,
                metadata,
                "CombatStateRecorded",
            )
            .await?
        {
            let mut request_shape = existing_event.clone();
            let CoreDomainEvent::CombatStateRecorded {
                character_health_updates,
                ..
            } = &mut request_shape
            else {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_combat_event_type",
                ));
            };
            character_health_updates.clear();
            if request_shape != event {
                return Err(CoreDomainRepositoryError::Integrity(
                    "idempotent_combat_request_conflict",
                ));
            }
            Some((
                existing_event,
                self.load_gameplay_retry_projection_targets(sequence)
                    .await?,
            ))
        } else {
            None
        };
        let existing = sqlx::query(
            r#"
            SELECT campaign_id, session_id, state_json, version,
                   last_event_sequence
              FROM public.combat_states
             WHERE combat_id = $1
            "#,
        )
        .bind(&combat_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(database_error("load_combat_state_transition"))?;
        let previous_state = if let Some(row) = existing {
            if row.get::<String, _>("campaign_id") != request.campaign_id
                || row.get::<String, _>("session_id") != request.session_id
            {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_state_identity_conflict",
                ));
            }
            let event_sequence: i64 = row.get("last_event_sequence");
            if self
                .projection_matches_command(event_sequence, metadata)
                .await?
            {
                let (existing_event, projection_targets) =
                    canonical_retry
                        .as_ref()
                        .ok_or(CoreDomainRepositoryError::Integrity(
                            "idempotent_combat_event_missing",
                        ))?;
                return self
                    .commit_gameplay_event(
                        metadata,
                        &request.campaign_id,
                        &combat_id,
                        ("combat_state", "combat.state.record"),
                        existing_event,
                        projection_targets.clone(),
                        "COMBAT",
                        &roll_consumptions,
                    )
                    .await;
            }
            if row.get::<i64, _>("version") != metadata.expected_version {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_state_projection_conflict",
                ));
            }
            Some(row.get::<Value, _>("state_json"))
        } else {
            None
        };
        if canonical_retry.is_none() {
            self.lock_active_gameplay_session(
                &mut transaction,
                &request.campaign_id,
                &request.session_id,
            )
            .await?;
        }
        if metadata.expected_version == 0 {
            self.validate_initial_combat_participants(
                &mut transaction,
                &request.campaign_id,
                &request.session_id,
                &combat_id,
                &request.state_json,
            )
            .await?;
        }
        if (metadata.expected_version == 0) != previous_state.is_none() {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_state_projection_conflict",
            ));
        }
        let previous_state_json = previous_state
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| CoreDomainRepositoryError::Serialization)?;
        let validated =
            validate_combat_state_transition(previous_state_json.as_deref(), &request.state_json)
                .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_transition"))?;
        if validated != inspected {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_state_validation_mismatch",
            ));
        }
        let next_state: Value = serde_json::from_str(&request.state_json)
            .map_err(|_| CoreDomainRepositoryError::InvalidInput("combat_state"))?;
        let changes = combat_health_changes(previous_state.as_ref(), &next_state)?;
        let canonical_updates = canonical_retry.as_ref().map(|(event, _)| {
            let CoreDomainEvent::CombatStateRecorded {
                character_health_updates,
                ..
            } = event
            else {
                unreachable!("canonical retry type was checked above");
            };
            character_health_updates.as_slice()
        });
        let health_projections = Box::pin(self.prepare_combat_health_projections(
            &mut transaction,
            &request.campaign_id,
            &combat_id,
            next_version,
            &changes,
            canonical_updates,
        ))
        .await?;
        let mut new_projection_targets = gameplay_state_projection_targets(
            "public.combat_states",
            &combat_id,
            !roll_consumptions.is_empty(),
        );
        for projection in &health_projections {
            new_projection_targets.push(projection_target(
                "public.character_sheet_versions",
                &projection.update.new_sheet_version_id,
            ));
            new_projection_targets.push(projection_target(
                "public.characters",
                &projection.update.character_id,
            ));
        }
        let projection_targets = if let Some((existing_event, existing_targets)) = canonical_retry {
            let expected = new_projection_targets
                .iter()
                .map(|target| (target.relation.as_str(), target.row_id.as_str()))
                .collect::<BTreeSet<_>>();
            let canonical = existing_targets
                .iter()
                .map(|target| (target.relation.as_str(), target.row_id.as_str()))
                .collect::<BTreeSet<_>>();
            if expected != canonical || expected.len() != new_projection_targets.len() {
                return Err(CoreDomainRepositoryError::Integrity(
                    "combat_health_projection_target_mismatch",
                ));
            }
            event = existing_event;
            existing_targets
        } else {
            let CoreDomainEvent::CombatStateRecorded {
                character_health_updates,
                ..
            } = &mut event
            else {
                unreachable!("new event is a CombatStateRecorded");
            };
            *character_health_updates = health_projections
                .iter()
                .map(|projection| projection.update.clone())
                .collect();
            new_projection_targets
        };
        self.lock_unconsumed_gameplay_rolls(&mut transaction, &roll_consumptions, metadata)
            .await?;
        let persisted = self
            .commit_gameplay_event(
                metadata,
                &request.campaign_id,
                &combat_id,
                ("combat_state", "combat.state.record"),
                &event,
                projection_targets,
                "COMBAT",
                &roll_consumptions,
            )
            .await?;
        project_gameplay_roll_consumptions(
            &mut transaction,
            &roll_consumptions,
            &request.campaign_id,
            "COMBAT",
            &combat_id,
            &metadata.visibility_label,
            &metadata.visibility_subject,
            &metadata.provenance_kind,
            &metadata.provenance_reference,
            &metadata.provenance_recorded_by,
            persisted.last_event_sequence,
        )
        .await?;
        Box::pin(Self::project_combat_health_projections(
            &mut transaction,
            &request.campaign_id,
            metadata,
            persisted.last_event_sequence,
            &health_projections,
        ))
        .await?;
        let result = sqlx::query(
            r#"
            INSERT INTO public.combat_states (
                combat_id, campaign_id, session_id, status, round,
                current_turn_index, state_json, version,
                visibility_label, visibility_subject,
                provenance_kind, provenance_reference, provenance_recorded_by,
                last_event_sequence
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7::JSONB, $8,
                $9, $10, $11, $12, $13, $14
            )
            ON CONFLICT (combat_id) DO UPDATE
               SET status = EXCLUDED.status,
                   round = EXCLUDED.round,
                   current_turn_index = EXCLUDED.current_turn_index,
                   state_json = EXCLUDED.state_json,
                   version = EXCLUDED.version,
                   visibility_label = EXCLUDED.visibility_label,
                   visibility_subject = EXCLUDED.visibility_subject,
                   provenance_kind = EXCLUDED.provenance_kind,
                   provenance_reference = EXCLUDED.provenance_reference,
                   provenance_recorded_by = EXCLUDED.provenance_recorded_by,
                   last_event_sequence = EXCLUDED.last_event_sequence
             WHERE combat_states.campaign_id = EXCLUDED.campaign_id
               AND combat_states.session_id = EXCLUDED.session_id
               AND combat_states.status = 'ONGOING'
               AND combat_states.version = $15
            "#,
        )
        .bind(&combat_id)
        .bind(&request.campaign_id)
        .bind(&request.session_id)
        .bind(status)
        .bind(round)
        .bind(turn_index)
        .bind(&request.state_json)
        .bind(next_version)
        .bind(&metadata.visibility_label)
        .bind(&metadata.visibility_subject)
        .bind(&metadata.provenance_kind)
        .bind(&metadata.provenance_reference)
        .bind(&metadata.provenance_recorded_by)
        .bind(persisted.last_event_sequence)
        .bind(metadata.expected_version)
        .execute(&mut *transaction)
        .await
        .map_err(database_error("project_combat_state"))?;
        if result.rows_affected() != 1 {
            return Err(CoreDomainRepositoryError::Integrity(
                "combat_state_projection_conflict",
            ));
        }
        transaction
            .commit()
            .await
            .map_err(database_error("commit_combat_state"))?;
        Ok(persisted)
    }

    pub async fn record_chase_state(
        &self,
        metadata: &CoreCommandMetadata,
        request: &RecordChaseStateRequest,
    ) -> Result<PersistedCommit, CoreDomainRepositoryError> {
        Box::pin(self.record_chase_state_inner(metadata, request)).await
    }
}
