#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn production_realtime_identity_filters_canonical_replay_and_nats_only_wakes() {
    let primary_url = required("AR07_DATABASE_URL");
    let realtime_url = required("AR07_REALTIME_DATABASE_URL");
    let witness_url = required("AR07_WITNESS_DATABASE_URL");
    let nats_url = required("AR07_NATS_URL");
    let primary_name = required("AR07_DATABASE_NAME");
    let witness_name = required("AR07_WITNESS_DATABASE_NAME");
    reset_dedicated_database(&primary_url, &primary_name).await;
    reset_dedicated_database(&witness_url, &witness_name).await;

    let store = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "ar07-integrity-key",
        &INTEGRITY_KEY,
        "ar07-payload-key",
        &PAYLOAD_KEY,
    )
    .await
    .expect("connect AR07 canonical store");
    store
        .prepare_for_service()
        .await
        .expect("apply AR07 migration chain");

    let admin_pool = store.primary_pool();
    let migrated: bool = sqlx::query_scalar(
        "SELECT EXISTS (\
            SELECT 1 FROM _sqlx_migrations WHERE version = 20260730000300\
         )",
    )
    .fetch_one(&admin_pool)
    .await
    .expect("read migration ledger");
    assert!(migrated);
    let realtime_privileges: (bool, bool, bool, bool) = sqlx::query_as(
        r#"
        SELECT has_column_privilege(
                   'trpg_realtime_service', 'public.users', 'user_id', 'SELECT'
               ),
               has_column_privilege(
                   'trpg_realtime_service', 'public.users', 'password_hash', 'SELECT'
               ),
               has_column_privilege(
                   'trpg_realtime_service', 'public.sessions', 'token_hash', 'SELECT'
               ),
               has_table_privilege(
                   'trpg_realtime_service', 'public.event_store', 'INSERT'
               )
        "#,
    )
    .fetch_one(&admin_pool)
    .await
    .expect("read realtime role privileges");
    assert_eq!(realtime_privileges, (true, false, true, false));

    let now = now_unix_ms();
    let identity_url = primary_url.clone();
    let (keeper_token, player_a_token, player_b_token, spectator_token) =
        tokio::task::spawn_blocking(move || {
            let mut identity =
                IdentityService::from_postgres(&identity_url, &IDENTITY_KEY, 3_600_000)
                    .expect("create persistent AR07 identities");
            for (user_id, login, role) in [
                (OWNER, "owner-ar07@example.test", GlobalRole::ServerOwner),
                (KEEPER, "keeper-ar07@example.test", GlobalRole::User),
                (PLAYER_A, "player-a-ar07@example.test", GlobalRole::User),
                (PLAYER_B, "player-b-ar07@example.test", GlobalRole::User),
                (SPECTATOR, "spectator-ar07@example.test", GlobalRole::User),
            ] {
                identity
                    .create_user(user_id, login, PASSWORD, role)
                    .expect("create AR07 user");
            }
            let owner_login = identity
                .login("owner-ar07@example.test", PASSWORD, now)
                .expect("login AR07 owner");
            let owner_authentication = identity
                .authenticate_session(Some(owner_login.token.expose()), now + 1)
                .expect("authenticate AR07 owner");
            for (user, role) in [
                (KEEPER, CampaignRole::HumanKeeper),
                (PLAYER_A, CampaignRole::Player),
                (PLAYER_B, CampaignRole::Player),
                (SPECTATOR, CampaignRole::Spectator),
            ] {
                identity
                    .grant_membership(&owner_authentication, CAMPAIGN, user, role, now + 2)
                    .expect("grant AR07 campaign membership");
            }
            identity
                .register_authority_contract(&owner_authentication, authority(now + 3), now + 3)
                .expect("register AR07 immutable authority");
            identity
                .create_campaign_group(&owner_authentication, CAMPAIGN, GROUP, now + 4)
                .expect("create AR07 split-party group");
            identity
                .grant_group_membership(&owner_authentication, CAMPAIGN, GROUP, PLAYER_A, now + 5)
                .expect("grant player A split-party membership");

            (
                identity
                    .login("keeper-ar07@example.test", PASSWORD, now + 10)
                    .expect("login keeper")
                    .token
                    .expose()
                    .to_owned(),
                identity
                    .login("player-a-ar07@example.test", PASSWORD, now + 11)
                    .expect("login player A")
                    .token
                    .expose()
                    .to_owned(),
                identity
                    .login("player-b-ar07@example.test", PASSWORD, now + 12)
                    .expect("login player B")
                    .token
                    .expose()
                    .to_owned(),
                identity
                    .login("spectator-ar07@example.test", PASSWORD, now + 13)
                    .expect("login spectator")
                    .token
                    .expose()
                    .to_owned(),
            )
        })
        .await
        .expect("join AR07 identity bootstrap");

    for (ordinal, event_type, label, subject) in [
        (1, "CampaignCreated", "public", "not_applicable"),
        (2, "ClueRevealed", "private_to_group", GROUP),
        (3, "DiceRolled", "private_to_player", PLAYER_A),
        (4, "SessionSummaryCreated", "keeper_only", "not_applicable"),
    ] {
        store
            .commit(&draft(
                ordinal,
                i64::try_from(ordinal - 1).expect("stream version"),
                event_type,
                label,
                subject,
            ))
            .await
            .expect("commit AR07 canonical event");
    }

    let publisher = JetStreamOutboxPublisher::connect(
        store.clone(),
        &nats_url,
        "ar07-realtime-notification",
        None,
    )
    .await
    .expect("connect real AR07 NATS");
    publisher.ensure_stream().await.expect("ensure AR07 stream");
    let mut notifications = publisher
        .subscribe_canonical_notifications()
        .await
        .expect("subscribe to canonical notifications");
    let published = publisher
        .publish_batch()
        .await
        .expect("publish AR07 outbox");
    assert_eq!(published.published, 4);
    assert!(
        tokio::time::timeout(Duration::from_secs(2), notifications.next())
            .await
            .expect("NATS notification timeout")
            .expect("NATS notification stream"),
        "NATS must wake the realtime reader without becoming its payload truth"
    );

    let replay = store
        .load_replay_page(CAMPAIGN, 0, 20)
        .await
        .expect("load canonical AR07 replay");
    assert_eq!(replay.len(), 4);
    let realtime_store = PostgresCanonicalStore::connect(
        &realtime_url,
        &witness_url,
        "ar07-integrity-key",
        &INTEGRITY_KEY,
        "ar07-payload-key",
        &PAYLOAD_KEY,
    )
    .await
    .expect("connect canonical reader through realtime role");
    assert_eq!(
        realtime_store
            .load_replay_page(CAMPAIGN, 0, 20)
            .await
            .expect("replay through least-privilege production role")
            .len(),
        4
    );
    JetStreamOutboxPublisher::connect(realtime_store, &nats_url, "ar07-realtime-readiness", None)
        .await
        .expect("connect production notifier through realtime role")
        .check_readiness()
        .await
        .expect("production notifier readiness through realtime role");
    let realtime_identity = PersistentRealtimeIdentity::new(realtime_pool(&realtime_url).await);
    let keeper = realtime_identity
        .authenticate(Some(&keeper_token), CAMPAIGN, now + 20)
        .await
        .expect("authenticate keeper through realtime role");
    let player_a = realtime_identity
        .authenticate(Some(&player_a_token), CAMPAIGN, now + 20)
        .await
        .expect("authenticate player A through realtime role");
    let player_b = realtime_identity
        .authenticate(Some(&player_b_token), CAMPAIGN, now + 20)
        .await
        .expect("authenticate player B through realtime role");
    let spectator = realtime_identity
        .authenticate(Some(&spectator_token), CAMPAIGN, now + 20)
        .await
        .expect("authenticate spectator through realtime role");
    assert_eq!(
        visible_types(&keeper, &replay, now + 21).await,
        vec![
            "CampaignCreated",
            "ClueRevealed",
            "DiceRolled",
            "SessionSummaryCreated",
        ]
    );
    assert_eq!(
        visible_types(&player_a, &replay, now + 21).await,
        vec!["CampaignCreated", "ClueRevealed", "DiceRolled"]
    );
    assert_eq!(
        visible_types(&player_b, &replay, now + 21).await,
        vec!["CampaignCreated"]
    );
    assert_eq!(
        visible_types(&spectator, &replay, now + 21).await,
        vec!["CampaignCreated"]
    );

    sqlx::query(
        "UPDATE campaign_group_memberships SET revoked_at = now() \
         WHERE campaign_id = $1 AND group_id = $2 AND user_id = $3",
    )
    .bind(CAMPAIGN)
    .bind(GROUP)
    .bind(PLAYER_A)
    .execute(&admin_pool)
    .await
    .expect("revoke live group membership");
    assert_eq!(
        visible_types(&player_a, &replay, now + 22).await,
        vec!["CampaignCreated", "DiceRolled"]
    );
    let logout_url = primary_url.clone();
    let logout_token = player_a_token.clone();
    tokio::task::spawn_blocking(move || {
        let mut identity = IdentityService::from_postgres(&logout_url, &IDENTITY_KEY, 3_600_000)
            .expect("reconnect AR07 identity service");
        identity
            .logout(&logout_token)
            .expect("revoke player A session");
    })
    .await
    .expect("join AR07 identity revocation");
    let campaign = EntityId::new(CAMPAIGN).expect("valid campaign");
    let public = Visibility::try_from_parts("public", None).expect("public visibility");
    assert!(
        player_a
            .can_view(&campaign, &public, now + 23)
            .await
            .is_err(),
        "revoked session must not retain a replay capability"
    );
}
