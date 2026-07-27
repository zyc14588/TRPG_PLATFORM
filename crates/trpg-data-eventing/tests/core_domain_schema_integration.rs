use std::env;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::persistence_postgresql::{
    AcceptInviteRequest, AuthorityContractSnapshot, CoreCommandMetadata, CoreDomainClock,
    CoreDomainRepository, CoreDomainRepositoryError, CreateCampaignRequest, CreateCharacterRequest,
    ImportScenarioRequest, IssueInviteRequest, RecordCampaignForkRequest, RecordChaseStateRequest,
    RecordCombatStateRequest, RecordEndingRequest, RecordGrowthRequest,
    RequestReconsiderationRequest, ResolveReconsiderationRequest, ReviewReconsiderationRequest,
    StartSessionRequest, SwitchSceneRequest,
};
use trpg_domain_core::canonical_gameplay_state::validate_combat_server_roll_evidence;
use trpg_domain_core::domain_entities_value_objects::{
    MembershipRole, ReconsiderationOutcome, SessionState,
};
use trpg_domain_core::fork_canon_lineage::CopyScope;
use trpg_ruleset_coc7::character_combat_san_chase::parse_scenario_yaml;
use trpg_ruleset_coc7::chase_state_machine::{
    ChaseParticipant, ChaseRole, ChaseState, ChaseStatus,
};
use trpg_ruleset_coc7::combat_state_machine::{
    CombatActionKind, CombatCondition, CombatDefense, CombatSkillTargets, CombatState,
    CombatStatus, CombatantState,
};
use trpg_ruleset_coc7::dice_roll_contract::{
    server_roll_skill_growth, success_level, SuccessLevel,
};
use trpg_shared_kernel::{
    server_damage_roll, server_percentile_roll, EventActorOriginWire, ServerDamageRoll,
    ServerPercentileRoll,
};

const INTEGRITY_KEY: &[u8; 32] = &[0x36; 32];
const PAYLOAD_KEY: &[u8; 32] = &[0x47; 32];
const CAMPAIGN_ID: &str = "campaign_p06_schema";
const CHILD_CAMPAIGN_ID: &str = "campaign_p06_fork_child";
const KEEPER_ID: &str = "keeper_p06_schema";
const PLAYER_ID: &str = "player_p06_schema";
const OTHER_ID: &str = "other_p06_schema";
const AUTHORITY_ID: &str = "authority_campaign_p06_schema_1";
const CHILD_AUTHORITY_ID: &str = "authority_campaign_p06_fork_child_1";
const NOW_MS: u64 = 2_000_000_000_000;

fn percentile_with_result(target: u8, succeeds: bool) -> ServerPercentileRoll {
    loop {
        let roll = server_percentile_roll().unwrap();
        let outcome = success_level(roll.value(), target).unwrap();
        let actual = matches!(
            outcome,
            SuccessLevel::Critical
                | SuccessLevel::Extreme
                | SuccessLevel::Hard
                | SuccessLevel::Regular
        );
        if actual == succeeds {
            return roll;
        }
    }
}

fn percentile_with_level(target: u8, expected: SuccessLevel) -> ServerPercentileRoll {
    loop {
        let roll = server_percentile_roll().unwrap();
        if success_level(roll.value(), target).unwrap() == expected {
            return roll;
        }
    }
}

fn damage_with_value(dice_count: u8, die_sides: u8, flat_bonus: i8, value: u8) -> ServerDamageRoll {
    loop {
        let roll = server_damage_roll(dice_count, die_sides, flat_bonus).unwrap();
        if roll.value() == value {
            return roll;
        }
    }
}

#[derive(Debug)]
struct TestClock(AtomicU64);

impl CoreDomainClock for TestClock {
    fn now_unix_ms(&self) -> Result<u64, CoreDomainRepositoryError> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}

