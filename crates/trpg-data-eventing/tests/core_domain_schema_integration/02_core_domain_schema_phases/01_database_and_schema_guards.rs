{
    let primary_url =
        env::var("P06_DATABASE_URL").expect("P06_DATABASE_URL is required for the real DB gate");
    let witness_url = env::var("P06_WITNESS_DATABASE_URL")
        .expect("P06_WITNESS_DATABASE_URL is required for the independent witness gate");
    let primary_database =
        env::var("P06_RESET_DATABASE").expect("P06_RESET_DATABASE must name the dedicated DB");
    let witness_database = env::var("P06_WITNESS_RESET_DATABASE")
        .expect("P06_WITNESS_RESET_DATABASE must name the dedicated witness DB");
    let primary = reset_database(&primary_url, &primary_database, false).await;
    let witness = reset_database(&witness_url, &witness_database, true).await;
    witness.close().await;

    let store = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "p06-schema-integrity-key",
        INTEGRITY_KEY,
        "p06-schema-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .expect("connect primary and independent witness");
    store
        .prepare_for_service()
        .await
        .expect("apply the complete forward migration chain");
    let canonical_reader = store.clone();
    let clock = Arc::new(TestClock(AtomicU64::new(NOW_MS)));
    let repository = CoreDomainRepository::new_with_clock(primary.clone(), store, clock.clone());
    let api_projection_pool = PgPoolOptions::new()
        .max_connections(8)
        .after_connect(|connection, _metadata| {
            Box::pin(async move {
                sqlx::query("SET ROLE trpg_api_service")
                    .execute(&mut *connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with(PgConnectOptions::from_str(&primary_url).unwrap())
        .await
        .expect("connect a projection pool constrained to the production API role");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT current_user")
            .fetch_one(&api_projection_pool)
            .await
            .unwrap(),
        "trpg_api_service",
        "the rebuild regression must run with production projection privileges"
    );
    let api_repository = CoreDomainRepository::new_with_clock(
        api_projection_pool.clone(),
        canonical_reader.clone(),
        clock.clone(),
    );

    for (schema, table) in [
        ("public", "campaigns"),
        ("public", "rooms"),
        ("core_domain", "sessions"),
        ("public", "scenes"),
        ("public", "scenarios"),
        ("public", "characters"),
        ("public", "character_sheet_versions"),
        ("public", "campaign_forks"),
        ("public", "reconsiderations"),
        ("public", "combat_states"),
        ("public", "chase_states"),
        ("public", "ending_events"),
        ("public", "growth_events"),
    ] {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM information_schema.tables
                 WHERE table_schema = $1 AND table_name = $2
            )
            "#,
        )
        .bind(schema)
        .bind(table)
        .fetch_one(&primary)
        .await
        .expect("query core table catalog");
        assert!(exists, "{schema}.{table} must exist after empty migration");
    }
    let invite_projection_function: (bool, bool) = sqlx::query_as(
        r#"
        SELECT procedure.prosecdef,
               NOT EXISTS (
                   SELECT 1
                     FROM aclexplode(
                         COALESCE(
                             procedure.proacl,
                             acldefault('f', procedure.proowner)
                         )
                     ) AS privilege
                    WHERE privilege.grantee = 0
                      AND privilege.privilege_type = 'EXECUTE'
               )
          FROM pg_proc AS procedure
          JOIN pg_namespace AS namespace
            ON namespace.oid = procedure.pronamespace
         WHERE namespace.nspname = 'core_domain'
           AND procedure.proname = 'apply_campaign_invite_acceptance'
        "#,
    )
    .fetch_one(&primary)
    .await
    .expect("invite acceptance projection function exists");
    assert!(
        invite_projection_function.0,
        "invite projection must be SECURITY DEFINER"
    );
    assert!(
        invite_projection_function.1,
        "PUBLIC must not execute the invite projection"
    );
    let login_session_has_token_hash: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM information_schema.columns
             WHERE table_schema = 'public'
               AND table_name = 'sessions'
               AND column_name = 'token_hash'
        )
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        login_session_has_token_hash,
        "P06 must not overwrite the P02 login-session table"
    );
    let identity_depends_on_projection: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
              FROM pg_constraint AS constraint_row
              JOIN pg_class AS source
                ON source.oid = constraint_row.conrelid
              JOIN pg_class AS target
                ON target.oid = constraint_row.confrelid
             WHERE constraint_row.contype = 'f'
               AND source.relname IN (
                   'campaign_memberships', 'authority_contracts'
               )
               AND target.relname = 'campaigns'
        )
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        !identity_depends_on_projection,
        "P06 projections must not reverse the P02 identity dependency"
    );
    let campaign_authority_pair_constraint: String = sqlx::query_scalar(
        r#"
        SELECT pg_get_constraintdef(oid)
          FROM pg_constraint
         WHERE conname = 'campaigns_authority_contract_fkey'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        campaign_authority_pair_constraint
            .contains("FOREIGN KEY (authority_contract_id, campaign_id)")
            && campaign_authority_pair_constraint
                .contains("REFERENCES authority_contracts(contract_id, campaign_id)"),
        "Campaign must bind the exact immutable Authority Contract pair"
    );
    let live_session_index: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM pg_indexes
             WHERE schemaname = 'core_domain'
               AND tablename = 'sessions'
               AND indexname = 'sessions_one_live_per_room_idx'
               AND indexdef LIKE '%WHERE (state = ANY%'
        )
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        live_session_index,
        "live Session uniqueness must be physical"
    );
    let canonical_crosses_identity_boundary: bool = sqlx::query_scalar(
        r#"
        SELECT has_table_privilege(
                   'trpg_canonical_service', 'public.users', 'SELECT'
               )
            OR has_table_privilege(
                   'trpg_canonical_service', 'public.campaign_memberships', 'SELECT'
               )
            OR has_table_privilege(
                   'trpg_canonical_service', 'public.campaigns', 'SELECT'
               )
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        !canonical_crosses_identity_boundary,
        "canonical service must retain the P05 event-only database boundary"
    );
    let api_projection_privileges: bool = sqlx::query_scalar(
        r#"
        SELECT has_schema_privilege(
                   'trpg_api_service', 'core_domain', 'USAGE'
               )
           AND has_table_privilege(
                   'trpg_api_service', 'public.campaigns', 'INSERT'
               )
           AND has_table_privilege(
                   'trpg_api_service', 'core_domain.sessions', 'UPDATE'
               )
           AND NOT has_table_privilege(
                   'trpg_api_service', 'public.campaigns', 'DELETE'
               )
           AND NOT has_table_privilege(
                   'trpg_api_service', 'core_domain.sessions', 'DELETE'
               )
           AND has_function_privilege(
                   'trpg_api_service',
                   'core_domain.clear_p08_rebuildable_projections(text,text)',
                   'EXECUTE'
               )
           AND NOT has_function_privilege(
                   'trpg_worker_service',
                   'core_domain.clear_p08_rebuildable_projections(text,text)',
                   'EXECUTE'
               )
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        api_projection_privileges,
        "API projection role must be able to apply guarded rows but never delete them"
    );
    assert!(
        sqlx::query("DELETE FROM public.combat_states WHERE campaign_id = 'not_present'")
            .execute(&api_projection_pool)
            .await
            .is_err(),
        "the API role must not receive direct projection DELETE privileges"
    );
    assert!(
        sqlx::query("SELECT core_domain.clear_p08_rebuildable_projections($1, $2)",)
            .bind("not_present")
            .bind("not_present")
            .execute(&api_projection_pool)
            .await
            .is_err(),
        "the privileged cleanup must reject callers without a canonical capability"
    );
    let projection_targets_column: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM information_schema.columns
             WHERE table_schema = 'public'
               AND table_name = 'event_store'
               AND column_name = 'projection_targets'
               AND data_type = 'jsonb'
        )
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        projection_targets_column,
        "canonical events must carry integrity-protected projection target identities"
    );
    let projection_guard: String = sqlx::query_scalar(
        "SELECT pg_get_functiondef('public.enforce_core_projection_event()'::regprocedure)",
    )
    .fetch_one(&primary)
    .await
    .expect("load core projection guard definition");
    assert!(
        projection_guard.contains("canonical.authenticated_actor_role IS DISTINCT FROM 'workflow'")
            && projection_guard.contains("audit.action = 'write_official_state'")
            && projection_guard.contains("audit.decision = 'PERMIT'")
            && projection_guard.contains("canonical.projection_targets")
            && projection_guard.contains("trpg.projection_capability")
            && projection_guard.contains("capability_hash"),
        "database projections must require a permitted decision, exact target, and secret capability"
    );

    for (user_id, login) in [
        (KEEPER_ID, "keeper-p06"),
        (PLAYER_ID, "player-p06"),
        (OTHER_ID, "other-p06"),
        (CAMPAIGN_OWNER_ID, "owner-p06"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO public.users (
                user_id, login_normalized, password_hash, global_role
            ) VALUES ($1, $2, 'not-used-by-repository-test', 'USER')
            "#,
        )
        .bind(user_id)
        .bind(login)
        .execute(&primary)
        .await
        .expect("seed repository identity reference");
    }

    let mut direct_user_write =
        campaign_metadata(CAMPAIGN_ID, AUTHORITY_ID, "direct_user_write_rejected");
    direct_user_write.authenticated_actor_id = KEEPER_ID.to_owned();
    direct_user_write.authenticated_actor_role = "human_keeper".to_owned();
    direct_user_write.authenticated_actor_origin = EventActorOriginWire::UserSession {
        session_id: "identity_session_direct_user_write".to_owned(),
    };
    direct_user_write.audit.actor_id = KEEPER_ID.to_owned();
    direct_user_write.audit.actor_origin = "user_session".to_owned();
    direct_user_write.audit.authentication_reference =
        "identity_session_direct_user_write".to_owned();
    direct_user_write.audit.requested_role = "human_keeper".to_owned();
    include!("02_authority_and_direct_write_rejection.rs");
}
