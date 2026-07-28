async fn apply_fork_public_event_row(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    fork_id: &String,
    child_campaign_id: &String,
    row: CampaignForkMaterializedRow,
) -> Result<(), CoreDomainRepositoryError> {
    let CampaignForkMaterializedRow::PublicEvent {
        fork_event_id,
        source_event_sequence,
        source_event_type,
        source_resource_type,
        source_resource_id,
        source_payload_json,
        source_event_integrity_hash,
        visibility_label,
        visibility_subject,
    } = row else { unreachable!("expected fork PublicEvent row"); };
    let source_event_sequence =
        i64::try_from(source_event_sequence).map_err(|_| {
            CoreDomainRepositoryError::Integrity("fork_public_event_sequence")
        })?;
    let source_payload: Value = serde_json::from_str(&source_payload_json)
        .map_err(|_| {
            CoreDomainRepositoryError::Integrity("fork_public_event_payload")
        })?;
    if !source_payload.is_object()
        || !matches!(visibility_label.as_str(), "public" | "party_visible")
        || visibility_subject != "not_applicable"
        || !source_event_integrity_hash.starts_with("hmac-sha256:")
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_public_event_shape",
        ));
    }
    let source_matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
              FROM public.campaign_forks AS fork
              JOIN public.event_store AS source_event
                ON source_event.campaign_id =
                   fork.parent_campaign_id
               AND source_event.sequence = $2
             WHERE fork.fork_id = $1
               AND fork.child_campaign_id = $3
               AND source_event.event_type = $4
               AND source_event.resource_type = $5
               AND source_event.resource_id = $6
               AND source_event.event_integrity_hash = $7
               AND source_event.visibility_label = $8
               AND source_event.visibility_subject = $9
               AND source_event.integrity_status = 'verified_hmac'
               AND source_event.request_hash_source = 'formal_commit'
        )
        "#,
    )
    .bind(fork_id)
    .bind(source_event_sequence)
    .bind(child_campaign_id)
    .bind(&source_event_type)
    .bind(&source_resource_type)
    .bind(&source_resource_id)
    .bind(&source_event_integrity_hash)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_fork_public_event_source"))?;
    if !source_matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_public_event_source_mismatch",
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO public.campaign_fork_public_events (
            fork_event_id, fork_id, campaign_id,
            source_event_sequence, source_event_type,
            source_resource_type, source_resource_id,
            source_payload_json, source_event_integrity_hash,
            version, visibility_label, visibility_subject,
            provenance_kind, provenance_reference,
            provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8::JSONB, $9,
            1, $10, $11, $12, $13, $14, $15
        )
        ON CONFLICT (fork_event_id) DO NOTHING
        "#,
    )
    .bind(&fork_event_id)
    .bind(fork_id)
    .bind(child_campaign_id)
    .bind(source_event_sequence)
    .bind(&source_event_type)
    .bind(&source_resource_type)
    .bind(&source_resource_id)
    .bind(&source_payload_json)
    .bind(&source_event_integrity_hash)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_fork_public_event"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
              FROM public.campaign_fork_public_events
             WHERE fork_event_id = $1
               AND fork_id = $2
               AND campaign_id = $3
               AND source_event_sequence = $4
               AND source_event_type = $5
               AND source_resource_type = $6
               AND source_resource_id = $7
               AND source_payload_json = $8::JSONB
               AND source_event_integrity_hash = $9
               AND visibility_label::TEXT = $10
               AND visibility_subject = $11
               AND last_event_sequence = $12
        )
        "#,
    )
    .bind(&fork_event_id)
    .bind(fork_id)
    .bind(child_campaign_id)
    .bind(source_event_sequence)
    .bind(&source_event_type)
    .bind(&source_resource_type)
    .bind(&source_resource_id)
    .bind(&source_payload_json)
    .bind(&source_event_integrity_hash)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_fork_public_event"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_public_event_identity_conflict",
        ));
    }
    Ok(())
}