async fn reset_database(url: &str, expected_database: &str, witness: bool) -> PgPool {
    assert_eq!(
        env::var("P06_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "P06 integration tests require explicit dedicated-database reset authorization"
    );
    let options = PgConnectOptions::from_str(url).expect("valid P06 PostgreSQL URL");
    assert!(
        matches!(options.get_host(), "localhost" | "127.0.0.1" | "::1"),
        "P06 tests refuse to reset a non-local database"
    );
    assert_eq!(
        options.get_database(),
        Some(expected_database),
        "P06 tests refuse to reset a non-dedicated database"
    );
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect_with(options)
        .await
        .expect("connect dedicated P06 PostgreSQL database");
    let reset_sql = if witness {
        "DROP SCHEMA public CASCADE; CREATE SCHEMA public; GRANT ALL ON SCHEMA public TO public;"
    } else {
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; \
         CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;"
    };
    sqlx::raw_sql(reset_sql)
        .execute(&pool)
        .await
        .expect("reset dedicated P06 schemas");
    pool
}

fn authority(contract_id: &str) -> AuthorityContractSnapshot {
    AuthorityContractSnapshot {
        contract_id: contract_id.to_owned(),
        authority_mode: "HUMAN_KP".to_owned(),
        authority_owner: KEEPER_ID.to_owned(),
        ruleset_version: "coc7-1".to_owned(),
        house_rules_version: "none-1".to_owned(),
        scenario_version: "tutorial-0.1.0".to_owned(),
        prompt_version: "p06-1".to_owned(),
        agent_pack_version: "none-1".to_owned(),
        tool_schema_version: "tools-1".to_owned(),
        safety_profile_version: "safety-1".to_owned(),
        ai_provider_snapshot: "not_applicable".to_owned(),
        model_route_snapshot: "not_applicable".to_owned(),
        character_sheet_template_version: "coc7-sheet-1".to_owned(),
    }
}

#[allow(clippy::too_many_arguments)]
fn metadata(
    _campaign_id: &str,
    authority_id: &str,
    actor_id: &str,
    actor_role: &str,
    stream_id: &str,
    resource_type: &str,
    _action: &str,
    expected_version: i64,
    suffix: &str,
    visibility_label: &str,
    visibility_subject: &str,
    provenance_kind: &str,
) -> CoreCommandMetadata {
    CoreCommandMetadata {
        commit_id: format!("commit_{suffix}"),
        command_id: format!("command_{suffix}"),
        idempotency_key: format!("idempotency_{suffix}"),
        expected_version,
        requesting_actor_id: actor_id.to_owned(),
        requesting_actor_role: actor_role.to_owned(),
        authenticated_actor_id: "workflow_p06_core_domain".to_owned(),
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: authority_id.to_owned(),
        authority_owner: KEEPER_ID.to_owned(),
        visibility_label: visibility_label.to_owned(),
        visibility_subject: visibility_subject.to_owned(),
        data_subject_id: if visibility_subject == "not_applicable" {
            "not_applicable".to_owned()
        } else {
            visibility_subject.to_owned()
        },
        provenance_kind: provenance_kind.to_owned(),
        provenance_reference: format!("source_{suffix}"),
        provenance_recorded_by: actor_id.to_owned(),
        correlation_id: format!("correlation_{suffix}"),
        causation_id: format!("causation_{suffix}"),
        trace_id: format!("trace_{suffix}"),
        audit: PolicyAuditDraft {
            actor_id: "workflow_p06_core_domain".to_owned(),
            actor_origin: "workload".to_owned(),
            authentication_reference: "workflow_p06_core_domain".to_owned(),
            resource_type: resource_type.to_owned(),
            resource_id: stream_id.to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "workflow".to_owned(),
            openfga_decision_id: format!("lower_layer_fixture_openfga_{suffix}"),
            openfga_policy_revision: "lower-layer-formal-decision-fixture-v1".to_owned(),
            opa_decision_id: format!("lower_layer_fixture_opa_{suffix}"),
            opa_policy_revision: "lower-layer-formal-decision-fixture-v1".to_owned(),
        },
    }
}

fn campaign_metadata(campaign_id: &str, authority_id: &str, suffix: &str) -> CoreCommandMetadata {
    metadata(
        campaign_id,
        authority_id,
        KEEPER_ID,
        "human_keeper",
        campaign_id,
        "campaign",
        "campaign.create",
        0,
        suffix,
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    )
}

async fn create_campaign(
    repository: &CoreDomainRepository,
    campaign_id: &str,
    authority_id: &str,
    room_id: &str,
    suffix: &str,
) -> i64 {
    repository
        .create_campaign(
            &campaign_metadata(campaign_id, authority_id, suffix),
            &CreateCampaignRequest {
                campaign_id: campaign_id.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                title: format!("P06 Campaign {suffix}"),
                room_id: room_id.to_owned(),
                room_name: "Main table".to_owned(),
                created_at_unix_ms: NOW_MS,
                authority: authority(authority_id),
            },
        )
        .await
        .expect("create campaign and lock authority")
        .last_event_sequence
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn core_domain_schema_and_repository_are_event_backed_and_constrained() {
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
    let clock = Arc::new(TestClock(AtomicU64::new(NOW_MS)));
    let repository = CoreDomainRepository::new_with_clock(primary.clone(), store, clock.clone());

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
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        api_projection_privileges,
        "API projection role must be able to apply guarded rows but never delete them"
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
    let rejected = repository
        .create_campaign(
            &direct_user_write,
            &CreateCampaignRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                title: "Direct user write must fail".to_owned(),
                room_id: "room_direct_user_write_rejected".to_owned(),
                room_name: "Rejected room".to_owned(),
                created_at_unix_ms: NOW_MS,
                authority: authority(AUTHORITY_ID),
            },
        )
        .await;
    assert!(matches!(
        rejected,
        Err(CoreDomainRepositoryError::PolicyEvidenceMismatch)
    ));
    let events_after_direct_user_write: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store")
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(
        events_after_direct_user_write, 0,
        "direct business writes must not reach the canonical Event Store"
    );

    let recovery_campaign_id = "campaign_p06_projection_recovery";
    let recovery_authority_id = "authority_campaign_p06_projection_recovery_1";
    let recovery_metadata = campaign_metadata(
        recovery_campaign_id,
        recovery_authority_id,
        "campaign_projection_recovery",
    );
    let recovery_request = CreateCampaignRequest {
        campaign_id: recovery_campaign_id.to_owned(),
        owner_user_id: KEEPER_ID.to_owned(),
        title: "Recoverable projection Campaign".to_owned(),
        room_id: "room_p06_projection_recovery".to_owned(),
        room_name: "Recovery table".to_owned(),
        created_at_unix_ms: NOW_MS,
        authority: authority(recovery_authority_id),
    };
    sqlx::raw_sql(
        r#"
        CREATE FUNCTION public.reject_p06_projection_for_test()
        RETURNS trigger
        LANGUAGE plpgsql
        AS $$
        BEGIN
            RAISE EXCEPTION 'injected projection failure';
        END;
        $$;
        CREATE TRIGGER zz_reject_p06_projection_for_test
        BEFORE INSERT ON public.campaigns
        FOR EACH ROW EXECUTE FUNCTION public.reject_p06_projection_for_test();
        "#,
    )
    .execute(&primary)
    .await
    .expect("install session-local projection failure injection");
    assert!(matches!(
        repository
            .create_campaign(&recovery_metadata, &recovery_request)
            .await,
        Err(CoreDomainRepositoryError::Database("insert_campaign"))
    ));
    let recovery_event_count_after_failure: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'CampaignCreated'",
    )
    .bind(recovery_campaign_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    let recovery_projection_count_after_failure: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.campaigns WHERE campaign_id = $1")
            .bind(recovery_campaign_id)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(recovery_event_count_after_failure, 1);
    assert_eq!(
        recovery_projection_count_after_failure, 0,
        "a failed projection transaction must not leave partial business rows"
    );
    sqlx::raw_sql(
        r#"
        DROP TRIGGER zz_reject_p06_projection_for_test ON public.campaigns;
        DROP FUNCTION public.reject_p06_projection_for_test();
        "#,
    )
    .execute(&primary)
    .await
    .expect("remove session-local projection failure injection");

    let recovery_event_sequence: i64 = sqlx::query_scalar(
        "SELECT sequence FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'CampaignCreated'",
    )
    .bind(recovery_campaign_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    let mut exact_target_forgery = primary.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE trpg_api_service")
        .execute(&mut *exact_target_forgery)
        .await
        .unwrap();
    sqlx::query(
        r#"
        INSERT INTO public.campaign_memberships (
            campaign_id, user_id, role, granted_by, granted_at
        ) VALUES ($1, $2, 'HUMAN_KEEPER', $2, to_timestamp($3::double precision / 1000.0))
        "#,
    )
    .bind(recovery_campaign_id)
    .bind(KEEPER_ID)
    .bind(NOW_MS as i64)
    .execute(&mut *exact_target_forgery)
    .await
    .expect("seed attacker-controlled authority prerequisite inside rollback");
    sqlx::query(
        r#"
        INSERT INTO public.authority_contracts (
            contract_id, campaign_id, authority_mode, authority_owner,
            contract_version, ruleset_version, house_rules_version,
            scenario_version, prompt_version, agent_pack_version,
            tool_schema_version, safety_profile_version,
            ai_provider_snapshot, model_route_snapshot,
            character_sheet_template_version, created_at, locked, change_policy
        ) VALUES (
            $1, $2, 'HUMAN_KP', $3, 1, 'coc7-rules-1', 'house-rules-1',
            'scenario-1', 'prompt-1', 'agent-pack-1', 'tool-schema-1',
            'safety-1', 'not_applicable', 'not_applicable', 'coc7-sheet-1',
            to_timestamp($4::double precision / 1000.0), TRUE, 'FORK_ONLY'
        )
        "#,
    )
    .bind(recovery_authority_id)
    .bind(recovery_campaign_id)
    .bind(KEEPER_ID)
    .bind(NOW_MS as i64)
    .execute(&mut *exact_target_forgery)
    .await
    .expect("seed attacker-controlled authority inside rollback");
    let forged_exact_target = sqlx::query(
        r#"
        INSERT INTO public.campaigns (
            campaign_id, owner_user_id, authority_contract_id, title,
            state, version, created_at,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            $1, $2, $3, 'Forged exact-target contents', 'ACTIVE', 999,
            to_timestamp($4::double precision / 1000.0),
            'party_visible', 'not_applicable',
            'human_keeper_statement', 'source_campaign_projection_recovery',
            $2, $5
        )
        "#,
    )
    .bind(recovery_campaign_id)
    .bind(KEEPER_ID)
    .bind(recovery_authority_id)
    .bind(NOW_MS as i64)
    .bind(recovery_event_sequence)
    .execute(&mut *exact_target_forgery)
    .await;
    assert!(
        forged_exact_target.is_err(),
        "knowing an exact event target must not let the API database role consume its secret capability"
    );
    exact_target_forgery.rollback().await.unwrap();

    let recovered_projection = repository
        .create_campaign(&recovery_metadata, &recovery_request)
        .await
        .expect("exact retry recovers the projection from the durable canonical event");
    assert_eq!(
        recovered_projection.last_event_sequence,
        sqlx::query_scalar::<_, i64>(
            "SELECT last_event_sequence FROM public.campaigns WHERE campaign_id = $1",
        )
        .bind(recovery_campaign_id)
        .fetch_one(&primary)
        .await
        .unwrap()
    );
    let recovery_event_count_after_retry: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'CampaignCreated'",
    )
    .bind(recovery_campaign_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        recovery_event_count_after_retry, 1,
        "projection recovery must reuse, not duplicate, the canonical event"
    );

    let campaign_event_sequence = create_campaign(
        &repository,
        CAMPAIGN_ID,
        AUTHORITY_ID,
        "room_p06_schema",
        "campaign_create",
    )
    .await;
    let authority_row = sqlx::query(
        r#"
        SELECT authority_mode, authority_owner, locked, change_policy
          FROM public.authority_contracts
         WHERE campaign_id = $1
        "#,
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("load locked campaign authority");
    assert_eq!(authority_row.get::<String, _>("authority_mode"), "HUMAN_KP");
    assert_eq!(authority_row.get::<String, _>("authority_owner"), KEEPER_ID);
    assert!(authority_row.get::<bool, _>("locked"));
    assert_eq!(authority_row.get::<String, _>("change_policy"), "FORK_ONLY");
    assert!(
        sqlx::query(
            "UPDATE public.authority_contracts SET authority_owner = $1 WHERE campaign_id = $2"
        )
        .bind(OTHER_ID)
        .bind(CAMPAIGN_ID)
        .execute(&primary)
        .await
        .is_err(),
        "locked Authority Contract mutation must fail in PostgreSQL"
    );
    let protected_payload: bool = sqlx::query_scalar(
        "SELECT payload_json ? 'protected_payload' FROM public.event_store WHERE sequence = $1",
    )
    .bind(campaign_event_sequence)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        protected_payload,
        "canonical event payload must be encrypted"
    );

    let invite_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "invite_p06_player",
        "campaign_invite",
        "campaign.invite.issue",
        0,
        "invite_issue",
        "private_to_player",
        PLAYER_ID,
        "human_keeper_statement",
    );
    let invite_request = IssueInviteRequest {
        invite_id: "invite_p06_player".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        invited_user_id: PLAYER_ID.to_owned(),
        role: MembershipRole::Player,
        expires_at_unix_ms: NOW_MS + 60_000,
    };
    let issued = repository
        .issue_invite(&invite_metadata, &invite_request)
        .await
        .expect("issue non-persisted raw-token invitation");
    let issued_retry = repository
        .issue_invite(&invite_metadata, &invite_request)
        .await
        .expect("exact invite retry is idempotent");
    assert_eq!(issued_retry.raw_token, issued.raw_token);
    assert_eq!(
        issued_retry.persisted.last_event_sequence,
        issued.persisted.last_event_sequence
    );
    let invite_issue_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'CampaignInviteIssued'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        invite_issue_count, 1,
        "invite retry must not append an event"
    );
    let expired_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        PLAYER_ID,
        "investigator",
        "invite_p06_player",
        "campaign_invite",
        "campaign.invite.accept",
        1,
        "invite_accept_expired",
        "private_to_player",
        PLAYER_ID,
        "user_statement",
    );
    clock.0.store(NOW_MS + 60_000, Ordering::SeqCst);
    assert!(matches!(
        repository
            .accept_invite(
                &expired_metadata,
                &AcceptInviteRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    invite_id: "invite_p06_player".to_owned(),
                    accepting_user_id: PLAYER_ID.to_owned(),
                    raw_token: issued.raw_token.clone(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Domain(_))
    ));
    clock.0.store(NOW_MS + 1_000, Ordering::SeqCst);
    let wrong_subject_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        OTHER_ID,
        "investigator",
        "invite_p06_player",
        "campaign_invite",
        "campaign.invite.accept",
        1,
        "invite_accept_wrong_subject",
        "private_to_player",
        OTHER_ID,
        "user_statement",
    );
    assert!(matches!(
        repository
            .accept_invite(
                &wrong_subject_metadata,
                &AcceptInviteRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    invite_id: "invite_p06_player".to_owned(),
                    accepting_user_id: OTHER_ID.to_owned(),
                    raw_token: issued.raw_token.clone(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Domain(_))
    ));
    let accept_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        PLAYER_ID,
        "investigator",
        "invite_p06_player",
        "campaign_invite",
        "campaign.invite.accept",
        1,
        "invite_accept",
        "private_to_player",
        PLAYER_ID,
        "user_statement",
    );
    let accept_request = AcceptInviteRequest {
        campaign_id: CAMPAIGN_ID.to_owned(),
        invite_id: "invite_p06_player".to_owned(),
        accepting_user_id: PLAYER_ID.to_owned(),
        raw_token: issued.raw_token,
    };
    let accepted = repository
        .accept_invite(&accept_metadata, &accept_request)
        .await
        .expect("accept valid invite into durable membership");
    clock.0.store(NOW_MS + 60_000, Ordering::SeqCst);
    let accepted_retry = repository
        .accept_invite(&accept_metadata, &accept_request)
        .await
        .expect("exact retry remains idempotent after the invite expires");
    assert_eq!(
        accepted_retry.last_event_sequence,
        accepted.last_event_sequence
    );
    clock.0.store(NOW_MS + 1_000, Ordering::SeqCst);

    let conflict_invite_id = "invite_p07_membership_conflict";
    let conflict_issue_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        conflict_invite_id,
        "campaign_invite",
        "campaign.invite.issue",
        0,
        "invite_conflict_issue",
        "private_to_player",
        OTHER_ID,
        "human_keeper_statement",
    );
    let conflict_invite = repository
        .issue_invite(
            &conflict_issue_metadata,
            &IssueInviteRequest {
                invite_id: conflict_invite_id.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                invited_user_id: OTHER_ID.to_owned(),
                role: MembershipRole::Player,
                expires_at_unix_ms: NOW_MS + 120_000,
            },
        )
        .await
        .expect("issue invite used by the atomic conflict probe");
    sqlx::query(
        r#"
        INSERT INTO public.campaign_memberships (
            campaign_id, user_id, role, granted_by, granted_at, revoked_at
        ) VALUES ($1, $2, 'SPECTATOR', $3, to_timestamp($4 / 1000.0),
                  to_timestamp($4 / 1000.0))
        "#,
    )
    .bind(CAMPAIGN_ID)
    .bind(OTHER_ID)
    .bind(KEEPER_ID)
    .bind(NOW_MS as f64)
    .execute(&primary)
    .await
    .expect("seed a revoked conflicting membership");
    let conflict_accept_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        OTHER_ID,
        "investigator",
        conflict_invite_id,
        "campaign_invite",
        "campaign.invite.accept",
        1,
        "invite_conflict_accept",
        "private_to_player",
        OTHER_ID,
        "user_statement",
    );
    assert!(matches!(
        repository
            .accept_invite(
                &conflict_accept_metadata,
                &AcceptInviteRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    invite_id: conflict_invite_id.to_owned(),
                    accepting_user_id: OTHER_ID.to_owned(),
                    raw_token: conflict_invite.raw_token,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Canonical(_))
    ));
    let consumed_event_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = $2 \
           AND event_type = 'CampaignInviteAccepted'",
    )
    .bind(CAMPAIGN_ID)
    .bind(conflict_invite_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    let formal_commit_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.formal_commits WHERE commit_id = $1")
            .bind(&conflict_accept_metadata.commit_id)
            .fetch_one(&primary)
            .await
            .unwrap();
    let conflict_membership = sqlx::query(
        "SELECT role, revoked_at IS NOT NULL AS revoked \
         FROM public.campaign_memberships \
         WHERE campaign_id = $1 AND user_id = $2",
    )
    .bind(CAMPAIGN_ID)
    .bind(OTHER_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(consumed_event_count, 0);
    assert_eq!(formal_commit_count, 0);
    assert_eq!(conflict_membership.get::<String, _>("role"), "SPECTATOR");
    assert!(conflict_membership.get::<bool, _>("revoked"));

    let character_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        PLAYER_ID,
        "investigator",
        "character_p06_player",
        "character",
        "character.create",
        0,
        "character_create",
        "private_to_player",
        PLAYER_ID,
        "user_statement",
    );
    repository
        .create_character(
            &character_metadata,
            &CreateCharacterRequest {
                character_id: "character_p06_player".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: PLAYER_ID.to_owned(),
                display_name: "Evelyn Hart".to_owned(),
                sheet_version_id: "sheet_p06_player_v1".to_owned(),
                sheet_json:
                    r#"{"name":"Evelyn Hart","age":31,"ruleset":"coc7","skills":{"Library Use":70}}"#
                        .to_owned(),
            },
        )
        .await
        .expect("create character and initial sheet projection");
    let submit_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        PLAYER_ID,
        "investigator",
        "character_p06_player",
        "character",
        "character.submit",
        1,
        "character_submit",
        "private_to_player",
        PLAYER_ID,
        "user_statement",
    );
    let submitted = repository
        .submit_character(&submit_metadata, CAMPAIGN_ID, "character_p06_player")
        .await
        .expect("submit character");
    let submitted_retry = repository
        .submit_character(&submit_metadata, CAMPAIGN_ID, "character_p06_player")
        .await
        .expect("exact character submission retry is idempotent");
    assert_eq!(
        submitted_retry.last_event_sequence,
        submitted.last_event_sequence
    );
    let review_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "character_p06_player",
        "character",
        "character.review_initial",
        2,
        "character_review",
        "private_to_player",
        PLAYER_ID,
        "human_keeper_statement",
    );
    let approved = repository
        .approve_character_initial_version(&review_metadata, CAMPAIGN_ID, "character_p06_player")
        .await
        .expect("approve and lock initial character version");
    let approved_retry = repository
        .approve_character_initial_version(&review_metadata, CAMPAIGN_ID, "character_p06_player")
        .await
        .expect("exact character approval retry is idempotent");
    assert_eq!(
        approved_retry.last_event_sequence,
        approved.last_event_sequence
    );
    repository
        .create_character(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "character_p08_keeper_private",
                "character",
                "character.create",
                0,
                "character_create_keeper_private",
                "keeper_only",
                "not_applicable",
                "human_keeper_statement",
            ),
            &CreateCharacterRequest {
                character_id: "character_p08_keeper_private".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                display_name: "P08 Keeper Private Sentinel".to_owned(),
                sheet_version_id: "sheet_p08_keeper_private_v1".to_owned(),
                sheet_json: r#"{"keeper_only_fork_sentinel":true}"#.to_owned(),
            },
        )
        .await
        .expect("seed a keeper-only character that a default fork must exclude");
    let character_state = sqlx::query(
        r#"
        SELECT character.state, character.initial_version_locked,
               sheet.locked AS sheet_locked
          FROM public.characters AS character
          JOIN public.character_sheet_versions AS sheet
            ON sheet.character_id = character.character_id
         WHERE character.character_id = 'character_p06_player'
           AND sheet.version = 1
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(character_state.get::<String, _>("state"), "APPROVED");
    assert!(character_state.get::<bool, _>("initial_version_locked"));
    assert!(character_state.get::<bool, _>("sheet_locked"));

    let mut forged_projection = primary.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE trpg_api_service")
        .execute(&mut *forged_projection)
        .await
        .unwrap();
    let forged = sqlx::query(
        r#"
        INSERT INTO public.characters (
            character_id, campaign_id, owner_user_id, display_name,
            state, current_sheet_version, initial_version_locked, version,
            visibility_label, visibility_subject,
            provenance_kind, provenance_reference, provenance_recorded_by,
            last_event_sequence
        ) VALUES (
            'character_forged_by_reused_event', $1, $2, 'Forged',
            'APPROVED', 1, TRUE, 999,
            'private_to_player', $2,
            'human_keeper_statement', 'source_character_review', $3, $4
        )
        "#,
    )
    .bind(CAMPAIGN_ID)
    .bind(PLAYER_ID)
    .bind(KEEPER_ID)
    .bind(approved.last_event_sequence)
    .execute(&mut *forged_projection)
    .await;
    assert!(
        forged.is_err(),
        "a legitimate event for one Character must not authorize another projection row"
    );
    forged_projection.rollback().await.unwrap();

    let mut forbidden_delete = primary.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE trpg_api_service")
        .execute(&mut *forbidden_delete)
        .await
        .unwrap();
    let deleted =
        sqlx::query("DELETE FROM public.characters WHERE character_id = 'character_p06_player'")
            .execute(&mut *forbidden_delete)
            .await;
    assert!(
        deleted.is_err(),
        "the API projection role must not delete canonical projections"
    );
    forbidden_delete.rollback().await.unwrap();

    let tutorial = parse_scenario_yaml(include_str!(
        "../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml"
    ))
    .expect("parse raw Tutorial Scenario");
    repository
        .import_scenario(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "scenario_p06_tutorial",
                "scenario",
                "scenario.import",
                0,
                "scenario_import",
                "keeper_only",
                "not_applicable",
                "imported_source",
            ),
            &ImportScenarioRequest {
                scenario_id: "scenario_p06_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                ruleset_id: tutorial.ruleset_id,
                format_version: tutorial.format_version,
                content_hash: tutorial.content_hash,
                document_json: tutorial.canonical_json,
            },
        )
        .await
        .expect("persist validated Tutorial Scenario");

    repository
        .start_session(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p06_schema",
                "session",
                "session.start",
                0,
                "session_start",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: "session_p06_schema".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p06_schema".to_owned(),
                scenario_id: "scenario_p06_tutorial".to_owned(),
                scene_id: "scene_p06_front".to_owned(),
                scene_key: "scene_archive_front".to_owned(),
                scene_name: "灰港市政档案室前厅".to_owned(),
                started_at_unix_ms: NOW_MS + 2_000,
            },
        )
        .await
        .expect("start session with active scene");
    repository
        .switch_scene(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p06_schema",
                "session",
                "scene.switch",
                1,
                "scene_switch",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &SwitchSceneRequest {
                session_id: "session_p06_schema".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                next_scene_id: "scene_p06_basement".to_owned(),
                next_scene_key: "scene_basement".to_owned(),
                next_scene_name: "地下盐窖".to_owned(),
                switched_at_unix_ms: NOW_MS + 3_000,
            },
        )
        .await
        .expect("switch active scene");
    repository
        .change_session_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p06_schema",
                "session",
                "session.pause",
                2,
                "session_pause",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            "session_p06_schema",
            SessionState::Paused,
            NOW_MS + 4_000,
        )
        .await
        .expect("pause active session");
    repository
        .change_session_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p06_schema",
                "session",
                "session.resume",
                3,
                "session_resume",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            "session_p06_schema",
            SessionState::Active,
            NOW_MS + 5_000,
        )
        .await
        .expect("resume paused session");

    let mut combat = CombatState::start(
        "combat_p08_schema",
        vec![
            CombatantState::new(
                "character_p06_player",
                70,
                10,
                1,
                CombatSkillTargets::new(45, 35, 40).unwrap(),
            )
            .unwrap(),
            CombatantState::new(
                "npc_marta",
                80,
                8,
                0,
                CombatSkillTargets::new(60, 80, 40).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    repository
        .record_combat_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_schema",
                "combat_state",
                "combat.state.start",
                0,
                "combat_p08_start",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: combat.persistence_json().unwrap(),
                attacker_roll: None,
                defender_roll: None,
                damage_roll: None,
                medical_roll: None,
            },
        )
        .await
        .expect("persist started combat aggregate");
    let combat_events_before_forgery: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = 'combat_p08_schema'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let mut mismatched_evidence_state = combat.clone();
    let recorded_attack = percentile_with_result(80, true);
    let recorded_damage = damage_with_value(1, 6, 5, 6);
    mismatched_evidence_state
        .apply_damage(
            "character_p06_player",
            CombatActionKind::Firearm,
            CombatDefense::None,
            &recorded_attack,
            None,
            &recorded_damage,
        )
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
                    1,
                    "combat_p08_mismatched_roll",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordCombatStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p06_schema".to_owned(),
                    state_json: mismatched_evidence_state.persistence_json().unwrap(),
                    attacker_roll: Some(percentile_with_result(80, true)),
                    defender_roll: None,
                    damage_roll: Some(recorded_damage),
                    medical_roll: None,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "combat_roll_evidence"
        ))
    ));
    let mut foreign_lineage = CombatState::start(
        "combat_p08_schema",
        vec![
            CombatantState::new(
                "character_p06_player",
                99,
                30,
                20,
                CombatSkillTargets::new(99, 99, 99).unwrap(),
            )
            .unwrap(),
            CombatantState::new(
                "npc_marta",
                100,
                30,
                20,
                CombatSkillTargets::new(100, 100, 100).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let foreign_attack = percentile_with_result(100, true);
    let foreign_damage = damage_with_value(1, 6, 5, 11);
    foreign_lineage
        .apply_damage(
            "character_p06_player",
            CombatActionKind::Firearm,
            CombatDefense::None,
            &foreign_attack,
            None,
            &foreign_damage,
        )
        .unwrap();
    let foreign_state_json = foreign_lineage.persistence_json().unwrap();
    let foreign_evidence_validation = validate_combat_server_roll_evidence(
        &foreign_state_json,
        Some(&foreign_attack),
        None,
        Some(&foreign_damage),
        None,
    );
    assert!(
        foreign_evidence_validation.is_ok(),
        "foreign evidence should be internally consistent before lineage validation: \
         {foreign_evidence_validation:?}; state={foreign_state_json}"
    );
    let foreign_lineage_result = repository
        .record_combat_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_schema",
                "combat_state",
                "combat.state.damage",
                1,
                "combat_p08_foreign_lineage",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: foreign_state_json,
                attacker_roll: Some(foreign_attack),
                defender_roll: None,
                damage_roll: Some(foreign_damage),
                medical_roll: None,
            },
        )
        .await;
    assert!(
        matches!(
            foreign_lineage_result,
            Err(CoreDomainRepositoryError::InvalidInput("combat_transition"))
        ),
        "unexpected foreign-lineage error: {foreign_lineage_result:?}"
    );
    let combat_events_after_forgery: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = 'combat_p08_schema'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        combat_events_after_forgery, combat_events_before_forgery,
        "a same-ID aggregate from another lineage must be rejected before Event Store append"
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
            &first_damage_roll,
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
                1,
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
    let later_attack = percentile_with_result(60, true);
    let later_damage_roll = damage_with_value(1, 6, 0, 1);
    let later_damage = combat
        .apply_damage(
            "character_p06_player",
            CombatActionKind::Melee,
            CombatDefense::None,
            &later_attack,
            None,
            &later_damage_roll,
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
                2,
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
    let fight_back_attack = percentile_with_level(60, SuccessLevel::Regular);
    let fight_back_defense = percentile_with_level(45, SuccessLevel::Hard);
    let fight_back_damage = damage_with_value(1, 6, 0, 1);
    combat
        .apply_damage(
            "character_p06_player",
            CombatActionKind::Melee,
            CombatDefense::FightBack,
            &fight_back_attack,
            Some(&fight_back_defense),
            &fight_back_damage,
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
                    3,
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
                3,
                "combat_p08_fight_back",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: fight_back_state,
                attacker_roll: Some(fight_back_attack),
                defender_roll: Some(fight_back_defense),
                damage_roll: Some(fight_back_damage),
                medical_roll: None,
            },
        )
        .await
        .expect("persist a verified fight-back counterattack");
    combat.end().unwrap();
    assert_eq!(combat.status(), CombatStatus::Ended);
    repository
        .record_combat_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_schema",
                "combat_state",
                "combat.state.end",
                4,
                "combat_p08_end",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: combat.persistence_json().unwrap(),
                attacker_roll: None,
                defender_roll: None,
                damage_roll: None,
                medical_roll: None,
            },
        )
        .await
        .expect("persist terminal combat state");

    let mut chase = ChaseState::start(
        "chase_p08_schema",
        vec![
            ChaseParticipant::new("character_p06_player", ChaseRole::Quarry, 8).unwrap(),
            ChaseParticipant::new("npc_marta", ChaseRole::Pursuer, 8).unwrap(),
        ],
        1,
    )
    .unwrap();
    repository
        .record_chase_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "chase_p08_schema",
                "chase_state",
                "chase.state.start",
                0,
                "chase_p08_start",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordChaseStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: chase.persistence_json().unwrap(),
                participant_rolls: Vec::new(),
            },
        )
        .await
        .expect("persist started chase aggregate");
    let chase_events_before_forgery: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = 'chase_p08_schema'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let mut mismatched_chase = chase.clone();
    let recorded_chase_rolls = vec![
        percentile_with_result(40, false),
        percentile_with_result(40, true),
    ];
    mismatched_chase
        .advance(&recorded_chase_rolls, None)
        .unwrap();
    assert!(matches!(
        repository
            .record_chase_state(
                &metadata(
                    CAMPAIGN_ID,
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "chase_p08_schema",
                    "chase_state",
                    "chase.state.advance",
                    1,
                    "chase_p08_mismatched_roll",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordChaseStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p06_schema".to_owned(),
                    state_json: mismatched_chase.persistence_json().unwrap(),
                    participant_rolls: vec![
                        percentile_with_result(40, false),
                        percentile_with_result(40, true),
                    ],
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "chase_roll_evidence"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND stream_id = 'chase_p08_schema'",
        )
        .bind(CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        chase_events_before_forgery,
        "mismatched chase roll evidence must fail before canonical append"
    );
    let chase_rolls = vec![
        percentile_with_result(40, false),
        percentile_with_result(40, true),
    ];
    chase.advance(&chase_rolls, None).unwrap();
    assert_eq!(chase.status(), ChaseStatus::Caught);
    repository
        .record_chase_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "chase_p08_schema",
                "chase_state",
                "chase.state.advance",
                1,
                "chase_p08_caught",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordChaseStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                state_json: chase.persistence_json().unwrap(),
                participant_rolls: chase_rolls,
            },
        )
        .await
        .expect("persist terminal chase aggregate");
    assert!(chase
        .advance(
            &[
                percentile_with_result(40, true),
                percentile_with_result(40, false),
            ],
            None,
        )
        .is_err());

    repository
        .change_session_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p06_schema",
                "session",
                "session.end",
                4,
                "session_end",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            "session_p06_schema",
            SessionState::Ended,
            NOW_MS + 6_000,
        )
        .await
        .expect("end resumed session");

    repository
        .record_ending(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "ending_event_p08_schema",
                "ending",
                "ending.record",
                0,
                "ending_p08_record",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordEndingRequest {
                ending_event_id: "ending_event_p08_schema".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p06_schema".to_owned(),
                ending_id: "ending_expose_marta".to_owned(),
                summary: "The investigators expose Marta and preserve the archive.".to_owned(),
                ended_at_unix_ms: NOW_MS + 7_000,
            },
        )
        .await
        .expect("append tutorial ending event");
    let growth_roll = server_roll_skill_growth(70).unwrap();
    let growth_outcome = *growth_roll.outcome();
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
                source_sheet_version_id: "sheet_p06_player_v1".to_owned(),
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
    assert_eq!(persisted_combat.1, 5);
    assert_eq!(
        persisted_combat
            .2
            .pointer("/participants/0/condition")
            .and_then(serde_json::Value::as_str),
        Some("MAJOR_WOUND")
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
    assert_eq!(persisted_growth.0, 2);
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

    create_campaign(
        &repository,
        CHILD_CAMPAIGN_ID,
        CHILD_AUTHORITY_ID,
        "room_p06_fork_child",
        "child_campaign_create",
    )
    .await;
    let snapshot = repository
        .preview_campaign_fork(CAMPAIGN_ID, "session_p06_schema", KEEPER_ID)
        .await
        .expect("compute a canonical public-only fork snapshot");
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
    assert_eq!(
        child_state_counts.0, 1,
        "fork must materialize the world/scenario scope"
    );
    assert_eq!(child_state_counts.1, 1);
    assert_eq!(child_state_counts.2, 1);
    assert_eq!(child_state_counts.3, 2);
    assert_eq!(child_state_counts.4, 1);
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
    let child_projection_before: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'fork', (SELECT to_jsonb(fork) FROM public.campaign_forks AS fork
                      WHERE fork.fork_id = 'fork_p06_schema'),
            'manifest', (SELECT to_jsonb(manifest)
                           FROM public.campaign_fork_materializations AS manifest
                          WHERE manifest.fork_id = 'fork_p06_schema'),
            'scenario', (SELECT to_jsonb(scenario) FROM public.scenarios AS scenario
                          WHERE scenario.campaign_id = $1),
            'characters', (SELECT jsonb_agg(to_jsonb(character)
                                            ORDER BY character.character_id)
                             FROM public.characters AS character
                            WHERE character.campaign_id = $1),
            'sheets', (SELECT jsonb_agg(to_jsonb(sheet)
                                        ORDER BY sheet.sheet_version_id)
                         FROM public.character_sheet_versions AS sheet
                        WHERE sheet.campaign_id = $1),
            'sessions', (SELECT jsonb_agg(to_jsonb(session)
                                          ORDER BY session.session_id)
                           FROM core_domain.sessions AS session
                          WHERE session.campaign_id = $1),
            'scenes', (SELECT jsonb_agg(to_jsonb(scene) ORDER BY scene.scene_id)
                         FROM public.scenes AS scene
                        WHERE scene.campaign_id = $1),
            'public_events', (SELECT jsonb_agg(to_jsonb(public_event)
                                               ORDER BY public_event.source_event_sequence)
                                FROM public.campaign_fork_public_events AS public_event
                               WHERE public_event.campaign_id = $1),
            'clues', (SELECT jsonb_agg(to_jsonb(clue) ORDER BY clue.fork_clue_id)
                        FROM public.campaign_fork_clues AS clue
                       WHERE clue.campaign_id = $1),
            'npc_states', (SELECT jsonb_agg(to_jsonb(npc) ORDER BY npc.npc_state_id)
                             FROM public.campaign_fork_npc_states AS npc
                            WHERE npc.campaign_id = $1),
            'combat', (SELECT jsonb_agg(to_jsonb(combat) ORDER BY combat.combat_id)
                         FROM public.combat_states AS combat
                        WHERE combat.campaign_id = $1),
            'chase', (SELECT jsonb_agg(to_jsonb(chase) ORDER BY chase.chase_id)
                        FROM public.chase_states AS chase
                       WHERE chase.campaign_id = $1),
            'endings', (SELECT jsonb_agg(to_jsonb(ending)
                                         ORDER BY ending.ending_event_id)
                          FROM public.ending_events AS ending
                         WHERE ending.campaign_id = $1)
        )
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let child_events_before_replay: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CHILD_CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    let mut remove_child_projection = primary.begin().await.unwrap();
    sqlx::query("SET CONSTRAINTS ALL DEFERRED")
        .execute(&mut *remove_child_projection)
        .await
        .unwrap();
    for statement in [
        "DELETE FROM public.ending_events WHERE campaign_id = $1",
        "DELETE FROM public.chase_states WHERE campaign_id = $1",
        "DELETE FROM public.combat_states WHERE campaign_id = $1",
        "DELETE FROM public.campaign_fork_npc_states WHERE campaign_id = $1",
        "DELETE FROM public.campaign_fork_clues WHERE campaign_id = $1",
        "DELETE FROM public.campaign_fork_public_events WHERE campaign_id = $1",
        "DELETE FROM public.campaign_fork_materializations WHERE campaign_id = $1",
        "DELETE FROM public.character_sheet_versions WHERE campaign_id = $1",
        "DELETE FROM public.characters WHERE campaign_id = $1",
        "DELETE FROM public.scenes WHERE campaign_id = $1",
        "DELETE FROM core_domain.sessions WHERE campaign_id = $1",
        "DELETE FROM public.scenarios WHERE campaign_id = $1",
        "DELETE FROM public.campaign_forks WHERE campaign_id = $1",
    ] {
        sqlx::query(statement)
            .bind(CHILD_CAMPAIGN_ID)
            .execute(&mut *remove_child_projection)
            .await
            .unwrap();
    }
    remove_child_projection.commit().await.unwrap();
    let rebuilt_child = repository
        .rebuild_p08_projections(CHILD_CAMPAIGN_ID)
        .await
        .expect("rebuild the entire child fork state solely from canonical P08 events");
    assert_eq!(rebuilt_child.replayed_events, child_event_types.len());
    assert_eq!(rebuilt_child.campaign_forks, 1);
    assert_eq!(rebuilt_child.fork_materializations, 1);
    assert_eq!(
        rebuilt_child.fork_public_events,
        snapshot_scope_len("public_events")
    );
    assert_eq!(
        rebuilt_child.fork_clues,
        snapshot_scope_len("discovered_clues")
    );
    assert_eq!(
        rebuilt_child.fork_npc_states,
        snapshot_scope_len("npc_state")
    );
    assert_eq!(
        rebuilt_child.combat_states,
        snapshot_scope_len("combat_state")
    );
    assert_eq!(
        rebuilt_child.chase_states,
        snapshot_scope_len("chase_state")
    );
    assert_eq!(
        rebuilt_child.ending_events,
        snapshot_scope_len("conclusion_state")
    );
    let child_projection_after: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'fork', (SELECT to_jsonb(fork) FROM public.campaign_forks AS fork
                      WHERE fork.fork_id = 'fork_p06_schema'),
            'manifest', (SELECT to_jsonb(manifest)
                           FROM public.campaign_fork_materializations AS manifest
                          WHERE manifest.fork_id = 'fork_p06_schema'),
            'scenario', (SELECT to_jsonb(scenario) FROM public.scenarios AS scenario
                          WHERE scenario.campaign_id = $1),
            'characters', (SELECT jsonb_agg(to_jsonb(character)
                                            ORDER BY character.character_id)
                             FROM public.characters AS character
                            WHERE character.campaign_id = $1),
            'sheets', (SELECT jsonb_agg(to_jsonb(sheet)
                                        ORDER BY sheet.sheet_version_id)
                         FROM public.character_sheet_versions AS sheet
                        WHERE sheet.campaign_id = $1),
            'sessions', (SELECT jsonb_agg(to_jsonb(session)
                                          ORDER BY session.session_id)
                           FROM core_domain.sessions AS session
                          WHERE session.campaign_id = $1),
            'scenes', (SELECT jsonb_agg(to_jsonb(scene) ORDER BY scene.scene_id)
                         FROM public.scenes AS scene
                        WHERE scene.campaign_id = $1),
            'public_events', (SELECT jsonb_agg(to_jsonb(public_event)
                                               ORDER BY public_event.source_event_sequence)
                                FROM public.campaign_fork_public_events AS public_event
                               WHERE public_event.campaign_id = $1),
            'clues', (SELECT jsonb_agg(to_jsonb(clue) ORDER BY clue.fork_clue_id)
                        FROM public.campaign_fork_clues AS clue
                       WHERE clue.campaign_id = $1),
            'npc_states', (SELECT jsonb_agg(to_jsonb(npc) ORDER BY npc.npc_state_id)
                             FROM public.campaign_fork_npc_states AS npc
                            WHERE npc.campaign_id = $1),
            'combat', (SELECT jsonb_agg(to_jsonb(combat) ORDER BY combat.combat_id)
                         FROM public.combat_states AS combat
                        WHERE combat.campaign_id = $1),
            'chase', (SELECT jsonb_agg(to_jsonb(chase) ORDER BY chase.chase_id)
                        FROM public.chase_states AS chase
                       WHERE chase.campaign_id = $1),
            'endings', (SELECT jsonb_agg(to_jsonb(ending)
                                         ORDER BY ending.ending_event_id)
                          FROM public.ending_events AS ending
                         WHERE ending.campaign_id = $1)
        )
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        child_projection_after, child_projection_before,
        "fork replay must reproduce byte-equivalent child read-model facts"
    );
    let child_events_after_replay: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CHILD_CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(
        child_events_after_replay, child_events_before_replay,
        "fork projection replay must never append or rewrite canonical history"
    );
    let source_after_fork = repository
        .preview_campaign_fork(CAMPAIGN_ID, "session_p06_schema", KEEPER_ID)
        .await
        .expect("recompute source snapshot after creating child");
    assert_eq!(
        source_after_fork, snapshot,
        "fork creation must not mutate the source campaign snapshot"
    );

    let reconsideration_request_metadata = metadata(
        CAMPAIGN_ID,
        AUTHORITY_ID,
        PLAYER_ID,
        "investigator",
        "reconsideration_p06_schema",
        "reconsideration",
        "reconsideration.request",
        0,
        "reconsideration_request",
        "party_visible",
        "not_applicable",
        "user_statement",
    );
    let reconsideration_request = RequestReconsiderationRequest {
        reconsideration_id: "reconsideration_p06_schema".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        original_event_sequence: campaign_event_sequence,
        requested_by: PLAYER_ID.to_owned(),
        reason: "Review the opening ruling".to_owned(),
    };
    let reconsideration_requested = repository
        .request_reconsideration(&reconsideration_request_metadata, &reconsideration_request)
        .await
        .expect("append reconsideration request");
    let reconsideration_retry = repository
        .request_reconsideration(&reconsideration_request_metadata, &reconsideration_request)
        .await
        .expect("exact repeated reconsideration request is idempotent");
    assert_eq!(
        reconsideration_retry.last_event_sequence,
        reconsideration_requested.last_event_sequence
    );
    repository
        .review_reconsideration(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p06_schema",
                "reconsideration",
                "reconsideration.review",
                1,
                "reconsideration_review",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ReviewReconsiderationRequest {
                reconsideration_id: "reconsideration_p06_schema".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                review_event_id: "review_event_p06_schema".to_owned(),
                review_summary: "The original ruling omitted a material clue".to_owned(),
            },
        )
        .await
        .expect("append review event without rewriting the original event");
    repository
        .resolve_reconsideration(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p06_schema",
                "reconsideration",
                "reconsideration.resolve",
                2,
                "reconsideration_resolution",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ResolveReconsiderationRequest {
                reconsideration_id: "reconsideration_p06_schema".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                resolution_event_id: "resolution_event_p06_schema".to_owned(),
                outcome: ReconsiderationOutcome::Corrected,
                resolution: "Append a corrected ruling that includes the clue".to_owned(),
                corrected_event_type: Some("RulingCorrected".to_owned()),
                corrected_payload_json: Some(
                    r#"{"ruling":"clue admitted","supersedes_sequence":1}"#.to_owned(),
                ),
            },
        )
        .await
        .expect("append a correction event and resolve reconsideration");
    let reconsideration = sqlx::query(
        r#"
        SELECT state, outcome, version,
               jsonb_array_length(event_chain) AS chain_length
          FROM public.reconsiderations
         WHERE reconsideration_id = 'reconsideration_p06_schema'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(reconsideration.get::<String, _>("state"), "RESOLVED");
    assert_eq!(reconsideration.get::<String, _>("outcome"), "CORRECTED");
    assert_eq!(reconsideration.get::<i64, _>("version"), 3);
    assert_eq!(reconsideration.get::<i32, _>("chain_length"), 3);
    let original_event_still_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.event_store WHERE sequence = $1)")
            .bind(campaign_event_sequence)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert!(
        original_event_still_exists,
        "a corrected reconsideration must never delete its original event"
    );

    repository
        .request_reconsideration(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "reconsideration_p08_upheld",
                "reconsideration",
                "reconsideration.request",
                0,
                "reconsideration_upheld_request",
                "party_visible",
                "not_applicable",
                "user_statement",
            ),
            &RequestReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_upheld".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                original_event_sequence: campaign_event_sequence,
                requested_by: PLAYER_ID.to_owned(),
                reason: "Request a second review of the opening ruling".to_owned(),
            },
        )
        .await
        .expect("append reconsideration request for upheld path");
    repository
        .review_reconsideration(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_upheld",
                "reconsideration",
                "reconsideration.review",
                1,
                "reconsideration_upheld_review",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ReviewReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_upheld".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                review_event_id: "review_event_p08_upheld".to_owned(),
                review_summary: "The original evidence and rule citation are complete".to_owned(),
            },
        )
        .await
        .expect("append review for upheld path");
    repository
        .resolve_reconsideration(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_upheld",
                "reconsideration",
                "reconsideration.resolve",
                2,
                "reconsideration_upheld_resolution",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ResolveReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_upheld".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                resolution_event_id: "resolution_event_p08_upheld".to_owned(),
                outcome: ReconsiderationOutcome::Upheld,
                resolution: "Original ruling upheld after review".to_owned(),
                corrected_event_type: None,
                corrected_payload_json: None,
            },
        )
        .await
        .expect("append upheld resolution without a correction payload");
    let upheld_outcome: String = sqlx::query_scalar(
        "SELECT outcome FROM public.reconsiderations \
         WHERE reconsideration_id = 'reconsideration_p08_upheld'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(upheld_outcome, "UPHELD");

    let p08_projection_before: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'combat', (SELECT to_jsonb(combat) FROM public.combat_states AS combat
                        WHERE combat.campaign_id = $1),
            'chase', (SELECT to_jsonb(chase) FROM public.chase_states AS chase
                       WHERE chase.campaign_id = $1),
            'ending', (SELECT to_jsonb(ending) FROM public.ending_events AS ending
                        WHERE ending.campaign_id = $1),
            'growth', (SELECT to_jsonb(growth) FROM public.growth_events AS growth
                        WHERE growth.campaign_id = $1),
            'growth_sheet', (
                SELECT to_jsonb(sheet)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.sheet_version_id = 'sheet_p06_player_v2'
            ),
            'character', (
                SELECT to_jsonb(character)
                  FROM public.characters AS character
                 WHERE character.character_id = 'character_p06_player'
            ),
            'reconsiderations', (
                SELECT jsonb_agg(to_jsonb(reconsideration)
                                 ORDER BY reconsideration.reconsideration_id)
                  FROM public.reconsiderations AS reconsideration
                 WHERE reconsideration.campaign_id = $1
            )
        )
        "#,
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let event_count_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    let approval_event_sequence: i64 = sqlx::query_scalar(
        "SELECT sequence FROM public.event_store \
         WHERE campaign_id = $1 AND stream_id = 'character_p06_player' \
           AND event_type = 'CharacterInitialVersionApproved'",
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    let mut remove_p08_projection = primary.begin().await.unwrap();
    sqlx::query("SET CONSTRAINTS ALL DEFERRED")
        .execute(&mut *remove_p08_projection)
        .await
        .unwrap();
    sqlx::query("DELETE FROM public.growth_events WHERE campaign_id = $1")
        .bind(CAMPAIGN_ID)
        .execute(&mut *remove_p08_projection)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE public.characters DISABLE TRIGGER characters_event_guard")
        .execute(&mut *remove_p08_projection)
        .await
        .unwrap();
    sqlx::query(
        r#"
        UPDATE public.characters AS character
           SET current_sheet_version = 1,
               version = character.version - 1,
               visibility_label = event.visibility_label::core_domain.visibility_label,
               visibility_subject = event.visibility_subject,
               provenance_kind = event.fact_provenance_kind::core_domain.provenance_kind,
               provenance_reference = event.fact_provenance_reference,
               provenance_recorded_by = event.fact_recorded_by,
               last_event_sequence = event.sequence
          FROM public.event_store AS event
         WHERE character.character_id = 'character_p06_player'
           AND event.sequence = $1
        "#,
    )
    .bind(approval_event_sequence)
    .execute(&mut *remove_p08_projection)
    .await
    .unwrap();
    sqlx::query("ALTER TABLE public.characters ENABLE TRIGGER characters_event_guard")
        .execute(&mut *remove_p08_projection)
        .await
        .unwrap();
    sqlx::query(
        "DELETE FROM public.character_sheet_versions \
         WHERE sheet_version_id = 'sheet_p06_player_v2'",
    )
    .execute(&mut *remove_p08_projection)
    .await
    .unwrap();
    for statement in [
        "DELETE FROM public.ending_events WHERE campaign_id = $1",
        "DELETE FROM public.reconsiderations WHERE campaign_id = $1",
        "DELETE FROM public.combat_states WHERE campaign_id = $1",
        "DELETE FROM public.chase_states WHERE campaign_id = $1",
    ] {
        sqlx::query(statement)
            .bind(CAMPAIGN_ID)
            .execute(&mut *remove_p08_projection)
            .await
            .unwrap();
    }
    remove_p08_projection.commit().await.unwrap();
    let rebuilt_p08 = repository
        .rebuild_p08_projections(CAMPAIGN_ID)
        .await
        .expect("rebuild all P08 projections solely from canonical Event Store history");
    assert_eq!(rebuilt_p08.replayed_events, 15);
    assert_eq!(rebuilt_p08.combat_states, 1);
    assert_eq!(rebuilt_p08.chase_states, 1);
    assert_eq!(rebuilt_p08.reconsiderations, 2);
    assert_eq!(rebuilt_p08.ending_events, 1);
    assert_eq!(rebuilt_p08.growth_events, 1);
    let p08_projection_after: serde_json::Value = sqlx::query_scalar(
        r#"
        SELECT jsonb_build_object(
            'combat', (SELECT to_jsonb(combat) FROM public.combat_states AS combat
                        WHERE combat.campaign_id = $1),
            'chase', (SELECT to_jsonb(chase) FROM public.chase_states AS chase
                       WHERE chase.campaign_id = $1),
            'ending', (SELECT to_jsonb(ending) FROM public.ending_events AS ending
                        WHERE ending.campaign_id = $1),
            'growth', (SELECT to_jsonb(growth) FROM public.growth_events AS growth
                        WHERE growth.campaign_id = $1),
            'growth_sheet', (
                SELECT to_jsonb(sheet)
                  FROM public.character_sheet_versions AS sheet
                 WHERE sheet.sheet_version_id = 'sheet_p06_player_v2'
            ),
            'character', (
                SELECT to_jsonb(character)
                  FROM public.characters AS character
                 WHERE character.character_id = 'character_p06_player'
            ),
            'reconsiderations', (
                SELECT jsonb_agg(to_jsonb(reconsideration)
                                 ORDER BY reconsideration.reconsideration_id)
                  FROM public.reconsiderations AS reconsideration
                 WHERE reconsideration.campaign_id = $1
            )
        )
        "#,
    )
    .bind(CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        p08_projection_after, p08_projection_before,
        "P08 replay must reproduce the exact combat/chase/reconsideration/ending/growth projections"
    );
    let rebuilt = repository
        .rebuild_session_scene_projection(CAMPAIGN_ID)
        .await
        .expect("rebuild Session/Scene projection solely from canonical events");
    assert_eq!(rebuilt.restored_sessions, 1);
    assert_eq!(rebuilt.restored_scenes, 2);
    assert_eq!(rebuilt.replayed_events, 5);
    let event_count_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(
        event_count_after, event_count_before,
        "projection rebuild must not rewrite Event Store history"
    );
    let recovered = sqlx::query(
        r#"
        SELECT session.state, session.version, scene.state AS scene_state
          FROM core_domain.sessions AS session
          JOIN public.scenes AS scene
            ON scene.scene_id = session.active_scene_id
         WHERE session.session_id = 'session_p06_schema'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(recovered.get::<String, _>("state"), "ENDED");
    assert_eq!(recovered.get::<i64, _>("version"), 5);
    assert_eq!(recovered.get::<String, _>("scene_state"), "CLOSED");

    assert!(
        sqlx::query(
            "UPDATE public.characters SET display_name = 'tampered' \
             WHERE character_id = 'character_p06_player'"
        )
        .execute(&primary)
        .await
        .is_err(),
        "projection mutation without a newer matching canonical event must fail"
    );
}
