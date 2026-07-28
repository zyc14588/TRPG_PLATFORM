{
    let missed_event_sequence: i64 = sqlx::query_scalar(
        "SELECT max(sequence) FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = 'combat_p08_schema'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence \
               FROM public.gameplay_roll_consumptions \
              WHERE roll_id = $1",
        )
        .bind(missed_attack.roll_id())
        .fetch_one(&primary)
        .await
        .unwrap(),
        missed_event_sequence,
        "the canonical transaction must durably reserve a roll even when its state projection fails"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT version FROM public.combat_states \
             WHERE combat_id = 'combat_p08_schema'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        1,
        "the injected failure must roll back the separate state projection"
    );
    sqlx::raw_sql(
        r#"
        DROP TRIGGER zz_reject_p08_combat_projection_for_test
            ON public.combat_states;
        DROP FUNCTION public.reject_p08_combat_projection_for_test();
        "#,
    )
    .execute(&primary)
    .await
    .expect("remove P08 projection failure injection");
    repository
        .record_combat_state(&missed_attack_metadata, &missed_attack_request)
        .await
        .expect("exact retry must project the canonically reserved missed attack");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND stream_id = 'combat_p08_schema' \
               AND sequence = $2",
        )
        .bind(CAMPAIGN_ID)
        .bind(missed_event_sequence)
        .fetch_one(&primary)
        .await
        .unwrap(),
        1,
        "projection recovery must not append a duplicate canonical event"
    );
    assert_eq!(
        persist_combat_turn_advance(
            &repository,
            &mut combat,
            2,
            "combat_p08_after_miss_to_player",
        )
        .await,
        "character_p06_player"
    );
    assert_eq!(
        persist_combat_turn_advance(
            &repository,
            &mut combat,
            3,
            "combat_p08_after_miss_to_marta",
        )
        .await,
        "npc_marta"
    );
    let first_attack = percentile_with_result(80, true);
    let first_damage_roll = damage_with_value(1, 6, 5, 6);
    let first_damage = combat
        .apply_damage(
            "character_p06_player",
            CombatActionKind::Firearm,
            CombatDefense::None,
            &first_attack,
            None,
            Some(&first_damage_roll),
        )
        .unwrap();
    assert_eq!(first_damage.condition, CombatCondition::MajorWound);
    repository
        .record_combat_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_schema",
                "combat_state",
                "combat.state.damage",
                4,
                "combat_p08_major_wound",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: combat.persistence_json().unwrap(),
                attacker_roll: Some(first_attack),
                defender_roll: None,
                damage_roll: Some(first_damage_roll),
                medical_roll: None,
            },
        )
        .await
        .expect("persist MajorWound combat state");
    let projected_wound: (i64, String, String, String, String) = sqlx::query_as(
        r#"
        SELECT character.current_sheet_version,
               sheet.sheet_json #>> '{combat_profile,current_hp}',
               sheet.sheet_json #>> '{combat_profile,condition}',
               character.visibility_label::TEXT,
               sheet.visibility_label::TEXT
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
        projected_wound,
        (
            2,
            "5".to_owned(),
            "MAJOR_WOUND".to_owned(),
            "private_to_player".to_owned(),
            "private_to_player".to_owned(),
        ),
        "Combat damage must create a new locked private Character sheet version"
    );
    assert_eq!(
        persist_combat_turn_advance(
            &repository,
            &mut combat,
            5,
            "combat_p08_after_wound_to_player",
        )
        .await,
        "character_p06_player"
    );
    assert_eq!(
        persist_combat_turn_advance(
            &repository,
            &mut combat,
            6,
            "combat_p08_after_wound_to_marta",
        )
        .await,
        "npc_marta"
    );
    let later_attack = percentile_with_result(60, true);
    let later_damage_roll = damage_with_value(1, 6, 0, 1);
    let later_damage = combat
        .apply_damage(
            "character_p06_player",
            CombatActionKind::Melee,
            CombatDefense::None,
            &later_attack,
            None,
            Some(&later_damage_roll),
        )
        .unwrap();
    assert_eq!(
        later_damage.condition,
        CombatCondition::MajorWound,
        "later small damage cannot clear an existing MajorWound"
    );
    repository
        .record_combat_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_schema",
                "combat_state",
                "combat.state.damage",
                7,
                "combat_p08_wound_persists",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: combat.persistence_json().unwrap(),
                attacker_roll: Some(later_attack),
                defender_roll: None,
                damage_roll: Some(later_damage_roll),
                medical_roll: None,
            },
        )
        .await
        .expect("persist continuing MajorWound state");
    assert_eq!(
        persist_combat_turn_advance(
            &repository,
            &mut combat,
            8,
            "combat_p08_after_small_hit_to_player",
        )
        .await,
        "character_p06_player"
    );
    assert_eq!(
        persist_combat_turn_advance(
            &repository,
            &mut combat,
            9,
            "combat_p08_after_small_hit_to_marta",
        )
        .await,
        "npc_marta"
    );
    let fight_back_attack = percentile_with_level(60, SuccessLevel::Regular);
    let fight_back_defense = percentile_with_level(45, SuccessLevel::Hard);
    let wrong_attacker_formula = damage_with_value(1, 6, 0, 1);
    let before_wrong_formula = combat.persistence_json().unwrap();
    assert_eq!(
        combat
            .apply_damage(
                "character_p06_player",
                CombatActionKind::Melee,
                CombatDefense::FightBack,
                &fight_back_attack,
                Some(&fight_back_defense),
                Some(&wrong_attacker_formula),
            )
            .unwrap_err(),
        trpg_shared_kernel::TrpgError::InvalidConfiguration("combat_damage_evidence"),
        "fight-back damage must use the defender's selected melee weapon formula"
    );
    assert_eq!(
        combat.persistence_json().unwrap(),
        before_wrong_formula,
        "rejecting a weapon-formula mismatch must not mutate canonical combat state"
    );
    let fight_back_damage = damage_with_value(1, 6, 1, 2);
    combat
        .apply_damage(
            "character_p06_player",
            CombatActionKind::Melee,
            CombatDefense::FightBack,
            &fight_back_attack,
            Some(&fight_back_defense),
            Some(&fight_back_damage),
        )
        .unwrap();
    let fight_back_state = combat.persistence_json().unwrap();
    let forged_fight_back_state = fight_back_state.replace("DEFENDER_FOUGHT_BACK", "ATTACKER_HIT");
    let combat_events_before_forged_outcome: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = 'combat_p08_schema'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(matches!(
        repository
            .record_combat_state(
                &metadata(
                    CAMPAIGN_ID,
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "combat_p08_schema",
                    "combat_state",
                    "combat.state.damage",
                    10,
                    "combat_p08_forged_fight_back",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordCombatStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p06_schema".to_owned(),
                    state_json: forged_fight_back_state,
                    attacker_roll: Some(fight_back_attack.clone()),
                    defender_roll: Some(fight_back_defense.clone()),
                    damage_roll: Some(fight_back_damage.clone()),
                    medical_roll: None,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput("combat_transition"))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND stream_id = 'combat_p08_schema'",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        combat_events_before_forged_outcome
    );
    include!("07_combat_terminal_and_health_projection.rs");
}
