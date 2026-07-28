async fn replay_campaign_fork_recorded(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    event: CoreDomainEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let CoreDomainEvent::CampaignForkRecorded {
        fork_id,
        parent_campaign_id,
        child_campaign_id,
        source_session_id,
        snapshot_hash,
        child_snapshot_hash,
        copy_scopes,
        canonical_snapshot_json,
        reason,
        ..
    } = event else { unreachable!("expected CampaignForkRecorded"); };
    if replay.campaign_id != child_campaign_id {
        return Err(CoreDomainRepositoryError::Integrity(
            "campaign_fork_replay_campaign_mismatch",
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO public.campaign_forks (
            fork_id, campaign_id, parent_campaign_id, child_campaign_id,
            source_session_id, source_snapshot_hash, reason, version,
            child_snapshot_hash, copy_scope_json, snapshot_json,
            materialization_version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            $1, $2, $3, $2, $4, $5, $6, 1,
            $7, $8, $9::JSONB, 2,
            $10, $11, $12, $13, $14, $15
        )
        ON CONFLICT (fork_id) DO NOTHING
        "#,
    )
    .bind(&fork_id)
    .bind(&child_campaign_id)
    .bind(&parent_campaign_id)
    .bind(&source_session_id)
    .bind(&snapshot_hash)
    .bind(reason.trim())
    .bind(&child_snapshot_hash)
    .bind(sqlx::types::Json(&copy_scopes))
    .bind(&canonical_snapshot_json)
    .bind(&replay.visibility_label)
    .bind(&replay.visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_campaign_fork"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.campaign_forks
             WHERE fork_id = $1
               AND campaign_id = $2
               AND parent_campaign_id = $3
               AND child_campaign_id = $2
               AND source_session_id = $4
               AND source_snapshot_hash = $5
               AND child_snapshot_hash = $6
               AND copy_scope_json = $7
               AND snapshot_json = $8::JSONB
               AND materialization_version = 2
               AND last_event_sequence = $9
        )
        "#,
    )
    .bind(&fork_id)
    .bind(&child_campaign_id)
    .bind(&parent_campaign_id)
    .bind(&source_session_id)
    .bind(&snapshot_hash)
    .bind(&child_snapshot_hash)
    .bind(sqlx::types::Json(&copy_scopes))
    .bind(&canonical_snapshot_json)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_campaign_fork"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "campaign_fork_replay_identity_conflict",
        ));
    }
    Ok(())
}
