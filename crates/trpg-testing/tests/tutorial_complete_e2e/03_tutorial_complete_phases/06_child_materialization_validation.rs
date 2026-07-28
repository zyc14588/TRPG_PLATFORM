{
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store WHERE event_type = 'EndingRecorded'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        ending_count_before_race + 1,
        "the losing concurrent ending must not append canonical history"
    );
    let concurrent_ending_event_id: String = sqlx::query_scalar(
        "SELECT ending_event_id FROM public.ending_events \
         WHERE session_id = 'session_p08_concurrency'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();

    let growth_count_before_race: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'CharacterGrowthApplied'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let concurrent_library_roll =
        server_roll_skill_growth(later_growth_after).expect("concurrent Library Use rolls");
    let concurrent_psychology_roll =
        server_roll_skill_growth(55).expect("concurrent Psychology rolls");
    let growth_race_metadata_a = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "growth_event_p08_race_library",
        "growth",
        0,
        "p08_growth_race_library",
        "private_to_player",
        PLAYER_ID,
        "rules_engine_decision",
    );
    let growth_race_metadata_b = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "growth_event_p08_race_psychology",
        "growth",
        0,
        "p08_growth_race_psychology",
        "private_to_player",
        PLAYER_ID,
        "rules_engine_decision",
    );
    let growth_race_request_a = RecordGrowthRequest {
        growth_event_id: "growth_event_p08_race_library".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: "session_p08_concurrency".to_owned(),
        ending_event_id: concurrent_ending_event_id.clone(),
        character_id: CHARACTER_ID.to_owned(),
        source_sheet_version_id: "sheet_p08_evelyn_v4".to_owned(),
        new_sheet_version_id: "sheet_p08_evelyn_v5_library".to_owned(),
        skill_name: "Library Use".to_owned(),
        growth_rolls: concurrent_library_roll.evidence().clone(),
    };
    let growth_race_request_b = RecordGrowthRequest {
        growth_event_id: "growth_event_p08_race_psychology".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: "session_p08_concurrency".to_owned(),
        ending_event_id: concurrent_ending_event_id,
        character_id: CHARACTER_ID.to_owned(),
        source_sheet_version_id: "sheet_p08_evelyn_v4".to_owned(),
        new_sheet_version_id: "sheet_p08_evelyn_v5_psychology".to_owned(),
        skill_name: "Psychology".to_owned(),
        growth_rolls: concurrent_psychology_roll.evidence().clone(),
    };
    let (growth_race_a, growth_race_b) = tokio::join!(
        repository.record_growth(&growth_race_metadata_a, &growth_race_request_a),
        repository.record_growth(&growth_race_metadata_b, &growth_race_request_b),
    );
    assert_eq!(
        usize::from(growth_race_a.is_ok()) + usize::from(growth_race_b.is_ok()),
        1,
        "the character advisory lock must serialize growth from one source sheet"
    );
    let growth_race_failure = if growth_race_a.is_err() {
        &growth_race_a
    } else {
        &growth_race_b
    };
    assert!(matches!(
        growth_race_failure,
        Err(CoreDomainRepositoryError::NotFound("growth_source"))
            | Err(CoreDomainRepositoryError::Integrity(
                "growth_source_not_current"
            ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE event_type = 'CharacterGrowthApplied'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        growth_count_before_race + 1,
        "the losing concurrent growth must not append canonical history"
    );

    create_campaign(
        &repository,
        CHILD_CAMPAIGN_ID,
        CHILD_AUTHORITY_ID,
        "room_p08_tutorial_fork",
        "p08_child_campaign_create",
    )
    .await;
    let snapshot = repository
        .preview_campaign_fork(CAMPAIGN_ID, SESSION_ID, KEEPER_ID)
        .await
        .expect("compute the canonical public fork snapshot");
    assert!(
        snapshot
            .canonical_snapshot_json
            .contains("ReconsiderationCorrected"),
        "the fork must include a completed correction chain for source-session history"
    );
    assert!(
        snapshot
            .canonical_snapshot_json
            .contains("reconsideration_p08_after_later")
            && snapshot
                .canonical_snapshot_json
                .contains("ReconsiderationUpheld"),
        "a relevant late reconsideration chain must be included separately"
    );
    assert!(
        !snapshot
            .canonical_snapshot_json
            .contains("session_p08_later")
            && !snapshot
                .canonical_snapshot_json
                .contains("ending_event_p08_later"),
        "a late reconsideration must not widen the base cutoff to unrelated later-session events"
    );
    assert!(!snapshot.canonical_snapshot_json.contains("keeper_note"));
    assert!(!snapshot.canonical_snapshot_json.contains("private_message"));
    assert!(!snapshot.canonical_snapshot_json.contains("ai_internal"));
    let snapshot_json: serde_json::Value =
        serde_json::from_str(&snapshot.canonical_snapshot_json).unwrap();
    let fork_characters = snapshot_json
        .pointer("/state/character_state")
        .and_then(serde_json::Value::as_array)
        .expect("fork snapshot characters");
    let fork_growth_awards = snapshot_json
        .pointer("/state/conclusion_state/0/growth_awards")
        .and_then(serde_json::Value::as_array)
        .expect("fork snapshot must carry the source ending's growth awards");
    assert_eq!(
        fork_growth_awards
            .iter()
            .filter_map(|award| { award.get("skill_name").and_then(serde_json::Value::as_str) })
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["Library Use", "Psychology"]),
        "growth authorization must be content-addressed into the fork snapshot"
    );
    assert_eq!(
        snapshot_json
            .pointer("/state/conclusion_state/0/consumed_growth_awards")
            .and_then(serde_json::Value::as_array)
            .expect("fork snapshot consumed Growth markers")
            .iter()
            .filter_map(|consumed| {
                Some((
                    consumed.get("character_id")?.as_str()?,
                    consumed.get("skill_name")?.as_str()?,
                ))
            })
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([(CHARACTER_ID, "Library Use")]),
        "the source snapshot must bind settled awards to the source character"
    );
    assert_eq!(
        fork_characters.len(),
        1,
        "an investigator updated after the cutoff must be replayed, not omitted"
    );
    assert_eq!(
        fork_characters[0]
            .pointer("/current_sheet/sheet_json/skills/Library Use")
            .and_then(serde_json::Value::as_u64),
        Some(u64::from(growth_after)),
        "the source-session fork must retain the sheet as of its canonical cutoff"
    );
    assert_eq!(
        fork_characters[0]
            .pointer("/current_sheet/sheet_json/sanity_state/current_sanity")
            .and_then(serde_json::Value::as_u64),
        Some(u64::from(65 - sanity_loss)),
        "the selected-session flat PlayerActionSubmitted payload and its dependent SAN loss must survive fork replay"
    );
    repository
        .record_campaign_fork(
            &metadata(
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "fork_p08_tutorial",
                "campaign_fork",
                0,
                "p08_fork",
                "keeper_only",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordCampaignForkRequest {
                fork_id: "fork_p08_tutorial".to_owned(),
                parent_campaign_id: CAMPAIGN_ID.to_owned(),
                child_campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                source_session_id: SESSION_ID.to_owned(),
                snapshot_hash: snapshot.snapshot_hash.clone(),
                reason: "Preserve the corrected public branch".to_owned(),
                copy_scopes: snapshot.copy_scopes.clone(),
            },
        )
        .await
        .expect("materialize the fork as child-owned durable state");
    let source_after_fork = repository
        .preview_campaign_fork(CAMPAIGN_ID, SESSION_ID, KEEPER_ID)
        .await
        .expect("re-read the source after child materialization");
    assert_eq!(
        source_after_fork, snapshot,
        "fork materialization must not mutate the source Campaign"
    );

    let parent_projection = sqlx::query(
        r#"
        SELECT
          (SELECT status FROM public.combat_states
            WHERE combat_id = 'combat_p08_tutorial') AS combat_status,
          (SELECT state_json -> 'participants' -> 0 ->> 'condition'
             FROM public.combat_states
            WHERE combat_id = 'combat_p08_tutorial') AS combat_condition,
          (SELECT status FROM public.chase_states
            WHERE chase_id = 'chase_p08_tutorial') AS chase_status,
          (SELECT state FROM public.reconsiderations
            WHERE reconsideration_id = 'reconsideration_p08_tutorial')
              AS reconsideration_state,
          (SELECT outcome FROM public.reconsiderations
            WHERE reconsideration_id = 'reconsideration_p08_tutorial')
              AS reconsideration_outcome,
          (SELECT sheet_json -> 'skills' ->> 'Library Use'
             FROM public.character_sheet_versions
            WHERE sheet_version_id = 'sheet_p08_evelyn_v3') AS growth_skill,
          (SELECT random_source FROM public.growth_events
            WHERE growth_event_id = 'growth_event_p08_tutorial')
              AS growth_random_source,
          (SELECT server_roll_id FROM public.growth_events
            WHERE growth_event_id = 'growth_event_p08_tutorial')
              AS growth_server_roll_id,
          (SELECT increase_roll_id FROM public.growth_events
            WHERE growth_event_id = 'growth_event_p08_tutorial')
              AS growth_increase_roll_id
        "#,
    )
    .fetch_one(&primary)
    .await
    .expect("load the completed Tutorial read models");
    assert_eq!(parent_projection.get::<String, _>("combat_status"), "ENDED");
    assert_eq!(
        parent_projection.get::<String, _>("combat_condition"),
        "MAJOR_WOUND"
    );
    assert_eq!(parent_projection.get::<String, _>("chase_status"), "CAUGHT");
    assert_eq!(
        parent_projection.get::<String, _>("reconsideration_state"),
        "RESOLVED"
    );
    assert_eq!(
        parent_projection.get::<String, _>("reconsideration_outcome"),
        "CORRECTED"
    );
    assert_eq!(
        parent_projection
            .get::<String, _>("growth_skill")
            .parse::<u8>()
            .unwrap(),
        growth_after
    );
    assert_eq!(
        parent_projection.get::<String, _>("growth_random_source"),
        "SERVER_OS_CSPRNG"
    );
    assert_eq!(
        parent_projection.get::<String, _>("growth_server_roll_id"),
        growth_roll.evidence().improvement_check().roll_id()
    );
    assert_eq!(
        parent_projection
            .get::<Option<String>, _>("growth_increase_roll_id")
            .as_deref(),
        growth_roll.evidence().increase().map(|roll| roll.roll_id())
    );

    let child_counts: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
          (SELECT count(*) FROM public.scenarios WHERE campaign_id = $1),
          (SELECT count(*) FROM public.characters WHERE campaign_id = $1),
          (SELECT count(*) FROM core_domain.sessions WHERE campaign_id = $1),
          (SELECT count(*) FROM public.scenes WHERE campaign_id = $1),
          (SELECT count(*) FROM public.campaign_fork_materializations
            WHERE campaign_id = $1),
          (SELECT count(*) FROM public.campaign_fork_public_events
            WHERE campaign_id = $1),
          (SELECT count(*) FROM public.campaign_fork_clues
            WHERE campaign_id = $1),
          (SELECT count(*) FROM public.combat_states WHERE campaign_id = $1),
          (SELECT count(*) FROM public.chase_states WHERE campaign_id = $1),
          (SELECT count(*) FROM public.ending_events WHERE campaign_id = $1)
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("load child fork materialization");
    assert_eq!(
        (
            child_counts.0,
            child_counts.1,
            child_counts.2,
            child_counts.3,
            child_counts.4
        ),
        (1, 1, 1, 2, 1),
        "fork must materialize real child-owned scenario, character, session, scenes and manifest"
    );
    assert!(child_counts.5 > 0);
    include!("07_rebuild_and_integrity.rs");
}
