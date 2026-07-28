
async fn apply_reconsideration_replay_event(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    event: &CoreDomainEvent,
) -> Result<(), CoreDomainRepositoryError> {
    match event {
        CoreDomainEvent::ReconsiderationRequested {
            reconsideration_id,
            campaign_id,
            original_event_sequence,
            requested_by,
            reason,
            ..
        } => {
            if campaign_id != &replay.campaign_id || *original_event_sequence == 0 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_replay_request_shape",
                ));
            }
            let current_version: Option<i64> = sqlx::query_scalar(
                "SELECT version FROM public.reconsiderations WHERE reconsideration_id = $1",
            )
            .bind(reconsideration_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(database_error("load_reconsideration_replay_request"))?;
            if current_version.is_some_and(|current| current > 1) {
                return Ok(());
            }
            if current_version.is_none() {
                sqlx::query(
                    r#"
                    INSERT INTO public.reconsiderations (
                        reconsideration_id, campaign_id, original_event_sequence,
                        requested_by, reason, state, resolution, event_chain, version,
                        review_workflow_version,
                        visibility_label, visibility_subject,
                        provenance_kind, provenance_reference, provenance_recorded_by,
                        last_event_sequence
                    ) VALUES (
                        $1, $2, $3, $4, $5, 'REQUESTED', NULL,
                        jsonb_build_array($6::TEXT), 1, 2,
                        $7, $8, $9, $10, $11, $12
                    )
                    ON CONFLICT (reconsideration_id) DO NOTHING
                    "#,
                )
                .bind(reconsideration_id)
                .bind(campaign_id)
                .bind(i64::try_from(*original_event_sequence).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("reconsideration_source_sequence")
                })?)
                .bind(requested_by)
                .bind(reason)
                .bind(&replay.command_id)
                .bind(&replay.visibility_label)
                .bind(&replay.visibility_subject)
                .bind(&replay.provenance_kind)
                .bind(&replay.provenance_reference)
                .bind(&replay.provenance_recorded_by)
                .bind(replay.sequence)
                .execute(&mut **transaction)
                .await
                .map_err(database_error("replay_reconsideration_request"))?;
            }
            let matches: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM public.reconsiderations
                     WHERE reconsideration_id = $1 AND campaign_id = $2
                       AND original_event_sequence = $3 AND requested_by = $4
                       AND reason = $5 AND state = 'REQUESTED'
                       AND event_chain = jsonb_build_array($6::TEXT)
                       AND version = 1 AND review_workflow_version = 2
                       AND last_event_sequence = $7
                )
                "#,
            )
            .bind(reconsideration_id)
            .bind(campaign_id)
            .bind(i64::try_from(*original_event_sequence).map_err(|_| {
                CoreDomainRepositoryError::Integrity("reconsideration_source_sequence")
            })?)
            .bind(requested_by)
            .bind(reason)
            .bind(&replay.command_id)
            .bind(replay.sequence)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_reconsideration_request"))?;
            if !matches {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_request_replay_mismatch",
                ));
            }
        }
        CoreDomainEvent::ReconsiderationReviewed {
            reconsideration_id,
            review_event_id,
            review_summary,
            ..
        } => {
            let normalized_review_summary = review_summary.trim();
            if normalized_review_summary.is_empty() || normalized_review_summary.len() > 512 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_replay_review_shape",
                ));
            }
            let current_version: i64 = sqlx::query_scalar(
                "SELECT version FROM public.reconsiderations WHERE reconsideration_id = $1",
            )
            .bind(reconsideration_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("load_reconsideration_replay_review"))?;
            if current_version > 2 {
                return Ok(());
            }
            if current_version == 1 {
                sqlx::query(
                    r#"
                    UPDATE public.reconsiderations
                       SET state = 'REVIEWED',
                           review_summary = $1,
                           event_chain = event_chain || jsonb_build_array($2::TEXT),
                           version = 2,
                           visibility_label = $3,
                           visibility_subject = $4,
                           provenance_kind = $5,
                           provenance_reference = $6,
                           provenance_recorded_by = $7,
                           last_event_sequence = $8
                     WHERE reconsideration_id = $9
                       AND state = 'REQUESTED' AND version = 1
                    "#,
                )
                .bind(normalized_review_summary)
                .bind(review_event_id)
                .bind(&replay.visibility_label)
                .bind(&replay.visibility_subject)
                .bind(&replay.provenance_kind)
                .bind(&replay.provenance_reference)
                .bind(&replay.provenance_recorded_by)
                .bind(replay.sequence)
                .bind(reconsideration_id)
                .execute(&mut **transaction)
                .await
                .map_err(database_error("replay_reconsideration_review"))?;
            }
            let matches: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM public.reconsiderations
                     WHERE reconsideration_id = $1 AND state = 'REVIEWED'
                       AND review_summary = $2
                       AND event_chain ->> 1 = $3
                       AND jsonb_array_length(event_chain) = 2
                       AND version = 2 AND last_event_sequence = $4
                )
                "#,
            )
            .bind(reconsideration_id)
            .bind(normalized_review_summary)
            .bind(review_event_id)
            .bind(replay.sequence)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_reconsideration_review"))?;
            if !matches {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_review_replay_mismatch",
                ));
            }
        }
        CoreDomainEvent::ReconsiderationUpheld {
            reconsideration_id,
            resolution_event_id,
            original_event_sequence,
            resolution,
            ..
        }
        | CoreDomainEvent::ReconsiderationCorrected {
            reconsideration_id,
            resolution_event_id,
            original_event_sequence,
            resolution,
            ..
        } => {
            let normalized_resolution = resolution.trim();
            if normalized_resolution.is_empty() || normalized_resolution.len() > 512 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_replay_resolution_shape",
                ));
            }
            let current_version: i64 = sqlx::query_scalar(
                "SELECT version FROM public.reconsiderations WHERE reconsideration_id = $1",
            )
            .bind(reconsideration_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("load_reconsideration_replay_resolution"))?;
            let (outcome, corrected_event_type, corrected_payload_json) = match event {
                CoreDomainEvent::ReconsiderationUpheld { .. } => ("UPHELD", None, None),
                CoreDomainEvent::ReconsiderationCorrected {
                    corrected_event_type,
                    corrected_payload_json,
                    ..
                } => {
                    let payload: Value =
                        serde_json::from_str(corrected_payload_json).map_err(|_| {
                            CoreDomainRepositoryError::Integrity(
                                "reconsideration_corrected_payload",
                            )
                        })?;
                    if !payload.is_object() {
                        return Err(CoreDomainRepositoryError::Integrity(
                            "reconsideration_corrected_payload",
                        ));
                    }
                    (
                        "CORRECTED",
                        Some(corrected_event_type.as_str()),
                        Some(corrected_payload_json.as_str()),
                    )
                }
                _ => unreachable!("matched reconsideration resolution above"),
            };
            if current_version == 2 {
                sqlx::query(
                    r#"
                    UPDATE public.reconsiderations
                       SET state = 'RESOLVED', outcome = $1, resolution = $2,
                           corrected_event_type = $3,
                           corrected_payload = $4::JSONB,
                           event_chain = event_chain || jsonb_build_array($5::TEXT),
                           version = 3,
                           visibility_label = $6,
                           visibility_subject = $7,
                           provenance_kind = $8,
                           provenance_reference = $9,
                           provenance_recorded_by = $10,
                           last_event_sequence = $11
                     WHERE reconsideration_id = $12
                       AND state = 'REVIEWED' AND version = 2
                       AND original_event_sequence = $13
                    "#,
                )
                .bind(outcome)
                .bind(normalized_resolution)
                .bind(corrected_event_type)
                .bind(corrected_payload_json)
                .bind(resolution_event_id)
                .bind(&replay.visibility_label)
                .bind(&replay.visibility_subject)
                .bind(&replay.provenance_kind)
                .bind(&replay.provenance_reference)
                .bind(&replay.provenance_recorded_by)
                .bind(replay.sequence)
                .bind(reconsideration_id)
                .bind(i64::try_from(*original_event_sequence).map_err(|_| {
                    CoreDomainRepositoryError::Integrity("reconsideration_source_sequence")
                })?)
                .execute(&mut **transaction)
                .await
                .map_err(database_error("replay_reconsideration_resolution"))?;
            } else if current_version != 3 {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_replay_sequence_gap",
                ));
            }
            let matches: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM public.reconsiderations
                     WHERE reconsideration_id = $1 AND state = 'RESOLVED'
                       AND outcome = $2 AND resolution = $3
                       AND corrected_event_type IS NOT DISTINCT FROM $4
                       AND corrected_payload IS NOT DISTINCT FROM $5::JSONB
                       AND event_chain ->> 2 = $6
                       AND jsonb_array_length(event_chain) = 3
                       AND version = 3 AND last_event_sequence = $7
                )
                "#,
            )
            .bind(reconsideration_id)
            .bind(outcome)
            .bind(normalized_resolution)
            .bind(corrected_event_type)
            .bind(corrected_payload_json)
            .bind(resolution_event_id)
            .bind(replay.sequence)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error("verify_replayed_reconsideration_resolution"))?;
            if !matches {
                return Err(CoreDomainRepositoryError::Integrity(
                    "reconsideration_resolution_replay_mismatch",
                ));
            }
        }
        _ => {
            return Err(CoreDomainRepositoryError::Integrity(
                "reconsideration_replay_event_type",
            ))
        }
    }
    Ok(())
}

async fn lock_ending_projection_identity(
    transaction: &mut Transaction<'_, Postgres>,
    ending_event_id: &str,
) -> Result<(), CoreDomainRepositoryError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("p08-ending-id:{ending_event_id}"))
        .execute(&mut **transaction)
        .await
        .map_err(database_error("lock_ending_identity"))?;
    Ok(())
}
