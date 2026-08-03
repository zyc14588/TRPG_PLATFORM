#[test]
fn published_v1_api_completes_the_core_lifecycle_with_real_least_privilege_logins() {
    let admin_database_url = required("AR06_ADMIN_DATABASE_URL");
    let admin_witness_database_url = required("AR06_ADMIN_WITNESS_DATABASE_URL");
    let api_database_url = required("AR06_API_DATABASE_URL");
    let canonical_database_url = required("AR06_CANONICAL_DATABASE_URL");
    let witness_database_url = required("AR06_WITNESS_DATABASE_URL");
    let redis_url = required("AR06_REDIS_URL");
    let setup_runtime = tokio::runtime::Runtime::new().expect("AR06 setup runtime");

    let migration_store = setup_runtime
        .block_on(PostgresCanonicalStore::connect(
            &admin_database_url,
            &admin_witness_database_url,
            "ar06-migration-integrity",
            &INTEGRITY_KEY,
            "ar06-migration-payload",
            &PAYLOAD_KEY,
        ))
        .expect("connect AR06 migration stores");
    setup_runtime
        .block_on(migration_store.prepare_for_service())
        .expect("apply complete migration chain");
    let witness_owner_pool = setup_runtime
        .block_on(
            PgPoolOptions::new()
                .max_connections(1)
                .connect(&admin_witness_database_url),
        )
        .expect("connect witness owner for runtime-role bootstrap");
    setup_runtime
        .block_on(
            sqlx::query(
                "GRANT trpg_witness_append_service \
                 TO trpg_witness_append_login",
            )
            .execute(&witness_owner_pool),
        )
        .expect("grant the production witness append role");

    let api_pool = setup_runtime
        .block_on(
            PgPoolOptions::new()
                .max_connections(4)
                .connect(&api_database_url),
        )
        .expect("connect the real trpg_api_login");
    let canonical_pool = setup_runtime
        .block_on(
            PgPoolOptions::new()
                .max_connections(2)
                .connect(&canonical_database_url),
        )
        .expect("connect the real trpg_canonical_login");
    let canonical_witness_pool = setup_runtime
        .block_on(
            PgPoolOptions::new()
                .max_connections(2)
                .connect(&witness_database_url),
        )
        .expect("connect the real trpg_witness_append_login");
    assert_eq!(
        setup_runtime
            .block_on(sqlx::query_scalar::<_, String>("SELECT current_user").fetch_one(&api_pool))
            .expect("read API login"),
        "trpg_api_login"
    );
    assert_eq!(
        setup_runtime
            .block_on(
                sqlx::query_scalar::<_, String>("SELECT current_user").fetch_one(&canonical_pool)
            )
            .expect("read canonical login"),
        "trpg_canonical_login"
    );
    assert_eq!(
        setup_runtime
            .block_on(
                sqlx::query_scalar::<_, String>("SELECT current_user")
                    .fetch_one(&canonical_witness_pool)
            )
            .expect("read witness append login"),
        "trpg_witness_append_login"
    );
    let least_privilege: (bool, bool, bool, bool, bool) = setup_runtime
        .block_on(
            sqlx::query_as(
                r#"
            SELECT has_table_privilege(current_user, 'public.campaign_exports', 'SELECT'),
                   has_table_privilege(current_user, 'public.campaign_exports', 'INSERT'),
                   has_table_privilege(current_user, 'public.campaign_exports', 'UPDATE'),
                   has_table_privilege(current_user, 'public.campaign_exports', 'DELETE'),
                   has_table_privilege(
                       current_user, 'core_domain.session_characters', 'INSERT'
                   )
            "#,
            )
            .fetch_one(&api_pool),
        )
        .expect("read AR06 API privileges");
    assert_eq!(least_privilege, (true, true, false, false, true));
    let can_read_player_action: bool = setup_runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT has_table_privilege(\
                 current_user, 'public.player_actions', 'SELECT'\
             )",
            )
            .fetch_one(&api_pool),
        )
        .expect("read Player Action query privilege");
    assert!(can_read_player_action);
    let canonical_crosses_projection_boundary: bool = setup_runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT has_table_privilege(\
                current_user, 'public.campaign_exports', 'INSERT'\
             ) OR has_table_privilege(\
                current_user, 'core_domain.session_characters', 'INSERT'\
             )",
            )
            .fetch_one(&canonical_pool),
        )
        .expect("read canonical projection boundary");
    assert!(!canonical_crosses_projection_boundary);
    let canonical_payload_key_privileges: (bool, bool, bool, bool) = setup_runtime
        .block_on(
            sqlx::query_as(
                r#"
            SELECT has_table_privilege(
                       current_user, 'public.privacy_subject_keys', 'SELECT'
                   ),
                   has_table_privilege(
                       current_user, 'public.privacy_subject_keys', 'INSERT'
                   ),
                   has_table_privilege(
                       current_user, 'public.privacy_subject_keys', 'UPDATE'
                   ),
                   has_table_privilege(
                       current_user, 'public.privacy_subject_keys', 'DELETE'
                   )
            "#,
            )
            .fetch_one(&canonical_pool),
        )
        .expect("read canonical subject-payload-key boundary");
    assert_eq!(canonical_payload_key_privileges, (true, true, false, false));

    let now = now_unix_ms();
    let mut identity = IdentityService::from_prepared_postgres_with_security_and_redis_tls(
        &api_database_url,
        None,
        &redis_url,
        "ar06:identity",
        &IDENTITY_KEY,
        3_600_000,
        2,
        None,
        None,
        None,
    )
    .expect("connect persistent AR06 identity through trpg_api_login");
    for (user_id, login, role) in [
        (
            BOOTSTRAP_ID,
            "bootstrap-ar06@example.test",
            GlobalRole::ServerOwner,
        ),
        (KEEPER_ID, "keeper-ar06@example.test", GlobalRole::User),
        (PLAYER_ID, "player-ar06@example.test", GlobalRole::User),
        (OUTSIDER_ID, "outsider-ar06@example.test", GlobalRole::User),
    ] {
        identity
            .create_user(user_id, login, PASSWORD, role)
            .expect("create AR06 identity");
    }
    let bootstrap_session = identity
        .login("bootstrap-ar06@example.test", PASSWORD, now)
        .expect("login AR06 authority bootstrap owner");
    let bootstrap_authentication = identity
        .authenticate_session(Some(bootstrap_session.token.expose()), now + 1)
        .expect("authenticate AR06 authority bootstrap owner");
    for (campaign_id, created_at) in [(CAMPAIGN_ID, 1_u64), (CHILD_CAMPAIGN_ID, 2_u64)] {
        identity
            .grant_membership(
                &bootstrap_authentication,
                campaign_id,
                KEEPER_ID,
                CampaignRole::HumanKeeper,
                now + 1,
            )
            .expect("pre-provision canonical keeper membership");
        identity
            .register_authority_contract(
                &bootstrap_authentication,
                authority(campaign_id, created_at),
                now + 1,
            )
            .expect("pre-provision immutable Authority Contract");
    }

    let canonical_store = setup_runtime
        .block_on(PostgresCanonicalStore::connect(
            &canonical_database_url,
            &witness_database_url,
            "ar06-service-integrity",
            &INTEGRITY_KEY,
            "ar06-service-payload",
            &PAYLOAD_KEY,
        ))
        .expect("connect canonical stores with service login");
    let privacy_runtime = tokio::runtime::Runtime::new().expect("AR06 privacy runtime");
    let deletion_repository = privacy_runtime
        .block_on(PostgresDeletionRepository::connect(&api_database_url))
        .expect("connect deletion repository through API login");
    let canonical_runtime = tokio::runtime::Runtime::new().expect("AR06 canonical runtime");
    let audit_path = PathBuf::from(format!(
        "/tmp/trpg-ar06-v1-audit-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&audit_path);
    let _ = std::fs::remove_file(audit_path.with_extension("jsonl.head"));
    let audit =
        FileAuditLog::open(&audit_path, "ar06-audit-key", &AUDIT_KEY).expect("open AR06 audit log");
    let application = ApiApplication::new_production_governed_with_v1_lifecycle(
        identity,
        policy(),
        audit,
        canonical_runtime,
        canonical_store,
        privacy_runtime,
        deletion_repository,
        &api_database_url,
    )
    .expect("compose the published V1 application without importing a Repository");

    let keeper_token = login(&application, "keeper-ar06@example.test");
    let player_token = login(&application, "player-ar06@example.test");
    let outsider_token = login(&application, "outsider-ar06@example.test");

    let unauthenticated = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        None,
        Some(json!({})),
    );
    expect_status(&unauthenticated, 401, "unauthenticated Campaign create");

    let parent_campaign = json!({
        "command": command("campaign_create", 0),
        "campaign_id": CAMPAIGN_ID,
        "owner_user_id": KEEPER_ID,
        "title": "AR06 public lifecycle",
        "room_id": "room_ar06_http",
        "room_name": "AR06 table",
        "created_at_unix_ms": 1,
        "authority": authority_body(CAMPAIGN_ID)
    });
    let mut wrong_authority = parent_campaign.clone();
    wrong_authority["authority"]["authority_mode"] = json!("AI_KP");
    let wrong_authority_response = call(
        &application,
        "POST",
        "/api/v1/campaigns",
        Some(&keeper_token),
        Some(wrong_authority),
    );
    expect_status(&wrong_authority_response, 400, "wrong Authority mode");
    let event_count_before_create: i64 = setup_runtime
        .block_on(
            sqlx::query_scalar("SELECT count(*) FROM public.event_store").fetch_one(&api_pool),
        )
        .expect("count events before Campaign create");
    assert_eq!(event_count_before_create, 0);
    include!("02_lifecycle_contract/02_campaign_session_and_action.rs");
}
