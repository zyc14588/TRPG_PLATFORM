{
    Box::pin(async {
        let unrelated_child_campaign_id = "campaign_p08_unrelated_fork_child";
        let unrelated_child_authority_id = "authority_campaign_p08_unrelated_fork_child_1";
        create_campaign(
            &repository,
            unrelated_child_campaign_id,
            unrelated_child_authority_id,
            "room_p08_unrelated_fork_child",
            "unrelated_fork_child_create",
        )
        .await;
        let unrelated_child_events_before: i64 =
            sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
                .bind(unrelated_child_campaign_id)
                .fetch_one(&primary)
                .await
                .unwrap();
        assert!(matches!(
            repository
                .record_campaign_fork(
                    &metadata(
                        unrelated_child_campaign_id,
                        unrelated_child_authority_id,
                        KEEPER_ID,
                        "human_keeper",
                        "fork_p08_unrelated_authority",
                        "campaign_fork",
                        "campaign.fork.record",
                        0,
                        "fork_unrelated_authority",
                        "keeper_only",
                        "not_applicable",
                        "human_keeper_statement",
                    ),
                    &RecordCampaignForkRequest {
                        fork_id: "fork_p08_unrelated_authority".to_owned(),
                        parent_campaign_id: CAMPAIGN_ID.to_owned(),
                        child_campaign_id: unrelated_child_campaign_id.to_owned(),
                        source_session_id: "session_p06_schema".to_owned(),
                        snapshot_hash: snapshot.snapshot_hash.clone(),
                        reason: "An unrelated campaign cannot masquerade as a fork".to_owned(),
                        copy_scopes: snapshot.copy_scopes.clone(),
                    },
                )
                .await,
            Err(CoreDomainRepositoryError::InvalidInput(
                "fork_authority_contract"
            ))
        ));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM public.event_store WHERE campaign_id = $1",
            )
            .bind(unrelated_child_campaign_id)
            .fetch_one(&primary)
            .await
            .unwrap(),
            unrelated_child_events_before,
            "a non-derived Authority Contract must fail before fork lineage enters canonical history"
        );
    })
    .await;
    assert!(
        !snapshot
            .canonical_snapshot_json
            .contains("ScenarioImported"),
        "keeper-only scenario payload metadata must not enter the public fork snapshot"
    );
    assert!(
        !snapshot
            .canonical_snapshot_json
            .contains("keeper_only_fork_sentinel"),
        "keeper-only character sheets must not enter the default fork snapshot"
    );
    assert!(
        !snapshot
            .canonical_snapshot_json
            .contains("P08 Keeper Private Sentinel"),
        "keeper-only character rows must not enter the default fork snapshot"
    );
    assert!(
        !snapshot.canonical_snapshot_json.contains("ai_internal"),
        "AI-internal state must be excluded from the default fork snapshot"
    );
    for scope in [
        CopyScope::CombatState,
        CopyScope::ChaseState,
        CopyScope::ConclusionState,
    ] {
        assert!(
            snapshot.copy_scopes.contains(&scope),
            "the declared copy scope must cover each P08 state embedded in the snapshot"
        );
    }
    let snapshot_value: serde_json::Value =
        serde_json::from_str(&snapshot.canonical_snapshot_json).unwrap();
    assert!(
        snapshot_value["state"]["character_state"]
            .as_array()
            .is_some_and(|characters| characters.iter().any(|character| {
                character["character_id"] == "character_p08_late_joiner"
                    && character["state"] == "APPROVED"
            })),
        "a character created after session start must remain in the fork snapshot even without an action"
    );
    for state_key in ["combat_state", "chase_state", "conclusion_state"] {
        assert!(
            snapshot_value["state"][state_key]
                .as_array()
                .is_some_and(|rows| !rows.is_empty()),
            "the source snapshot must contain actual {state_key} rows"
        );
    }
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
        .expect("record immutable fork lineage");
    Box::pin(async {
        let child_owned_marker: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                  FROM public.event_store AS event
                  CROSS JOIN LATERAL jsonb_array_elements(
                      event.projection_targets
                  ) AS target
                 WHERE event.campaign_id = $1
                   AND event.event_type = 'CampaignForkRecorded'
                   AND target ->> 'relation' =
                       'public.campaign_fork_materializations'
                   AND target ->> 'row_id' = 'fork_p06_schema'
            )
            "#,
        )
        .bind(CHILD_CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap();
        assert!(
            child_owned_marker,
            "new child-owned lineage must persist the HMAC-bound v2 discriminator"
        );
    })
    .await;
    let fork_snapshot = sqlx::query(
        r#"
        SELECT campaign_id, source_snapshot_hash, child_snapshot_hash,
               copy_scope_json, snapshot_json, materialization_version
          FROM public.campaign_forks
         WHERE fork_id = 'fork_p06_schema'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        fork_snapshot.get::<String, _>("campaign_id"),
        CHILD_CAMPAIGN_ID
    );
    assert_eq!(
        fork_snapshot.get::<String, _>("source_snapshot_hash"),
        snapshot.snapshot_hash
    );
    assert_ne!(
        fork_snapshot.get::<String, _>("child_snapshot_hash"),
        snapshot.snapshot_hash,
        "the child hash must seal the child-owned IDs and materialized state, not alias the source hash"
    );
    assert_eq!(fork_snapshot.get::<i16, _>("materialization_version"), 2);
    let snapshot_reference = fork_snapshot.get::<serde_json::Value, _>("snapshot_json");
    assert_eq!(
        snapshot_reference["kind"],
        "CONTENT_ADDRESSED_FORK_SNAPSHOT"
    );
    assert_eq!(
        snapshot_reference["content_address"],
        snapshot.snapshot_hash
    );
    assert_ne!(
        snapshot_reference,
        serde_json::from_str::<serde_json::Value>(&snapshot.canonical_snapshot_json).unwrap(),
        "the unbounded source snapshot must not be embedded in one canonical event"
    );
    let copied_scopes = fork_snapshot.get::<serde_json::Value, _>("copy_scope_json");
    assert!(!copied_scopes
        .as_array()
        .unwrap()
        .iter()
        .any(|scope| matches!(
            scope.as_str(),
            Some("KEEPER_NOTES" | "HIDDEN_CLUES" | "PRIVATE_MESSAGES" | "AI_INTERNAL_MEMORY")
        )));
    let child_state_counts: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
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
            (SELECT count(*) FROM public.campaign_fork_npc_states
              WHERE campaign_id = $1),
            (SELECT count(*) FROM public.combat_states WHERE campaign_id = $1),
            (SELECT count(*) FROM public.chase_states WHERE campaign_id = $1),
            (SELECT count(*) FROM public.ending_events WHERE campaign_id = $1)
        "#,
        )
        .bind(CHILD_CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap();
    let snapshot_scope_len = |name: &str| {
        i64::try_from(
            snapshot_value["state"][name]
                .as_array()
                .expect("fork scope must be an array")
                .len(),
        )
        .unwrap()
    };
    assert_eq!(
        child_state_counts.0, 1,
        "fork must materialize the world/scenario scope"
    );
    assert_eq!(
        child_state_counts.1,
        snapshot_scope_len("character_state"),
        "every copyable character at the source cutoff must be materialized"
    );
    assert_eq!(child_state_counts.2, 1);
    assert_eq!(child_state_counts.3, 2);
    assert_eq!(child_state_counts.4, 1);
    assert_eq!(
        child_state_counts.5,
        snapshot_scope_len("public_events"),
        "every copied public event must have a queryable child projection"
    );
    assert_eq!(child_state_counts.6, snapshot_scope_len("discovered_clues"));
    assert_eq!(child_state_counts.7, snapshot_scope_len("npc_state"));
    assert_eq!(
        (
            child_state_counts.8,
            child_state_counts.9,
            child_state_counts.10
        ),
        (
            snapshot_scope_len("combat_state"),
            snapshot_scope_len("chase_state"),
            snapshot_scope_len("conclusion_state")
        ),
        "combat, chase and conclusion scopes must materialize into normal child projections"
    );
    let child_character_id: String = sqlx::query_scalar(
        "SELECT character_id FROM public.characters \
         WHERE campaign_id = $1 AND owner_user_id = $2",
    )
    .bind(CHILD_CAMPAIGN_ID)
    .bind(PLAYER_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let child_npc_id: String = sqlx::query_scalar(
        "SELECT npc_state_id FROM public.campaign_fork_npc_states \
         WHERE campaign_id = $1 AND source_npc_id = 'npc_marta'",
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let child_gameplay_states: Vec<serde_json::Value> = sqlx::query_scalar(
        r#"
        SELECT state_json FROM public.combat_states WHERE campaign_id = $1
        UNION ALL
        SELECT state_json FROM public.chase_states WHERE campaign_id = $1
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_all(&primary)
    .await
    .unwrap();
    assert!(!child_gameplay_states.is_empty());
    for child_gameplay_state in child_gameplay_states {
        let encoded = serde_json::to_string(&child_gameplay_state).unwrap();
        assert!(
            !encoded.contains("character_p06_player") && !encoded.contains("npc_marta"),
            "forked Combat/Chase state must not retain parent participant identifiers"
        );
        assert!(
            encoded.contains(&child_character_id) && encoded.contains(&child_npc_id),
            "all Combat/Chase participant, initiative, transition, and roll references must use child-owned identifiers"
        );
    }
    let child_event_types: Vec<String> = sqlx::query_scalar(
        "SELECT event_type FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = 'fork_p06_schema' \
         ORDER BY stream_version",
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_all(&primary)
    .await
    .unwrap();
    assert_eq!(child_event_types[0], "CampaignForkRecorded");
    assert_eq!(child_event_types[1], "CampaignForkMaterializationRecorded");
    assert!(child_event_types.len() > 2);
    assert!(child_event_types[2..]
        .iter()
        .all(|event_type| event_type == "CampaignForkMaterialized"));
    include!("12_fork_projection_snapshot.rs");
}
