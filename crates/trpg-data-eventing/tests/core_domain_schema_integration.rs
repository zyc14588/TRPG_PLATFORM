use std::env;
use std::str::FromStr;

use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::persistence_postgresql::{
    AcceptInviteRequest, AuthorityContractSnapshot, CoreCommandMetadata, CoreDomainRepository,
    CoreDomainRepositoryError, CreateCampaignRequest, CreateCharacterRequest,
    ImportScenarioRequest, IssueInviteRequest, RecordCampaignForkRequest,
    RequestReconsiderationRequest, ReviewReconsiderationRequest, StartSessionRequest,
    SwitchSceneRequest,
};
use trpg_domain_core::domain_entities_value_objects::{MembershipRole, SessionState};
use trpg_ruleset_coc7::character_combat_san_chase::parse_scenario_yaml;
use trpg_shared_kernel::EventActorOriginWire;

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
    let repository = CoreDomainRepository::new(primary.clone(), store);

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
        now_unix_ms: NOW_MS,
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
    assert!(matches!(
        repository
            .accept_invite(
                &expired_metadata,
                &AcceptInviteRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    invite_id: "invite_p06_player".to_owned(),
                    accepting_user_id: PLAYER_ID.to_owned(),
                    raw_token: issued.raw_token.clone(),
                    accepted_at_unix_ms: NOW_MS + 60_000,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Domain(_))
    ));
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
                    accepted_at_unix_ms: NOW_MS + 1_000,
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
        accepted_at_unix_ms: NOW_MS + 1_000,
    };
    let accepted = repository
        .accept_invite(&accept_metadata, &accept_request)
        .await
        .expect("accept valid invite into durable membership");
    let accepted_retry = repository
        .accept_invite(&accept_metadata, &accept_request)
        .await
        .expect("exact invite acceptance retry is idempotent");
    assert_eq!(
        accepted_retry.last_event_sequence,
        accepted.last_event_sequence
    );

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
                sheet_json: r#"{"name":"Evelyn Hart","age":31,"ruleset":"coc7"}"#.to_owned(),
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

    create_campaign(
        &repository,
        CHILD_CAMPAIGN_ID,
        CHILD_AUTHORITY_ID,
        "room_p06_fork_child",
        "child_campaign_create",
    )
    .await;
    let snapshot_hash = format!(
        "sha256:{:x}",
        Sha256::digest(b"session_p06_schema:event_snapshot_v1")
    );
    repository
        .record_campaign_fork(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
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
                snapshot_hash,
                reason: "Preserve an alternate ruling".to_owned(),
            },
        )
        .await
        .expect("record immutable fork lineage");

    repository
        .request_reconsideration(
            &metadata(
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
            ),
            &RequestReconsiderationRequest {
                reconsideration_id: "reconsideration_p06_schema".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                original_event_sequence: campaign_event_sequence,
                requested_by: PLAYER_ID.to_owned(),
                reason: "Review the opening ruling".to_owned(),
            },
        )
        .await
        .expect("append reconsideration request");
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
                resolved: true,
                resolution: "Original ruling retained with explanation".to_owned(),
            },
        )
        .await
        .expect("append review event and resolve reconsideration");
    let reconsideration = sqlx::query(
        r#"
        SELECT state, version, jsonb_array_length(event_chain) AS chain_length
          FROM public.reconsiderations
         WHERE reconsideration_id = 'reconsideration_p06_schema'
        "#,
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(reconsideration.get::<String, _>("state"), "RESOLVED");
    assert_eq!(reconsideration.get::<i64, _>("version"), 2);
    assert_eq!(reconsideration.get::<i32, _>("chain_length"), 2);

    let event_count_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE campaign_id = $1")
            .bind(CAMPAIGN_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
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
