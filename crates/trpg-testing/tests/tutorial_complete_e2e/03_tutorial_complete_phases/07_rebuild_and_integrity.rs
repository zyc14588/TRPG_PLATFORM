{
    assert_eq!(
        (
            child_counts.6,
            child_counts.7,
            child_counts.8,
            child_counts.9
        ),
        (1, 1, 1, 1),
        "clue, combat, chase and conclusion copy scopes must be queryable in the child"
    );
    let child_visibility = sqlx::query(
        r#"
        SELECT
          (SELECT visibility_label::TEXT FROM public.scenarios
            WHERE campaign_id = $1) AS scenario_visibility,
          (SELECT visibility_label::TEXT FROM public.characters
            WHERE campaign_id = $1) AS character_visibility,
          (SELECT visibility_subject FROM public.characters
            WHERE campaign_id = $1) AS character_subject,
          (SELECT visibility_label::TEXT
             FROM public.character_sheet_versions
            WHERE campaign_id = $1) AS sheet_visibility,
          (SELECT visibility_subject
             FROM public.character_sheet_versions
            WHERE campaign_id = $1) AS sheet_subject,
          (SELECT visibility_label::TEXT FROM core_domain.sessions
            WHERE campaign_id = $1) AS session_visibility,
          (SELECT sheet_json -> 'skills' ->> 'Library Use'
             FROM public.character_sheet_versions
            WHERE campaign_id = $1) AS fork_growth_skill,
          (SELECT sheet_json #>> '{sanity_state,current_sanity}'
             FROM public.character_sheet_versions
            WHERE campaign_id = $1) AS fork_sanity,
          (SELECT sheet_json -> 'skills' ->> 'Library Use'
             FROM public.character_sheet_versions
            WHERE sheet_version_id = 'sheet_p08_evelyn_v4')
              AS current_parent_growth_skill
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("load fork visibility and cutoff state");
    assert_eq!(
        child_visibility.get::<String, _>("scenario_visibility"),
        "keeper_only"
    );
    assert_eq!(
        child_visibility.get::<String, _>("character_visibility"),
        "private_to_player"
    );
    assert_eq!(
        child_visibility.get::<String, _>("character_subject"),
        PLAYER_ID
    );
    assert_eq!(
        child_visibility.get::<String, _>("sheet_visibility"),
        "private_to_player"
    );
    assert_eq!(
        child_visibility.get::<String, _>("sheet_subject"),
        PLAYER_ID
    );
    assert_eq!(
        child_visibility.get::<String, _>("session_visibility"),
        "party_visible"
    );
    assert_eq!(
        child_visibility
            .get::<String, _>("fork_growth_skill")
            .parse::<u8>()
            .unwrap(),
        growth_after
    );
    assert_eq!(
        child_visibility
            .get::<String, _>("fork_sanity")
            .parse::<u8>()
            .unwrap(),
        65 - sanity_loss,
        "fork materialization must persist the selected-session SAN result"
    );
    assert_eq!(
        child_visibility
            .get::<String, _>("current_parent_growth_skill")
            .parse::<u8>()
            .unwrap(),
        later_growth_after
    );
    let child_growth_source = sqlx::query(
        r#"
        SELECT child_session.session_id,
               child_ending.ending_event_id,
               child_character.character_id,
               child_sheet.sheet_version_id,
               child_sheet.sheet_json,
               child_scenario.document_json
          FROM core_domain.sessions AS child_session
          JOIN public.scenarios AS child_scenario
            ON child_scenario.scenario_id = child_session.scenario_id
           AND child_scenario.campaign_id = child_session.campaign_id
          JOIN public.ending_events AS child_ending
            ON child_ending.session_id = child_session.session_id
           AND child_ending.campaign_id = child_session.campaign_id
          JOIN public.characters AS child_character
            ON child_character.campaign_id = child_session.campaign_id
          JOIN public.character_sheet_versions AS child_sheet
            ON child_sheet.character_id = child_character.character_id
           AND child_sheet.campaign_id = child_character.campaign_id
           AND child_sheet.version = child_character.current_sheet_version
         WHERE child_session.campaign_id = $1
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("load the child-owned conclusion and current character sheet");
    let child_scenario_document: serde_json::Value = child_growth_source.get("document_json");
    let child_awarded_skills = child_scenario_document
        .pointer("/endings/0/growth_awards")
        .and_then(serde_json::Value::as_array)
        .expect("materialized child scenario growth awards")
        .iter()
        .filter_map(|award| award.get("skill_name").and_then(serde_json::Value::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        child_awarded_skills,
        BTreeSet::from(["Library Use", "Psychology"]),
        "the child scenario must retain the source ending's complete award authorization"
    );
    let child_sheet_json: serde_json::Value = child_growth_source.get("sheet_json");
    let child_character_id: String = child_growth_source.get("character_id");
    assert_eq!(
        child_scenario_document
            .pointer("/endings/0/growth_awards")
            .and_then(serde_json::Value::as_array)
            .and_then(|awards| {
                awards.iter().find(|award| {
                    award.get("skill_name").and_then(serde_json::Value::as_str)
                        == Some("Library Use")
                })
            })
            .and_then(|award| award.get("consumed_by_character_ids"))
            .and_then(serde_json::Value::as_array)
            .expect("materialized consumed Library Use marker"),
        &[serde_json::Value::String(child_character_id.clone())],
        "the consumed source award must be rebound to the child-owned character ID"
    );
    let child_library_before = child_sheet_json
        .pointer("/skills/Library Use")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .expect("forked Library Use skill");
    let duplicate_child_library_roll = server_roll_skill_growth(child_library_before)
        .expect("server-owned duplicate child growth evidence");
    let child_growth_events_before_duplicate: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'CharacterGrowthApplied'",
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(matches!(
        repository
            .record_growth(
                &metadata(
                    CHILD_AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "growth_event_p08_child_duplicate_library",
                    "growth",
                    0,
                    "p08_child_duplicate_library",
                    "private_to_player",
                    PLAYER_ID,
                    "rules_engine_decision",
                ),
                &RecordGrowthRequest {
                    growth_event_id: "growth_event_p08_child_duplicate_library".to_owned(),
                    campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                    session_id: child_growth_source.get("session_id"),
                    ending_event_id: child_growth_source.get("ending_event_id"),
                    character_id: child_character_id.clone(),
                    source_sheet_version_id: child_growth_source.get("sheet_version_id"),
                    new_sheet_version_id: "sheet_p08_child_duplicate_library".to_owned(),
                    skill_name: "Library Use".to_owned(),
                    growth_rolls: duplicate_child_library_roll.evidence().clone(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Integrity(
            "growth_skill_already_recorded"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND event_type = 'CharacterGrowthApplied'",
        )
        .bind(CHILD_CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        child_growth_events_before_duplicate,
        "a consumed source award must be rejected before child canonical append"
    );
    let child_psychology_before = child_sheet_json
        .pointer("/skills/Psychology")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .expect("forked Psychology skill");
    let child_growth_roll = server_roll_skill_growth(child_psychology_before)
        .expect("server-owned child fork growth evidence");
    let child_psychology_after = child_growth_roll.outcome().skill_after;
    repository
        .record_growth(
            &metadata(
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "growth_event_p08_child_psychology",
                "growth",
                0,
                "p08_child_growth_after_fork",
                "private_to_player",
                PLAYER_ID,
                "rules_engine_decision",
            ),
            &RecordGrowthRequest {
                growth_event_id: "growth_event_p08_child_psychology".to_owned(),
                campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                session_id: child_growth_source.get("session_id"),
                ending_event_id: child_growth_source.get("ending_event_id"),
                character_id: child_growth_source.get("character_id"),
                source_sheet_version_id: child_growth_source.get("sheet_version_id"),
                new_sheet_version_id: "sheet_p08_child_psychology_growth".to_owned(),
                skill_name: "Psychology".to_owned(),
                growth_rolls: child_growth_roll.evidence().clone(),
            },
        )
        .await
        .expect("settle an unconsumed ending award entirely inside the forked campaign");
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT sheet_json -> 'skills' ->> 'Psychology' \
             FROM public.character_sheet_versions \
             WHERE sheet_version_id = 'sheet_p08_child_psychology_growth'",
        )
        .fetch_one(&primary)
        .await
        .expect("load the child growth result")
        .parse::<u8>()
        .unwrap(),
        child_psychology_after,
        "a fork created before this award is settled must remain growth-capable"
    );
    let materialized_visibility: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT DISTINCT visibility_label, visibility_subject, data_subject_id
          FROM public.event_store
         WHERE campaign_id = $1
           AND event_type = 'CampaignForkMaterialized'
         ORDER BY visibility_label, visibility_subject, data_subject_id
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_all(&primary)
    .await
    .expect("load per-event fork visibility");
    assert_eq!(
        materialized_visibility,
        vec![
            (
                "keeper_only".to_owned(),
                "not_applicable".to_owned(),
                "not_applicable".to_owned()
            ),
            (
                "party_visible".to_owned(),
                "not_applicable".to_owned(),
                "not_applicable".to_owned()
            ),
            (
                "private_to_player".to_owned(),
                PLAYER_ID.to_owned(),
                PLAYER_ID.to_owned()
            )
        ]
    );
    let private_fork_crypto_binding: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
          count(*) FILTER (
            WHERE event.visibility_label = 'private_to_player'
          ),
          count(*) FILTER (
            WHERE event.visibility_label = 'private_to_player'
              AND event.data_subject_id = event.visibility_subject
              AND subject_key.subject_id = event.data_subject_id
              AND subject_key.key_reference = event.payload_key_reference
              AND subject_key.wrapped_key IS NOT NULL
              AND subject_key.destroyed_at IS NULL
          )
          FROM public.event_store AS event
          LEFT JOIN public.privacy_subject_keys AS subject_key
            ON subject_key.subject_id = event.data_subject_id
         WHERE event.campaign_id = $1
           AND event.event_type = 'CampaignForkMaterialized'
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("verify private fork payload crypto binding");
    assert!(private_fork_crypto_binding.0 > 0);
    assert_eq!(
        private_fork_crypto_binding.1, private_fork_crypto_binding.0,
        "every owner-private fork payload must use that player's live subject key"
    );

    let actual_event_types = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT event_type FROM public.event_store ORDER BY event_type",
    )
    .fetch_all(&primary)
    .await
    .expect("read the actual canonical event types")
    .into_iter()
    .collect::<BTreeSet<_>>();
    include!("08_event_coverage_and_witness.rs");
}
