{
    for required in [
        "CampaignCreated",
        "CampaignInviteIssued",
        "CampaignInviteAccepted",
        "CharacterCreated",
        "CharacterSubmitted",
        "CharacterInitialVersionApproved",
        "ScenarioImported",
        "SessionStarted",
        "PlayerActionSubmitted",
        "DiceRolled",
        "SkillCheckResolved",
        "ClueRevealed",
        "SanityLossApplied",
        "DecisionCommitted",
        "SceneSwitched",
        "CombatStateRecorded",
        "ChaseStateRecorded",
        "SessionStateChanged",
        "EndingRecorded",
        "CharacterGrowthApplied",
        "ReconsiderationRequested",
        "ReconsiderationReviewed",
        "ReconsiderationCorrected",
        "CampaignForkRecorded",
        "CampaignForkMaterializationRecorded",
        "CampaignForkMaterialized",
    ] {
        assert!(
            actual_event_types.contains(required),
            "the production Event Store is missing required Tutorial event {required}; actual={actual_event_types:?}"
        );
    }

    let canonical_counts = sqlx::query(
        r#"
        SELECT
          count(*) AS events,
          count(*) FILTER (
            WHERE integrity_status = 'verified_hmac'
              AND event_integrity_version = 3
              AND payload_json ? 'protected_payload'
          ) AS verified_events,
          (SELECT count(*) FROM public.event_outbox) AS outbox,
          (SELECT count(*) FROM public.event_outbox
            WHERE integrity_status = 'verified_hmac'
              AND payload_json ? 'protected_payload') AS protected_outbox,
          (SELECT count(*) FROM public.formal_commits
            WHERE status = 'committed') AS committed,
          (SELECT count(*) FROM public.formal_commits) AS total_commits
        FROM public.event_store
        "#,
    )
    .fetch_one(&primary)
    .await
    .expect("verify Event Store, Outbox and formal commits");
    let event_count = canonical_counts.get::<i64, _>("events");
    assert!(event_count > 0);
    assert_eq!(
        canonical_counts.get::<i64, _>("verified_events"),
        event_count
    );
    assert_eq!(canonical_counts.get::<i64, _>("outbox"), event_count);
    assert_eq!(
        canonical_counts.get::<i64, _>("protected_outbox"),
        event_count
    );
    assert_eq!(
        canonical_counts.get::<i64, _>("committed"),
        canonical_counts.get::<i64, _>("total_commits")
    );
    integrity_verifier
        .verify_integrity()
        .await
        .expect("verify primary audit/HMAC chains and independent Witness bindings");
}
