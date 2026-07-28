async fn replay_campaign_fork_manifest(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    event: CoreDomainEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let CoreDomainEvent::CampaignForkMaterializationRecorded {
        fork_id,
        child_campaign_id,
        child_session_id,
        child_scenario_id,
        child_snapshot_hash,
        child_state_json,
        materialized_row_count,
        batch_count,
        ..
    } = event else { unreachable!("expected CampaignForkMaterializationRecorded"); };
    if replay.campaign_id != child_campaign_id {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_manifest_replay_campaign_mismatch",
        ));
    }
    let row_count = i64::try_from(materialized_row_count)
        .map_err(|_| CoreDomainRepositoryError::Integrity("fork_row_count"))?;
    let batch_count = i64::try_from(batch_count)
        .map_err(|_| CoreDomainRepositoryError::Integrity("fork_batch_count"))?;
    sqlx::query(
        r#"
        INSERT INTO public.campaign_fork_materializations (
            fork_id, campaign_id, parent_campaign_id, source_session_id,
            child_session_id, child_scenario_id,
            source_snapshot_hash, child_snapshot_hash, child_state_json,
            materialized_row_count, batch_count, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        )
        SELECT fork.fork_id, fork.child_campaign_id,
               fork.parent_campaign_id, fork.source_session_id,
               $2, $3, fork.source_snapshot_hash, $4, $5,
               $6, $7, 1, $8, $9, $10, $11, $12, $13
          FROM public.campaign_forks AS fork
         WHERE fork.fork_id = $1
           AND fork.child_campaign_id = $14
        ON CONFLICT (fork_id) DO NOTHING
        "#,
    )
    .bind(&fork_id)
    .bind(&child_session_id)
    .bind(&child_scenario_id)
    .bind(&child_snapshot_hash)
    .bind(&child_state_json)
    .bind(row_count)
    .bind(batch_count)
    .bind(&replay.visibility_label)
    .bind(&replay.visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .bind(&child_campaign_id)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_campaign_fork_manifest"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.campaign_fork_materializations
             WHERE fork_id = $1
               AND campaign_id = $2
               AND child_session_id = $3
               AND child_scenario_id = $4
               AND child_snapshot_hash = $5
               AND child_state_json = $6
               AND materialized_row_count = $7
               AND batch_count = $8
               AND last_event_sequence = $9
        )
        "#,
    )
    .bind(&fork_id)
    .bind(&child_campaign_id)
    .bind(&child_session_id)
    .bind(&child_scenario_id)
    .bind(&child_snapshot_hash)
    .bind(&child_state_json)
    .bind(row_count)
    .bind(batch_count)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_campaign_fork_manifest"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_manifest_replay_identity_conflict",
        ));
    }
    Ok(())
}
