{
    let post_fork_projection_before: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'scenario', (SELECT to_jsonb(scenario)
                           FROM public.scenarios AS scenario
                          WHERE scenario.scenario_id =
                                'scenario_p08_post_fork'),
            'character', (SELECT to_jsonb(character)
                            FROM public.characters AS character
                           WHERE character.character_id =
                                 'character_p08_post_fork'),
            'sheets', (
                SELECT jsonb_agg(to_jsonb(sheet) ORDER BY sheet.version)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.character_id = 'character_p08_post_fork'
            ),
            'session', (SELECT to_jsonb(session)
                          FROM core_domain.sessions AS session
                         WHERE session.session_id =
                               'session_p08_post_fork'),
            'scene', (SELECT to_jsonb(scene)
                        FROM public.scenes AS scene
                       WHERE scene.scene_id = 'scene_p08_post_fork'),
            'ending', (SELECT to_jsonb(ending)
                         FROM public.ending_events AS ending
                        WHERE ending.ending_event_id =
                              'ending_p08_post_fork'),
            'growth', (SELECT to_jsonb(growth)
                         FROM public.growth_events AS growth
                        WHERE growth.growth_event_id =
                              'growth_p08_post_fork'),
            'roll_consumptions', (
                SELECT jsonb_agg(to_jsonb(consumption)
                                 ORDER BY consumption.roll_id)
                  FROM public.gameplay_roll_consumptions AS consumption
                 WHERE consumption.campaign_id = $1
                   AND consumption.aggregate_id = 'growth_p08_post_fork'
            )
        )
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let child_events_before_fork_retry: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CHILD_CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    repository
        .record_campaign_fork(
            &metadata(
                CHILD_CAMPAIGN_ID,
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "fork_p06_schema",
                "campaign_fork",
                "campaign.fork.record",
                0,
                "fork_record",
                "keeper_only",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordCampaignForkRequest {
                fork_id: "fork_p06_schema".to_owned(),
                parent_campaign_id: CAMPAIGN_ID.to_owned(),
                child_campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                source_session_id: "session_p06_schema".to_owned(),
                snapshot_hash: snapshot.snapshot_hash.clone(),
                reason: "Preserve an alternate ruling".to_owned(),
                copy_scopes: snapshot.copy_scopes.clone(),
            },
        )
        .await
        .expect("an exact fork retry must ignore unrelated post-fork child rows");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store WHERE campaign_id = $1",
        )
        .bind(CHILD_CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        child_events_before_fork_retry,
        "an exact fork retry after child activity must not append canonical history"
    );
    api_repository
        .rebuild_p08_projections(CHILD_CAMPAIGN_ID)
        .await
        .expect("rebuild only fork-owned P08 rows after normal child activity");
    let copied_fork_character_after: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'character', (
                SELECT to_jsonb(character)
                  FROM public.characters AS character
                 WHERE character.campaign_id = $1
                   AND character.character_id = $2
            ),
            'sheets', (
                SELECT jsonb_agg(to_jsonb(sheet) ORDER BY sheet.version)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.campaign_id = $1
                   AND sheet.character_id = $2
            ),
            'action', (
                SELECT to_jsonb(action)
                  FROM public.player_actions AS action
                 WHERE action.campaign_id = $1
                   AND action.action_id =
                       'action_p08_copied_character_sanity'
            ),
            'sanity', (
                SELECT to_jsonb(sanity)
                  FROM public.sanity_events AS sanity
                 WHERE sanity.campaign_id = $1
                   AND sanity.sanity_event_id =
                       'sanity_p08_copied_character'
            )
        )
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .bind(&copied_late_joiner_character_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        copied_fork_character_after, copied_fork_character_before,
        "a P08 rebuild must preserve the canonical SAN suffix of a copied character"
    );
    let post_fork_projection_after: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'scenario', (SELECT to_jsonb(scenario)
                           FROM public.scenarios AS scenario
                          WHERE scenario.scenario_id =
                                'scenario_p08_post_fork'),
            'character', (SELECT to_jsonb(character)
                            FROM public.characters AS character
                           WHERE character.character_id =
                                 'character_p08_post_fork'),
            'sheets', (
                SELECT jsonb_agg(to_jsonb(sheet) ORDER BY sheet.version)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.character_id = 'character_p08_post_fork'
            ),
            'session', (SELECT to_jsonb(session)
                          FROM core_domain.sessions AS session
                         WHERE session.session_id =
                               'session_p08_post_fork'),
            'scene', (SELECT to_jsonb(scene)
                        FROM public.scenes AS scene
                       WHERE scene.scene_id = 'scene_p08_post_fork'),
            'ending', (SELECT to_jsonb(ending)
                         FROM public.ending_events AS ending
                        WHERE ending.ending_event_id =
                              'ending_p08_post_fork'),
            'growth', (SELECT to_jsonb(growth)
                         FROM public.growth_events AS growth
                        WHERE growth.growth_event_id =
                              'growth_p08_post_fork'),
            'roll_consumptions', (
                SELECT jsonb_agg(to_jsonb(consumption)
                                 ORDER BY consumption.roll_id)
                  FROM public.gameplay_roll_consumptions AS consumption
                 WHERE consumption.campaign_id = $1
                   AND consumption.aggregate_id = 'growth_p08_post_fork'
            )
        )
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        post_fork_projection_after, post_fork_projection_before,
        "a P08 rebuild must preserve later scenario, character, sheet, session and scene projections"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store WHERE campaign_id = $1",
        )
        .bind(CHILD_CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        child_events_before_fork_retry,
        "preserving post-fork projections must not append or rewrite canonical history"
    );

    create_campaign(
        &repository,
        RACE_CHILD_CAMPAIGN_ID,
        RACE_CHILD_AUTHORITY_ID,
        "room_p08_fork_race_child",
        "fork_race_child_create",
    )
    .await;
    let race_metadata_a = metadata(
        RACE_CHILD_CAMPAIGN_ID,
        RACE_CHILD_AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "fork_p08_race_a",
        "campaign_fork",
        "campaign.fork.record",
        0,
        "fork_race_a",
        "keeper_only",
        "not_applicable",
        "human_keeper_statement",
    );
    let race_metadata_b = metadata(
        RACE_CHILD_CAMPAIGN_ID,
        RACE_CHILD_AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "fork_p08_race_b",
        "campaign_fork",
        "campaign.fork.record",
        0,
        "fork_race_b",
        "keeper_only",
        "not_applicable",
        "human_keeper_statement",
    );
    let race_request_a = RecordCampaignForkRequest {
        fork_id: "fork_p08_race_a".to_owned(),
        parent_campaign_id: CAMPAIGN_ID.to_owned(),
        child_campaign_id: RACE_CHILD_CAMPAIGN_ID.to_owned(),
        source_session_id: "session_p06_schema".to_owned(),
        snapshot_hash: snapshot.snapshot_hash.clone(),
        reason: "First concurrent lineage candidate".to_owned(),
        copy_scopes: snapshot.copy_scopes.clone(),
    };
    let race_request_b = RecordCampaignForkRequest {
        fork_id: "fork_p08_race_b".to_owned(),
        parent_campaign_id: CAMPAIGN_ID.to_owned(),
        child_campaign_id: RACE_CHILD_CAMPAIGN_ID.to_owned(),
        source_session_id: "session_p06_schema".to_owned(),
        snapshot_hash: snapshot.snapshot_hash.clone(),
        reason: "Second concurrent lineage candidate".to_owned(),
        copy_scopes: snapshot.copy_scopes.clone(),
    };
    let (race_a, race_b) = tokio::join!(
        repository.record_campaign_fork(&race_metadata_a, &race_request_a),
        repository.record_campaign_fork(&race_metadata_b, &race_request_b),
    );
    assert_eq!(
        usize::from(race_a.is_ok()) + usize::from(race_b.is_ok()),
        1,
        "the child-scoped lock must allow exactly one concurrent fork lineage"
    );
    let rejected_race = if race_a.is_err() { race_a } else { race_b };
    assert!(
        matches!(
            &rejected_race,
            Err(CoreDomainRepositoryError::Integrity(
                "campaign_fork_child_lineage_conflict"
            ))
        ),
        "the losing fork must be rejected against canonical child lineage: {rejected_race:?}"
    );
    let race_lineage_counts: (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            (SELECT count(*) FROM public.campaign_forks
              WHERE child_campaign_id = $1),
            (SELECT count(*) FROM public.event_store
              WHERE campaign_id = $1 AND event_type = 'CampaignForkRecorded'),
            (SELECT count(*) FROM pg_constraint
              WHERE conname = 'campaign_forks_child_lineage_unique'
                AND conrelid = 'public.campaign_forks'::regclass)
        "#,
    )
    .bind(RACE_CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        race_lineage_counts,
        (1, 1, 1),
        "one child must have one projected lineage, one canonical lineage event, and one DB constraint"
    );

    create_campaign(
        &repository,
        STATE_RACE_CHILD_CAMPAIGN_ID,
        STATE_RACE_CHILD_AUTHORITY_ID,
        "room_p08_fork_state_race_child",
        "fork_state_race_child_create",
    )
    .await;
    let state_race_tutorial = parse_scenario_yaml(include_str!(
        "../../../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml"
    ))
    .expect("parse scenario for fork-versus-state race");
    let state_write_repository = repository.clone();
    let state_write_metadata = metadata(
        STATE_RACE_CHILD_CAMPAIGN_ID,
        STATE_RACE_CHILD_AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "scenario_p08_fork_state_race",
        "scenario",
        "scenario.import",
        0,
        "scenario_p08_fork_state_race_import",
        "keeper_only",
        "not_applicable",
        "imported_source",
    );
    let state_write_request = ImportScenarioRequest {
        scenario_id: "scenario_p08_fork_state_race".to_owned(),
        campaign_id: STATE_RACE_CHILD_CAMPAIGN_ID.to_owned(),
        ruleset_id: state_race_tutorial.ruleset_id,
        format_version: state_race_tutorial.format_version,
        content_hash: state_race_tutorial.content_hash,
        document_json: state_race_tutorial.canonical_json,
    };
    let state_fork_repository = repository.clone();
    include!("15_fork_state_race.rs");
}
