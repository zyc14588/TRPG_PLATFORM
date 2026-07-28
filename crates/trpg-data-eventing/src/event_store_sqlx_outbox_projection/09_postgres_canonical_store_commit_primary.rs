
impl PostgresCanonicalStore {

    async fn commit_primary(
        &self,
        draft: &AtomicCommitDraft,
        request_hash: &str,
        prepared: &WitnessRecord,
        atomic_projection: Option<AtomicProjection<'_>>,
    ) -> Result<(PersistedCommit, String), CanonicalStoreError> {
        let mut transaction =
            self.primary
                .begin()
                .await
                .map_err(|_| CanonicalStoreError::PrimaryWrite {
                    operation: "begin_transaction",
                })?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1 || ':' || $2, 0))")
            .bind(&draft.campaign_id)
            .bind(&draft.stream_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "lock_campaign_stream",
            })?;

        if let Some(existing) = load_existing_commit_in_transaction(
            &mut transaction,
            &draft.commit_id,
            &draft.campaign_id,
            &draft.stream_id,
            &draft.idempotency_key,
        )
        .await?
        {
            let stored_request_hash =
                load_request_hash_in_transaction(&mut transaction, &existing.commit_id).await?;
            if !stored_request_hash_matches(draft, &stored_request_hash) {
                return Err(CanonicalStoreError::IdempotencyConflict);
            }
            transaction
                .commit()
                .await
                .map_err(|_| CanonicalStoreError::PrimaryWrite {
                    operation: "commit_idempotent_transaction",
                })?;
            return Ok((existing, stored_request_hash));
        }

        let actual_version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(max(stream_version), 0) FROM event_store WHERE campaign_id = $1 AND stream_id = $2",
        )
        .bind(&draft.campaign_id)
        .bind(&draft.stream_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "read_stream_version",
        })?;
        if actual_version != draft.expected_version {
            return Err(CanonicalStoreError::VersionConflict {
                expected: draft.expected_version,
                actual: actual_version,
            });
        }

        let (event_sequences, event_hashes) = self
            .append_canonical_events(&mut transaction, draft, request_hash)
            .await?;

        let first_event_sequence =
            *event_sequences
                .first()
                .ok_or(CanonicalStoreError::Validation(
                    "at_least_one_event_required",
                ))?;
        let last_event_sequence =
            *event_sequences
                .last()
                .ok_or(CanonicalStoreError::Validation(
                    "at_least_one_event_required",
                ))?;
        let event_batch_hash = sha256_fields(&event_hashes);
        let audit_sequence = self
            .insert_audit(&mut transaction, draft, &event_batch_hash, prepared)
            .await?;
        let first_stream_version = draft.expected_version + 1;
        let last_stream_version = draft.expected_version + draft.events.len() as i64;

        sqlx::query(
            r#"
            INSERT INTO formal_commits (
                commit_id, campaign_id, idempotency_key, request_hash, expected_version,
                first_event_sequence, last_event_sequence, first_stream_version,
                last_stream_version, audit_sequence, witness_prepare_sequence,
                witness_prepare_hash, stream_id, idempotency_operation, status,
                result_event_sequence, response_payload
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17
            )
            "#,
        )
        .bind(&draft.commit_id)
        .bind(&draft.campaign_id)
        .bind(&draft.idempotency_key)
        .bind(request_hash)
        .bind(draft.expected_version)
        .bind(first_event_sequence)
        .bind(last_event_sequence)
        .bind(first_stream_version)
        .bind(last_stream_version)
        .bind(audit_sequence)
        .bind(prepared.sequence)
        .bind(&prepared.record_hash)
        .bind(&draft.stream_id)
        .bind(CANONICAL_IDEMPOTENCY_OPERATION)
        .bind("committed")
        .bind(last_event_sequence)
        .bind(Json(serde_json::json!({
            "first_event_sequence": first_event_sequence,
            "last_event_sequence": last_event_sequence,
            "first_stream_version": first_stream_version,
            "last_stream_version": last_stream_version,
        })))
        .execute(&mut *transaction)
        .await
        .map_err(|_| CanonicalStoreError::PrimaryWrite {
            operation: "insert_formal_commit",
        })?;

        if let Some(atomic_projection) = atomic_projection {
            let (projection, validation_error, capability_operation, apply_operation, statement) =
                match atomic_projection {
                    AtomicProjection::PlayerAction(projection) => (
                        projection,
                        "player_action_projection_must_be_object",
                        "set_player_action_projection_capability",
                        "apply_player_action_projection",
                        "SELECT core_domain.apply_player_action_projection($1, $2::JSONB)",
                    ),
                    AtomicProjection::CampaignInviteAcceptance(projection) => (
                        projection,
                        "campaign_invite_projection_must_be_object",
                        "set_campaign_invite_projection_capability",
                        "apply_campaign_invite_acceptance",
                        "SELECT core_domain.apply_campaign_invite_acceptance($1, $2::JSONB)",
                    ),
                    AtomicProjection::GameplayRollReservation(projection) => (
                        projection,
                        "gameplay_roll_reservation_must_be_object",
                        "set_gameplay_roll_reservation_capability",
                        "reserve_gameplay_roll_consumptions",
                        "SELECT core_domain.reserve_gameplay_roll_consumptions($1, $2::JSONB)",
                    ),
                    AtomicProjection::SessionEndingReservation(projection) => (
                        projection,
                        "session_ending_reservation_must_be_object",
                        "set_session_ending_reservation_capability",
                        "reserve_session_ending",
                        "SELECT core_domain.reserve_session_ending($1, $2::JSONB)",
                    ),
                };
            if !projection.is_object() {
                return Err(CanonicalStoreError::Validation(validation_error));
            }
            let projection_capability = self.derive_core_projection_capability(&draft.commit_id)?;
            sqlx::query("SELECT set_config('trpg.projection_capability', $1, TRUE)")
                .bind(projection_capability.as_str())
                .execute(&mut *transaction)
                .await
                .map_err(|_| CanonicalStoreError::PrimaryWrite {
                    operation: capability_operation,
                })?;
            sqlx::query(statement)
                .bind(&draft.commit_id)
                .bind(Json(projection.clone()))
                .execute(&mut *transaction)
                .await
                .map_err(|_| CanonicalStoreError::PrimaryWrite {
                    operation: apply_operation,
                })?;
        }

        transaction
            .commit()
            .await
            .map_err(|_| CanonicalStoreError::PrimaryWrite {
                operation: "commit_transaction",
            })?;

        Ok((
            PersistedCommit {
                commit_id: draft.commit_id.clone(),
                first_event_sequence,
                last_event_sequence,
                first_stream_version,
                last_stream_version,
                audit_sequence,
                witness_prepare_sequence: prepared.sequence,
                witness_prepare_hash: prepared.record_hash.clone(),
            },
            request_hash.to_owned(),
        ))
    }
}
