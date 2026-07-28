{
    let child_projection_before: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'fork', (SELECT to_jsonb(fork) FROM public.campaign_forks AS fork
                      WHERE fork.fork_id = 'fork_p06_schema'),
            'manifest', (SELECT to_jsonb(manifest)
                           FROM public.campaign_fork_materializations AS manifest
                          WHERE manifest.fork_id = 'fork_p06_schema'),
            'scenario', (SELECT to_jsonb(scenario) FROM public.scenarios AS scenario
                          WHERE scenario.campaign_id = $1),
            'characters', (SELECT jsonb_agg(to_jsonb(character)
                                            ORDER BY character.character_id)
                             FROM public.characters AS character
                            WHERE character.campaign_id = $1),
            'sheets', (SELECT jsonb_agg(to_jsonb(sheet)
                                        ORDER BY sheet.sheet_version_id)
                         FROM public.character_sheet_versions AS sheet
                        WHERE sheet.campaign_id = $1),
            'sessions', (SELECT jsonb_agg(to_jsonb(session)
                                          ORDER BY session.session_id)
                           FROM core_domain.sessions AS session
                          WHERE session.campaign_id = $1),
            'scenes', (SELECT jsonb_agg(to_jsonb(scene) ORDER BY scene.scene_id)
                         FROM public.scenes AS scene
                        WHERE scene.campaign_id = $1),
            'public_events', (SELECT jsonb_agg(to_jsonb(public_event)
                                               ORDER BY public_event.source_event_sequence)
                                FROM public.campaign_fork_public_events AS public_event
                               WHERE public_event.campaign_id = $1),
            'clues', (SELECT jsonb_agg(to_jsonb(clue) ORDER BY clue.fork_clue_id)
                        FROM public.campaign_fork_clues AS clue
                       WHERE clue.campaign_id = $1),
            'npc_states', (SELECT jsonb_agg(to_jsonb(npc) ORDER BY npc.npc_state_id)
                             FROM public.campaign_fork_npc_states AS npc
                            WHERE npc.campaign_id = $1),
            'combat', (SELECT jsonb_agg(to_jsonb(combat) ORDER BY combat.combat_id)
                         FROM public.combat_states AS combat
                        WHERE combat.campaign_id = $1),
            'chase', (SELECT jsonb_agg(to_jsonb(chase) ORDER BY chase.chase_id)
                        FROM public.chase_states AS chase
                       WHERE chase.campaign_id = $1),
            'endings', (SELECT jsonb_agg(to_jsonb(ending)
                                         ORDER BY ending.ending_event_id)
                          FROM public.ending_events AS ending
                         WHERE ending.campaign_id = $1)
        )
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let child_events_before_replay: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CHILD_CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    let mut corrupt_child_projection = primary.begin().await.unwrap();
    for statement in [
        "ALTER TABLE public.campaign_fork_materializations DISABLE TRIGGER campaign_fork_materializations_event_guard",
        "ALTER TABLE public.campaign_fork_npc_states DISABLE TRIGGER campaign_fork_npc_states_event_guard",
        "ALTER TABLE public.scenarios DISABLE TRIGGER scenarios_event_guard",
        "ALTER TABLE public.characters DISABLE TRIGGER characters_event_guard",
        "ALTER TABLE public.character_sheet_versions DISABLE TRIGGER character_sheet_versions_event_guard",
        "ALTER TABLE core_domain.sessions DISABLE TRIGGER sessions_event_guard",
        "ALTER TABLE public.scenes DISABLE TRIGGER scenes_event_guard",
    ] {
        sqlx::query(statement)
            .execute(&mut *corrupt_child_projection)
            .await
            .unwrap();
    }
    for statement in [
        "UPDATE public.campaign_fork_materializations \
         SET provenance_reference = 'corrupted_child_manifest' \
         WHERE campaign_id = $1",
        "UPDATE public.scenarios \
         SET document_json = jsonb_set(document_json, '{corrupted}', 'true'::jsonb) \
         WHERE campaign_id = $1",
        "UPDATE public.characters \
         SET display_name = 'CORRUPTED_CHILD_CHARACTER_' || character_id \
         WHERE campaign_id = $1",
        "UPDATE public.character_sheet_versions \
         SET sheet_json = jsonb_set(sheet_json, '{corrupted}', 'true'::jsonb) \
         WHERE campaign_id = $1",
        "UPDATE core_domain.sessions \
         SET provenance_reference = 'corrupted_child_session' \
         WHERE campaign_id = $1",
        "UPDATE public.scenes \
         SET name = 'CORRUPTED_CHILD_SCENE' \
         WHERE campaign_id = $1",
    ] {
        sqlx::query(statement)
            .bind(CHILD_CAMPAIGN_ID)
            .execute(&mut *corrupt_child_projection)
            .await
            .unwrap();
    }
    sqlx::query(
        r#"
        INSERT INTO public.campaign_fork_npc_states (
            npc_state_id, fork_id, campaign_id, source_npc_id, state_json,
            version, visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        )
        SELECT 'npc_state_p08_ghost', fork_id, campaign_id, 'npc_p08_ghost',
               '{"kind":"GHOST"}'::JSONB, version, visibility_label,
               visibility_subject, provenance_kind, provenance_reference,
               provenance_recorded_by, last_event_sequence
          FROM public.campaign_fork_npc_states
         WHERE campaign_id = $1
         LIMIT 1
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .execute(&mut *corrupt_child_projection)
    .await
    .unwrap();
    sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(&mut *corrupt_child_projection)
        .await
        .unwrap();
    for statement in [
        "ALTER TABLE public.campaign_fork_materializations ENABLE TRIGGER campaign_fork_materializations_event_guard",
        "ALTER TABLE public.campaign_fork_npc_states ENABLE TRIGGER campaign_fork_npc_states_event_guard",
        "ALTER TABLE public.scenarios ENABLE TRIGGER scenarios_event_guard",
        "ALTER TABLE public.characters ENABLE TRIGGER characters_event_guard",
        "ALTER TABLE public.character_sheet_versions ENABLE TRIGGER character_sheet_versions_event_guard",
        "ALTER TABLE core_domain.sessions ENABLE TRIGGER sessions_event_guard",
        "ALTER TABLE public.scenes ENABLE TRIGGER scenes_event_guard",
    ] {
        sqlx::query(statement)
            .execute(&mut *corrupt_child_projection)
            .await
            .unwrap();
    }
    corrupt_child_projection.commit().await.unwrap();
    let repaired_child = api_repository
        .rebuild_p08_projections(CHILD_CAMPAIGN_ID)
        .await
        .expect("replace corrupt and ghost rows across the entire fork materialization");
    assert_eq!(repaired_child.replayed_events, child_event_types.len());
    let remaining_child_corruption: i64 = sqlx::query_scalar(
        r#"
        SELECT
            (SELECT count(*) FROM public.campaign_fork_materializations
              WHERE campaign_id = $1
                AND provenance_reference = 'corrupted_child_manifest')
          + (SELECT count(*) FROM public.campaign_fork_npc_states
              WHERE campaign_id = $1
                AND npc_state_id = 'npc_state_p08_ghost')
          + (SELECT count(*) FROM public.scenarios
              WHERE campaign_id = $1 AND document_json ? 'corrupted')
          + (SELECT count(*) FROM public.characters
              WHERE campaign_id = $1
                AND display_name LIKE 'CORRUPTED_CHILD_CHARACTER_%')
          + (SELECT count(*) FROM public.character_sheet_versions
              WHERE campaign_id = $1 AND sheet_json ? 'corrupted')
          + (SELECT count(*) FROM core_domain.sessions
              WHERE campaign_id = $1
                AND provenance_reference = 'corrupted_child_session')
          + (SELECT count(*) FROM public.scenes
              WHERE campaign_id = $1 AND name = 'CORRUPTED_CHILD_SCENE')
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        remaining_child_corruption, 0,
        "fork rebuild must clear retained corruption and ghost rows from every child-owned P08 projection"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store WHERE campaign_id = $1",
        )
        .bind(CHILD_CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        child_events_before_replay,
        "repairing a fork projection must not append canonical history"
    );
    let mut remove_child_projection = primary.begin().await.unwrap();
    sqlx::query("SET CONSTRAINTS ALL DEFERRED")
        .execute(&mut *remove_child_projection)
        .await
        .unwrap();
    for statement in [
        "DELETE FROM public.ending_events WHERE campaign_id = $1",
        "DELETE FROM public.gameplay_roll_consumptions WHERE campaign_id = $1",
        "DELETE FROM public.chase_states WHERE campaign_id = $1",
        "DELETE FROM public.combat_states WHERE campaign_id = $1",
        "DELETE FROM public.campaign_fork_npc_states WHERE campaign_id = $1",
        "DELETE FROM public.campaign_fork_clues WHERE campaign_id = $1",
        "DELETE FROM public.campaign_fork_public_events WHERE campaign_id = $1",
        "DELETE FROM public.campaign_fork_materializations WHERE campaign_id = $1",
        "DELETE FROM public.character_sheet_versions WHERE campaign_id = $1",
        "DELETE FROM public.characters WHERE campaign_id = $1",
        "DELETE FROM public.scenes WHERE campaign_id = $1",
        "DELETE FROM core_domain.sessions WHERE campaign_id = $1",
        "DELETE FROM public.scenarios WHERE campaign_id = $1",
        "DELETE FROM public.campaign_forks WHERE campaign_id = $1",
    ] {
        sqlx::query(statement)
            .bind(CHILD_CAMPAIGN_ID)
            .execute(&mut *remove_child_projection)
            .await
            .unwrap();
    }
    remove_child_projection.commit().await.unwrap();
    let rebuilt_child = api_repository
        .rebuild_p08_projections(CHILD_CAMPAIGN_ID)
        .await
        .expect("rebuild the entire child fork state solely from canonical P08 events");
    assert_eq!(rebuilt_child.replayed_events, child_event_types.len());
    assert_eq!(rebuilt_child.campaign_forks, 1);
    assert_eq!(rebuilt_child.fork_materializations, 1);
    assert_eq!(
        rebuilt_child.fork_public_events,
        snapshot_scope_len("public_events")
    );
    assert_eq!(
        rebuilt_child.fork_clues,
        snapshot_scope_len("discovered_clues")
    );
    assert_eq!(
        rebuilt_child.fork_npc_states,
        snapshot_scope_len("npc_state")
    );
    assert_eq!(
        rebuilt_child.combat_states,
        snapshot_scope_len("combat_state")
    );
    assert_eq!(
        rebuilt_child.chase_states,
        snapshot_scope_len("chase_state")
    );
    assert_eq!(
        rebuilt_child.ending_events,
        snapshot_scope_len("conclusion_state")
    );
    let child_projection_after: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'fork', (SELECT to_jsonb(fork) FROM public.campaign_forks AS fork
                      WHERE fork.fork_id = 'fork_p06_schema'),
            'manifest', (SELECT to_jsonb(manifest)
                           FROM public.campaign_fork_materializations AS manifest
                          WHERE manifest.fork_id = 'fork_p06_schema'),
            'scenario', (SELECT to_jsonb(scenario) FROM public.scenarios AS scenario
                          WHERE scenario.campaign_id = $1),
            'characters', (SELECT jsonb_agg(to_jsonb(character)
                                            ORDER BY character.character_id)
                             FROM public.characters AS character
                            WHERE character.campaign_id = $1),
            'sheets', (SELECT jsonb_agg(to_jsonb(sheet)
                                        ORDER BY sheet.sheet_version_id)
                         FROM public.character_sheet_versions AS sheet
                        WHERE sheet.campaign_id = $1),
            'sessions', (SELECT jsonb_agg(to_jsonb(session)
                                          ORDER BY session.session_id)
                           FROM core_domain.sessions AS session
                          WHERE session.campaign_id = $1),
            'scenes', (SELECT jsonb_agg(to_jsonb(scene) ORDER BY scene.scene_id)
                         FROM public.scenes AS scene
                        WHERE scene.campaign_id = $1),
            'public_events', (SELECT jsonb_agg(to_jsonb(public_event)
                                               ORDER BY public_event.source_event_sequence)
                                FROM public.campaign_fork_public_events AS public_event
                               WHERE public_event.campaign_id = $1),
            'clues', (SELECT jsonb_agg(to_jsonb(clue) ORDER BY clue.fork_clue_id)
                        FROM public.campaign_fork_clues AS clue
                       WHERE clue.campaign_id = $1),
            'npc_states', (SELECT jsonb_agg(to_jsonb(npc) ORDER BY npc.npc_state_id)
                             FROM public.campaign_fork_npc_states AS npc
                            WHERE npc.campaign_id = $1),
            'combat', (SELECT jsonb_agg(to_jsonb(combat) ORDER BY combat.combat_id)
                         FROM public.combat_states AS combat
                        WHERE combat.campaign_id = $1),
            'chase', (SELECT jsonb_agg(to_jsonb(chase) ORDER BY chase.chase_id)
                        FROM public.chase_states AS chase
                       WHERE chase.campaign_id = $1),
            'endings', (SELECT jsonb_agg(to_jsonb(ending)
                                         ORDER BY ending.ending_event_id)
                          FROM public.ending_events AS ending
                         WHERE ending.campaign_id = $1)
        )
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        child_projection_after, child_projection_before,
        "fork replay must reproduce byte-equivalent child read-model facts"
    );
    let child_events_after_replay: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CHILD_CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(
        child_events_after_replay, child_events_before_replay,
        "fork projection replay must never append or rewrite canonical history"
    );
    let source_after_fork = repository
        .preview_campaign_fork(CAMPAIGN_ID, "session_p06_schema", KEEPER_ID)
        .await
        .expect("recompute source snapshot after creating child");
    assert_eq!(
        source_after_fork, snapshot,
        "fork creation must not mutate the source campaign snapshot"
    );

    let post_fork_tutorial = parse_scenario_yaml(include_str!(
        "../../../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml"
    ))
    .expect("parse a scenario for post-fork child activity");
    include!("13_post_fork_independence.rs");
}
