async fn apply_fork_conclusion_row(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    _fork_id: &String,
    child_campaign_id: &String,
    row: CampaignForkMaterializedRow,
) -> Result<(), CoreDomainRepositoryError> {
    let CampaignForkMaterializedRow::Conclusion {
        ending_event_id,
        session_id,
        ending_id,
        summary,
        ended_at_unix_ms,
        visibility_label,
        visibility_subject,
    } = row else { unreachable!("expected fork Conclusion row"); };
    if ending_id.trim().is_empty()
        || summary.trim().is_empty()
        || summary.len() > 1_024
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_conclusion_shape",
        ));
    }
    let ended_at =
        timestamp_from_unix_ms(ended_at_unix_ms, "fork_ending.ended_at")?;
    lock_ending_projection_identity(transaction, &ending_event_id).await?;
    sqlx::query(
        r#"
        INSERT INTO public.ending_events (
            ending_event_id, campaign_id, session_id,
            ending_id, summary, ended_at, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference,
            provenance_recorded_by, last_event_sequence
        ) VALUES (
            $1, $2, $3, $4, $5, $6, 1,
            $7, $8, $9, $10, $11, $12
        )
        ON CONFLICT (ending_event_id) DO NOTHING
        "#,
    )
    .bind(&ending_event_id)
    .bind(child_campaign_id)
    .bind(&session_id)
    .bind(&ending_id)
    .bind(summary.trim())
    .bind(ended_at)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(&replay.provenance_kind)
    .bind(&replay.provenance_reference)
    .bind(&replay.provenance_recorded_by)
    .bind(replay.sequence)
    .execute(&mut **transaction)
    .await
    .map_err(database_error("replay_fork_conclusion"))?;
    let matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.ending_events
             WHERE ending_event_id = $1
               AND campaign_id = $2
               AND session_id = $3
               AND ending_id = $4
               AND summary = $5
               AND ended_at = $6
               AND visibility_label::TEXT = $7
               AND visibility_subject = $8
               AND last_event_sequence = $9
        )
        "#,
    )
    .bind(&ending_event_id)
    .bind(child_campaign_id)
    .bind(&session_id)
    .bind(&ending_id)
    .bind(summary.trim())
    .bind(ended_at)
    .bind(&visibility_label)
    .bind(&visibility_subject)
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("verify_replayed_fork_conclusion"))?;
    if !matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_conclusion_identity_conflict",
        ));
    }
    Ok(())
}
