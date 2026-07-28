async fn verify_fork_materialized_row_count(
    transaction: &mut Transaction<'_, Postgres>,
    fork_id: &String,
    child_campaign_id: &String,
) -> Result<(), CoreDomainRepositoryError> {
    let expected_rows: i64 = sqlx::query_scalar(
        "SELECT materialized_row_count \
               FROM public.campaign_fork_materializations \
              WHERE fork_id = $1 AND campaign_id = $2",
    )
    .bind(fork_id)
    .bind(child_campaign_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("load_fork_expected_row_count"))?;
    let actual_rows: i64 = sqlx::query_scalar(
        r#"
            WITH fork_targets AS (
                SELECT DISTINCT
                       target ->> 'relation' AS relation_name,
                       target ->> 'row_id' AS row_id
                  FROM public.event_store AS event
                  CROSS JOIN LATERAL jsonb_array_elements(
                       event.projection_targets
                  ) AS target
                 WHERE event.campaign_id = $1
                   AND event.stream_id = $2
                   AND event.event_type =
                       'CampaignForkMaterialized'
                   AND event.integrity_status = 'verified_hmac'
                   AND event.request_hash_source = 'formal_commit'
                   AND event.event_integrity_hash IS NOT NULL
                   AND target ->> 'relation' <>
                       'public.character_sheet_versions'
            ),
            materialized_rows AS (
                SELECT 'public.scenarios' AS relation_name,
                       scenario_id AS row_id
                  FROM public.scenarios
                 WHERE campaign_id = $1
                UNION ALL
                SELECT 'public.characters', character_id
                  FROM public.characters
                 WHERE campaign_id = $1
                UNION ALL
                SELECT 'core_domain.sessions', session_id
                  FROM core_domain.sessions
                 WHERE campaign_id = $1
                UNION ALL
                SELECT 'public.scenes', scene_id
                  FROM public.scenes
                 WHERE campaign_id = $1
                UNION ALL
                SELECT 'public.campaign_fork_public_events',
                       fork_event_id
                  FROM public.campaign_fork_public_events
                 WHERE campaign_id = $1 AND fork_id = $2
                UNION ALL
                SELECT 'public.campaign_fork_clues', fork_clue_id
                  FROM public.campaign_fork_clues
                 WHERE campaign_id = $1 AND fork_id = $2
                UNION ALL
                SELECT 'public.campaign_fork_npc_states',
                       npc_state_id
                  FROM public.campaign_fork_npc_states
                 WHERE campaign_id = $1 AND fork_id = $2
                UNION ALL
                SELECT 'public.combat_states', combat_id
                  FROM public.combat_states
                 WHERE campaign_id = $1
                UNION ALL
                SELECT 'public.chase_states', chase_id
                  FROM public.chase_states
                 WHERE campaign_id = $1
                UNION ALL
                SELECT 'public.ending_events', ending_event_id
                  FROM public.ending_events
                 WHERE campaign_id = $1
            )
            SELECT count(*)
              FROM fork_targets
              JOIN materialized_rows
                USING (relation_name, row_id)
            "#,
    )
    .bind(child_campaign_id)
    .bind(fork_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(database_error("count_fork_materialized_rows"))?;
    if actual_rows != expected_rows {
        return Err(CoreDomainRepositoryError::Integrity(
            "fork_materialized_row_count_mismatch",
        ));
    }
    Ok(())
}
