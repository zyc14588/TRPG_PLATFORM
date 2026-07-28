async fn replay_campaign_fork_batch(
    transaction: &mut Transaction<'_, Postgres>,
    replay: &CanonicalReplayEvent,
    event: CoreDomainEvent,
) -> Result<(), CoreDomainRepositoryError> {
    let CoreDomainEvent::CampaignForkMaterialized {
        fork_id,
        child_campaign_id,
        batch_index,
        batch_count,
        rows,
        ..
    } = event else { unreachable!("expected CampaignForkMaterialized"); };
    if replay.campaign_id != child_campaign_id
        || batch_index == 0
        || batch_index > batch_count
        || rows.is_empty()
        || rows
            .iter()
            .map(CampaignForkMaterializedRow::projection_target_count)
            .sum::<usize>()
            > 32
    {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_batch_replay_shape",
        ));
    }
    let manifest_matches: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM public.campaign_fork_materializations
             WHERE fork_id = $1
               AND campaign_id = $2
               AND batch_count = $3
        )
        "#,
    )
    .bind(&fork_id)
    .bind(&child_campaign_id)
    .bind(
        i64::try_from(batch_count)
            .map_err(|_| CoreDomainRepositoryError::Integrity("fork_batch_count"))?,
    )
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("load_fork_manifest_for_batch"))?;
    if !manifest_matches {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_batch_manifest_mismatch",
        ));
    }
    let expected_data_subject_id = rows.first().map(fork_row_data_subject).ok_or(
        CoreDomainRepositoryError::Integrity("fork_batch_replay_shape"),
    )?;
    let event_data_subject_id: String = sqlx::query_scalar(
        "SELECT data_subject_id FROM public.event_store WHERE sequence = $1",
    )
    .bind(replay.sequence)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("load_fork_event_data_subject"))?;
    if event_data_subject_id != expected_data_subject_id {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_event_data_subject_mismatch",
        ));
    }
    for row in rows {
        let (row_visibility_label, row_visibility_subject) = fork_row_visibility(&row);
        if row_visibility_label != replay.visibility_label
            || row_visibility_subject != replay.visibility_subject
        {
            return Err(CoreDomainRepositoryError::Integrity(
                "fork_row_visibility_mismatch",
            ));
        }
        apply_campaign_fork_materialized_row(
            transaction,
            replay,
            &fork_id,
            &child_campaign_id,
            row,
        )
        .await?;
    }
    if batch_index == batch_count {
        verify_fork_materialized_row_count(
            transaction,
            &fork_id,
            &child_campaign_id,
        )
        .await?;
    }
    Ok(())
}
