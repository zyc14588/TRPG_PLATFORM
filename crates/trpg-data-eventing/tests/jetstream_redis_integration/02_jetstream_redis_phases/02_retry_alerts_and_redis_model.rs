{
    assert!(persistent_alert.requires_operator_attention());

    let cache_namespace = format!("p02:projection:test:{suffix}");
    let cache = RedisProjectionCache::connect(
        &redis_url,
        &cache_namespace,
        "redis-integration-v1",
        &[0x83; 32],
    )
    .await
    .unwrap();
    let cache_key = format!("campaign:{suffix}:clues");
    let campaign_id = format!("jetstream_campaign_{suffix}");
    let keeper_id = format!("cache_keeper_{suffix}");
    let mut identity = IdentityService::new(&[0x39; 32], 60_000).unwrap();
    identity
        .create_user(
            &keeper_id,
            &format!("cache-keeper-{suffix}@example.test"),
            "correct horse battery staple",
            GlobalRole::ServerOwner,
        )
        .unwrap();
    let session = identity
        .login(
            &format!("cache-keeper-{suffix}@example.test"),
            "correct horse battery staple",
            1_000,
        )
        .unwrap();
    let authentication = identity
        .authenticate_session(Some(session.token.expose()), 1_001)
        .unwrap();
    identity
        .grant_membership(
            &authentication,
            &campaign_id,
            &keeper_id,
            CampaignRole::HumanKeeper,
            1_002,
        )
        .unwrap();
    let replay = identity
        .verifier()
        .authorize_replay(
            &authentication,
            &EntityId::new(&campaign_id).unwrap(),
            1_003,
        )
        .unwrap();
    cache
        .put(
            &ProjectionCacheEntry::new(
                &cache_key,
                &campaign_id,
                &keeper_id,
                2,
                "keeper_only",
                "not_applicable",
                "rules_engine_decision",
                format!("jetstream_decision_{suffix}"),
                r#"{"count":1}"#,
                60,
            )
            .unwrap(),
        )
        .await
        .unwrap();

    // Redis contains only hashed keys and an AEAD envelope; neither the value
    // nor its data-subject/provenance metadata is present in plaintext.
    let redis_client = redis::Client::open(redis_url.as_str()).unwrap();
    let mut redis_connection = redis::aio::ConnectionManager::new(redis_client)
        .await
        .unwrap();
    let stored_keys: Vec<String> = redis::cmd("KEYS")
        .arg(format!("{cache_namespace}:entry:*"))
        .query_async(&mut redis_connection)
        .await
        .unwrap();
    assert_eq!(stored_keys.len(), 1);
    let stored_value: String = redis::cmd("GET")
        .arg(&stored_keys[0])
        .query_async(&mut redis_connection)
        .await
        .unwrap();
    assert!(!stored_value.contains(r#"\"count\":1"#));
    assert!(!stored_value.contains(&keeper_id));
    assert!(!stored_value.contains(&campaign_id));
    assert!(!stored_value.contains("keeper_only"));
    assert!(!stored_value.contains(&format!("jetstream_decision_{suffix}")));

    assert_eq!(
        cache
            .get_authorized(&cache_key, &replay, 1_004)
            .await
            .unwrap()
            .unwrap()
            .version(),
        2
    );
    assert!(cache
        .put(
            &ProjectionCacheEntry::new(
                &cache_key,
                &campaign_id,
                &keeper_id,
                1,
                "public",
                "not_applicable",
                "system_fixture",
                "stale_projection",
                r#"{"count":0}"#,
                60,
            )
            .unwrap(),
        )
        .await
        .is_err());
    let retained = cache
        .get_authorized(&cache_key, &replay, 1_005)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.version(), 2);
    assert_eq!(retained.visibility_label(), "keeper_only");
    assert_eq!(cache.invalidate_subject(&keeper_id).await.unwrap(), 1);
    assert!(cache
        .get_authorized(&cache_key, &replay, 1_006)
        .await
        .unwrap()
        .is_none());
    cache.invalidate(&cache_key).await.unwrap();

    // A storage restore or privileged tamper after startup invalidates the
    // entire canonical custody. Refuse the next batch before claiming any row
    // instead of treating corruption as one recoverable message failure.
    let mut corruption_transaction = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *corruption_transaction)
        .await
        .unwrap();
    let tampered_rows = sqlx::query(
        "UPDATE event_store \
            SET correlation_id = correlation_id || '_tampered' \
          WHERE campaign_id = $1",
    )
    .bind(format!("jetstream_campaign_{suffix}"))
    .execute(&mut *corruption_transaction)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(tampered_rows, 1);
    corruption_transaction.commit().await.unwrap();

    assert!(
        publisher.publish_batch().await.is_err(),
        "publisher accepted a canonical store whose keyed event chain was corrupted"
    );
    assert!(
        JetStreamOutboxPublisher::connect(
            store,
            &nats_url,
            "p02-jetstream-corruption-restart",
            None,
        )
        .await
        .is_err(),
        "publisher restart accepted corrupted canonical custody"
    );
}
