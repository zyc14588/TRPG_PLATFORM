{
    assert!(matches!(
        repository
            .record_growth(
                &metadata(
                    CAMPAIGN_ID,
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "growth_event_p08_reused_combat_roll",
                    "growth",
                    "growth.record",
                    0,
                    "growth_p08_reused_combat_roll",
                    "private_to_player",
                    PLAYER_ID,
                    "rules_engine_decision",
                ),
                &RecordGrowthRequest {
                    growth_event_id: "growth_event_p08_reused_combat_roll".to_owned(),
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p06_schema".to_owned(),
                    ending_event_id: "ending_event_p08_schema".to_owned(),
                    character_id: "character_p06_player".to_owned(),
                    source_sheet_version_id: combat_health_source.0.clone(),
                    new_sheet_version_id: "sheet_p06_player_v2_reused".to_owned(),
                    skill_name: "Library Use".to_owned(),
                    growth_rolls: growth_roll_reused_by_combat.evidence().clone(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "gameplay_roll_reuse"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND event_type = 'CharacterGrowthApplied'",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        growth_events_before_reuse,
        "a Combat roll reused by Growth must fail before canonical append"
    );
    for (visibility_label, event_id, sheet_id, suffix) in [
        (
            "public",
            "growth_event_p08_public_widening",
            "sheet_p06_player_v2_public_widening",
            "growth_p08_public_widening",
        ),
        (
            "party_visible",
            "growth_event_p08_party_widening",
            "sheet_p06_player_v2_party_widening",
            "growth_p08_party_widening",
        ),
    ] {
        assert!(
            matches!(
                repository
                    .record_growth(
                        &metadata(
                            CAMPAIGN_ID,
                            AUTHORITY_ID,
                            KEEPER_ID,
                            "human_keeper",
                            event_id,
                            "growth",
                            "growth.record",
                            0,
                            suffix,
                            visibility_label,
                            "not_applicable",
                            "rules_engine_decision",
                        ),
                        &RecordGrowthRequest {
                            growth_event_id: event_id.to_owned(),
                            campaign_id: CAMPAIGN_ID.to_owned(),
                            session_id: "session_p06_schema".to_owned(),
                            ending_event_id: "ending_event_p08_schema".to_owned(),
                            character_id: "character_p06_player".to_owned(),
                            source_sheet_version_id: combat_health_source.0.clone(),
                            new_sheet_version_id: sheet_id.to_owned(),
                            skill_name: "Library Use".to_owned(),
                            growth_rolls: growth_roll.evidence().clone(),
                        },
                    )
                    .await,
                Err(CoreDomainRepositoryError::PolicyEvidenceMismatch)
            ),
            "{visibility_label} must not widen an owner-private Growth source"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND event_type = 'CharacterGrowthApplied'",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        growth_events_before_reuse,
        "visibility widening must fail before canonical append"
    );
    let source_visibility: (String, String, String, String, i64) = sqlx::query_as(
        r#"
        SELECT character.visibility_label::TEXT,
               character.visibility_subject,
               sheet.visibility_label::TEXT,
               sheet.visibility_subject,
               character.current_sheet_version
          FROM public.characters AS character
          JOIN public.character_sheet_versions AS sheet
            ON sheet.character_id = character.character_id
           AND sheet.version = character.current_sheet_version
         WHERE character.character_id = 'character_p06_player'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        source_visibility,
        (
            "private_to_player".to_owned(),
            PLAYER_ID.to_owned(),
            "private_to_player".to_owned(),
            PLAYER_ID.to_owned(),
            combat_health_source.1,
        ),
        "a rejected Growth command must preserve the private source envelope and current sheet"
    );
    repository
        .record_growth(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "growth_event_p08_schema",
                "growth",
                "growth.record",
                0,
                "growth_p08_record",
                "private_to_player",
                PLAYER_ID,
                "rules_engine_decision",
            ),
            &RecordGrowthRequest {
                growth_event_id: "growth_event_p08_schema".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                ending_event_id: "ending_event_p08_schema".to_owned(),
                character_id: "character_p06_player".to_owned(),
                source_sheet_version_id: combat_health_source.0.clone(),
                new_sheet_version_id: "sheet_p06_player_v2".to_owned(),
                skill_name: "Library Use".to_owned(),
                growth_rolls: growth_roll.evidence().clone(),
            },
        )
        .await
        .expect("apply server-generated tutorial growth to a new sheet version");
    let persisted_combat: (String, i64, serde_json::Value) = sqlx::query_as(
        "SELECT status, version, state_json FROM public.combat_states \
         WHERE combat_id = 'combat_p08_schema'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(persisted_combat.0, "ENDED");
    assert_eq!(persisted_combat.1, 14);
    assert_eq!(
        persisted_combat
            .2
            .pointer("/participants/0/condition")
            .and_then(serde_json::Value::as_str),
        Some("ABLE"),
        "only the current healer's successful persisted First Aid roll clears MajorWound"
    );
    let persisted_chase: (String, i64) = sqlx::query_as(
        "SELECT status, version FROM public.chase_states \
         WHERE chase_id = 'chase_p08_schema'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(persisted_chase, ("CAUGHT".to_owned(), 2));
    let persisted_growth: (
        i64,
        serde_json::Value,
        String,
        String,
        Option<String>,
        Option<i16>,
    ) = sqlx::query_as(
        r#"
        SELECT character.current_sheet_version, sheet.sheet_json,
               growth.random_source, growth.server_roll_id,
               growth.increase_roll_id, growth.increase_roll
          FROM public.characters AS character
          JOIN public.character_sheet_versions AS sheet
            ON sheet.character_id = character.character_id
           AND sheet.version = character.current_sheet_version
          JOIN public.growth_events AS growth
            ON growth.new_sheet_version_id = sheet.sheet_version_id
         WHERE character.character_id = 'character_p06_player'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(persisted_growth.0, combat_health_source.1 + 1);
    assert_eq!(
        persisted_growth
            .1
            .pointer("/skills/Library Use")
            .and_then(serde_json::Value::as_i64),
        Some(i64::from(growth_outcome.skill_after))
    );
    assert_eq!(persisted_growth.2, "SERVER_OS_CSPRNG");
    assert_eq!(
        persisted_growth.3,
        growth_roll.evidence().improvement_check().roll_id()
    );
    assert_eq!(
        persisted_growth.4.as_deref(),
        growth_roll.evidence().increase().map(|roll| roll.roll_id())
    );
    assert_eq!(
        persisted_growth.5.map(|value| value as u8),
        growth_outcome.increase_roll
    );
    let p08_roll_consumptions_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.gameplay_roll_consumptions \
         WHERE campaign_id = $1",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        p08_roll_consumptions_before >= 4,
        "Growth must join Combat and Chase in the global server-roll ownership projection"
    );

    create_campaign(
        &repository,
        CHILD_CAMPAIGN_ID,
        CHILD_AUTHORITY_ID,
        "room_p06_fork_child",
        "child_campaign_create",
    )
    .await;
    sqlx::query(
        r#"
        INSERT INTO public.campaign_memberships (
            campaign_id, user_id, role, granted_by, granted_at
        ) VALUES (
            $1, $2, 'CAMPAIGN_OWNER', $3,
            to_timestamp($4::double precision / 1000.0)
        )
        "#,
    )
    .bind(CAMPAIGN_ID)
    .bind(CAMPAIGN_OWNER_ID)
    .bind(KEEPER_ID)
    .bind(NOW_MS as i64)
    .execute(&primary)
    .await
    .expect("seed the party-scoped Campaign Owner preview probe");
    assert!(matches!(
        repository
            .preview_campaign_fork(CAMPAIGN_ID, "session_p06_schema", CAMPAIGN_OWNER_ID,)
            .await,
        Err(CoreDomainRepositoryError::Forbidden)
    ));
    let snapshot = repository
        .preview_campaign_fork(CAMPAIGN_ID, "session_p06_schema", KEEPER_ID)
        .await
        .expect("compute a canonical public-only fork snapshot");
    include!("11_unrelated_fork_lineage.rs");
}
