{
    sqlx::query(
        r#"
        INSERT INTO public.reconsiderations (
            reconsideration_id, campaign_id, original_event_sequence,
            requested_by, reason, state, resolution, event_chain, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence, review_workflow_version, review_summary,
            outcome, corrected_event_type, corrected_payload
        )
        SELECT 'reconsideration_p08_ghost', campaign_id,
               original_event_sequence, requested_by, reason, state,
               resolution, event_chain, version, visibility_label,
               visibility_subject, provenance_kind, provenance_reference,
               provenance_recorded_by, last_event_sequence,
               review_workflow_version, review_summary, outcome,
               corrected_event_type, corrected_payload
          FROM public.reconsiderations
         WHERE campaign_id = $1
         ORDER BY reconsideration_id
         LIMIT 1
        "#,
    )
    .bind(CAMPAIGN_ID)
    .execute(&mut *corrupt_p08_projection)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO public.growth_events (
            growth_event_id, campaign_id, session_id, ending_event_id,
            character_id, source_sheet_version_id, new_sheet_version_id,
            skill_name, skill_before, improvement_check_roll,
            increase_roll, skill_after, server_roll_id, increase_roll_id,
            random_source, version, visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        )
        SELECT 'growth_event_p08_ghost', campaign_id, session_id,
               ending_event_id, character_id, source_sheet_version_id,
               new_sheet_version_id, 'Ghost Skill', skill_before,
               improvement_check_roll, increase_roll, skill_after,
               'server_percentile_p08_ghost',
               CASE WHEN increase_roll_id IS NULL
                    THEN NULL ELSE 'server_d10_p08_ghost' END,
               random_source, version, visibility_label, visibility_subject,
               provenance_kind, provenance_reference, provenance_recorded_by,
               last_event_sequence
          FROM public.growth_events
         WHERE growth_event_id = 'growth_event_p08_schema'
        "#,
    )
    .execute(&mut *corrupt_p08_projection)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO public.combat_states (
            combat_id, campaign_id, session_id, status, round,
            current_turn_index, state_json, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        )
        SELECT 'combat_p08_ghost', campaign_id, session_id, status, round,
               current_turn_index,
               jsonb_set(state_json, '{combat_id}', '"combat_p08_ghost"'::jsonb),
               version, visibility_label, visibility_subject,
               provenance_kind, provenance_reference, provenance_recorded_by,
               last_event_sequence
          FROM public.combat_states
         WHERE combat_id = 'combat_p08_schema'
        "#,
    )
    .execute(&mut *corrupt_p08_projection)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO public.chase_states (
            chase_id, campaign_id, session_id, status, range_band,
            segment, state_json, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        )
        SELECT 'chase_p08_ghost', campaign_id, session_id, status, range_band,
               segment,
               jsonb_set(state_json, '{chase_id}', '"chase_p08_ghost"'::jsonb),
               version, visibility_label, visibility_subject,
               provenance_kind, provenance_reference, provenance_recorded_by,
               last_event_sequence
          FROM public.chase_states
         WHERE chase_id = 'chase_p08_schema'
        "#,
    )
    .execute(&mut *corrupt_p08_projection)
    .await
    .unwrap();
    sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(&mut *corrupt_p08_projection)
        .await
        .unwrap();
    for statement in [
        "ALTER TABLE public.combat_states ENABLE TRIGGER combat_states_event_guard",
        "ALTER TABLE public.chase_states ENABLE TRIGGER chase_states_event_guard",
        "ALTER TABLE public.ending_events ENABLE TRIGGER ending_events_event_guard",
        "ALTER TABLE public.growth_events ENABLE TRIGGER growth_events_event_guard",
        "ALTER TABLE public.reconsiderations ENABLE TRIGGER reconsiderations_event_guard",
        "ALTER TABLE public.characters ENABLE TRIGGER characters_event_guard",
        "ALTER TABLE public.character_sheet_versions ENABLE TRIGGER character_sheet_versions_event_guard",
    ] {
        sqlx::query(statement)
            .execute(&mut *corrupt_p08_projection)
            .await
            .unwrap();
    }
    corrupt_p08_projection.commit().await.unwrap();
    let repaired_p08 = api_repository
        .rebuild_p08_projections(CAMPAIGN_ID)
        .await
        .expect("replace same-version corruption and remove non-canonical ghost projections");
    assert_eq!(repaired_p08.combat_states, 1);
    assert_eq!(repaired_p08.chase_states, 1);
    assert_eq!(repaired_p08.reconsiderations, 2);
    assert_eq!(repaired_p08.ending_events, 1);
    assert_eq!(repaired_p08.growth_events, 1);
    assert_eq!(
        repaired_p08.gameplay_roll_consumptions,
        p08_roll_consumptions_before
    );
    let remaining_corruption: i64 = sqlx::query_scalar(
        r#"
        SELECT
            (SELECT count(*) FROM public.combat_states
              WHERE campaign_id = $1
                AND (combat_id = 'combat_p08_ghost'
                     OR state_json ? 'corrupted'
                     OR provenance_reference = 'corrupted_same_version'))
          + (SELECT count(*) FROM public.chase_states
              WHERE campaign_id = $1
                AND (chase_id = 'chase_p08_ghost'
                     OR state_json ? 'corrupted'
                     OR provenance_reference = 'corrupted_same_version'))
          + (SELECT count(*) FROM public.ending_events
              WHERE campaign_id = $1
                AND (summary = 'CORRUPTED ENDING'
                     OR provenance_reference = 'corrupted_same_version'))
          + (SELECT count(*) FROM public.growth_events
              WHERE campaign_id = $1
                AND (growth_event_id = 'growth_event_p08_ghost'
                     OR provenance_reference = 'corrupted_same_version'))
          + (SELECT count(*) FROM public.reconsiderations
              WHERE campaign_id = $1
                AND (reconsideration_id = 'reconsideration_p08_ghost'
                     OR review_summary = 'CORRUPTED REVIEW'
                     OR provenance_reference = 'corrupted_same_version'))
          + (SELECT count(*) FROM public.characters
              WHERE campaign_id = $1
                AND character_id = 'character_p06_player'
                AND provenance_reference = 'corrupted_same_version')
          + (SELECT count(*) FROM public.character_sheet_versions
              WHERE campaign_id = $1
                AND sheet_version_id = 'sheet_p06_player_v2'
                AND sheet_json ? 'corrupted')
        "#,
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        remaining_corruption, 0,
        "rebuild must replace same-version corruption and delete ghost rows"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store WHERE campaign_id = $1",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        event_count_before,
        "repairing corrupted projections must not rewrite canonical history"
    );
    let approval_event_sequence: i64 = sqlx::query_scalar(
        "SELECT sequence FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = 'character_p06_player' \
           AND event_type = 'CharacterInitialVersionApproved'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let mut remove_p08_projection = primary.begin().await.unwrap();
    sqlx::query("SET CONSTRAINTS ALL DEFERRED")
        .execute(&mut *remove_p08_projection)
        .await
        .unwrap();
    sqlx::query("DELETE FROM public.growth_events WHERE campaign_id = $1")
        .bind(CAMPAIGN_ID)
        .execute(&mut *remove_p08_projection)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE public.characters DISABLE TRIGGER characters_event_guard")
        .execute(&mut *remove_p08_projection)
        .await
        .unwrap();
    sqlx::query(
        r#"
        UPDATE public.characters AS character
           SET current_sheet_version = 1,
               version = 3,
               visibility_label = event.visibility_label::core_domain.visibility_label,
               visibility_subject = event.visibility_subject,
               provenance_kind = event.fact_provenance_kind::core_domain.provenance_kind,
               provenance_reference = event.fact_provenance_reference,
               provenance_recorded_by = event.fact_recorded_by,
               last_event_sequence = event.sequence
          FROM public.event_store AS event
         WHERE character.character_id = 'character_p06_player'
           AND event.sequence = $1
        "#,
    )
    .bind(approval_event_sequence)
    .execute(&mut *remove_p08_projection)
    .await
    .unwrap();
    sqlx::query("ALTER TABLE public.characters ENABLE TRIGGER characters_event_guard")
        .execute(&mut *remove_p08_projection)
        .await
        .unwrap();
    sqlx::query(
        "DELETE FROM public.character_sheet_versions \
         WHERE sheet_version_id = 'sheet_p06_player_v2'",
    )
    .execute(&mut *remove_p08_projection)
    .await
    .unwrap();
    for statement in [
        "DELETE FROM public.ending_events WHERE campaign_id = $1",
        "DELETE FROM public.reconsiderations WHERE campaign_id = $1",
        "DELETE FROM public.gameplay_roll_consumptions WHERE campaign_id = $1",
        "DELETE FROM public.combat_states WHERE campaign_id = $1",
        "DELETE FROM public.chase_states WHERE campaign_id = $1",
    ] {
        sqlx::query(statement)
            .bind(CAMPAIGN_ID)
            .execute(&mut *remove_p08_projection)
            .await
            .unwrap();
    }
    remove_p08_projection.commit().await.unwrap();
    let rebuilt_p08 = api_repository
        .rebuild_p08_projections(CAMPAIGN_ID)
        .await
        .expect("rebuild all P08 projections solely from canonical Event Store history");
    assert_eq!(rebuilt_p08.replayed_events, 24);
    assert_eq!(rebuilt_p08.combat_states, 1);
    assert_eq!(rebuilt_p08.chase_states, 1);
    assert_eq!(
        rebuilt_p08.gameplay_roll_consumptions,
        p08_roll_consumptions_before
    );
    assert_eq!(rebuilt_p08.reconsiderations, 2);
    assert_eq!(rebuilt_p08.ending_events, 1);
    assert_eq!(rebuilt_p08.growth_events, 1);
    let p08_projection_after: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'combat', (SELECT to_jsonb(combat) FROM public.combat_states AS combat
                        WHERE combat.campaign_id = $1),
            'chase', (SELECT to_jsonb(chase) FROM public.chase_states AS chase
                       WHERE chase.campaign_id = $1),
            'roll_consumptions', (
                SELECT jsonb_agg(to_jsonb(consumption)
                                 ORDER BY consumption.roll_id)
                  FROM public.gameplay_roll_consumptions AS consumption
                 WHERE consumption.campaign_id = $1
            ),
            'ending', (SELECT to_jsonb(ending) FROM public.ending_events AS ending
                        WHERE ending.campaign_id = $1),
            'growth', (SELECT to_jsonb(growth) FROM public.growth_events AS growth
                        WHERE growth.campaign_id = $1),
            'growth_sheet', (
                SELECT to_jsonb(sheet)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.sheet_version_id = 'sheet_p06_player_v2'
            ),
            'character', (
                SELECT to_jsonb(character)
                  FROM public.characters AS character
                 WHERE character.character_id = 'character_p06_player'
            ),
            'reconsiderations', (
                SELECT jsonb_agg(to_jsonb(reconsideration)
                                 ORDER BY reconsideration.reconsideration_id)
                  FROM public.reconsiderations AS reconsideration
                 WHERE reconsideration.campaign_id = $1
            )
        )
        "#,
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        p08_projection_after, p08_projection_before,
        "P08 replay must reproduce the exact combat/chase/reconsideration/ending/growth projections"
    );
    let rebuilt = repository
        .rebuild_session_scene_projection(CAMPAIGN_ID)
        .await
        .expect("rebuild Session/Scene projection solely from canonical events");
    assert_eq!(rebuilt.restored_sessions, 1);
    assert_eq!(rebuilt.restored_scenes, 2);
    assert_eq!(rebuilt.replayed_events, 5);
    let event_count_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(
        event_count_after, event_count_before,
        "projection rebuild must not rewrite Event Store history"
    );
    include!("18_projection_rebuild_verification.rs");
}
