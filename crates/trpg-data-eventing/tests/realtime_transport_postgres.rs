use std::env;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::realtime_identity::{PersistentRealtimeIdentity, RealtimeIdentitySession};
use trpg_identity::{CampaignRole, GlobalRole, IdentityService};
use trpg_shared_kernel::{
    AuthorityContract, AuthorityContractDraft, AuthorityMode, AuthorityVersionSnapshotDraft,
    EntityId, EventActorOriginWire, Visibility,
};

const CAMPAIGN: &str = "campaign_ar07_real";
const GROUP: &str = "group_ar07_red";
const OWNER: &str = "owner_ar07_real";
const KEEPER: &str = "keeper_ar07_real";
const PLAYER_A: &str = "player_a_ar07_real";
const PLAYER_B: &str = "player_b_ar07_real";
const SPECTATOR: &str = "spectator_ar07_real";
const PASSWORD: &str = "AR07 real service password";
const IDENTITY_KEY: [u8; 32] = [0x31; 32];
const INTEGRITY_KEY: [u8; 32] = [0x42; 32];
const PAYLOAD_KEY: [u8; 32] = [0x53; 32];

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} is required for the AR07 real-service gate"))
}

fn realtime_database_url(primary_url: &str) -> String {
    let mut url = url::Url::parse(primary_url).expect("valid AR07 primary URL");
    url.set_username("trpg_ar07_realtime_login")
        .expect("set AR07 realtime username");
    url.set_password(Some("ar07_realtime_password"))
        .expect("set AR07 realtime password");
    url.to_string()
}

fn now_unix_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_millis(),
    )
    .expect("current timestamp fits u64")
}

fn authority(now: u64) -> AuthorityContract {
    AuthorityContract::new_locked(AuthorityContractDraft {
        contract_id: "authority_ar07_real_1".to_owned(),
        campaign_id: CAMPAIGN.to_owned(),
        mode: AuthorityMode::HumanKp,
        authority_owner: KEEPER.to_owned(),
        version: 1,
        snapshot: AuthorityVersionSnapshotDraft {
            ruleset_version: "coc7_rules_ar07".to_owned(),
            house_rules_version: "house_rules_ar07".to_owned(),
            scenario_version: "scenario_ar07".to_owned(),
            prompt_version: "prompt_ar07".to_owned(),
            agent_pack_version: "agent_pack_ar07".to_owned(),
            tool_schema_version: "tool_schema_ar07".to_owned(),
            safety_profile_version: "safety_ar07".to_owned(),
            ai_provider_snapshot: "provider_ar07".to_owned(),
            model_route_snapshot: "route_ar07".to_owned(),
            character_sheet_template_version: "sheet_ar07".to_owned(),
        },
        created_at_unix_ms: now,
    })
    .expect("valid AR07 authority")
}

fn draft(
    ordinal: u64,
    expected_version: i64,
    event_type: &str,
    visibility_label: &str,
    visibility_subject: &str,
) -> AtomicCommitDraft {
    AtomicCommitDraft {
        commit_id: format!("commit_ar07_{ordinal}"),
        campaign_id: CAMPAIGN.to_owned(),
        stream_id: CAMPAIGN.to_owned(),
        idempotency_key: format!("idempotency_ar07_{ordinal}"),
        expected_version,
        command_id: format!("command_ar07_{ordinal}"),
        authenticated_actor_id: "workflow_ar07".to_owned(),
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: "authority_ar07_real_1".to_owned(),
        authority_owner: KEEPER.to_owned(),
        visibility_label: visibility_label.to_owned(),
        visibility_subject: visibility_subject.to_owned(),
        data_subject_id: if matches!(
            visibility_label,
            "private_to_player" | "investigator_private"
        ) {
            visibility_subject.to_owned()
        } else {
            "not_applicable".to_owned()
        },
        provenance_kind: "rules_engine_decision".to_owned(),
        provenance_reference: format!("decision_ar07_{ordinal}"),
        provenance_recorded_by: "rules_engine_ar07".to_owned(),
        correlation_id: format!("correlation_ar07_{ordinal}"),
        causation_id: format!("causation_ar07_{ordinal}"),
        trace_id: format!("trace_ar07_{ordinal}"),
        events: vec![CanonicalEventDraft {
            event_type: event_type.to_owned(),
            payload_json: format!(r#"{{"ordinal":{ordinal}}}"#),
            visibility: None,
            projection_targets: Vec::new(),
        }],
        audit: PolicyAuditDraft {
            actor_id: KEEPER.to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: "session_ar07".to_owned(),
            resource_type: "campaign".to_owned(),
            resource_id: CAMPAIGN.to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "human_keeper".to_owned(),
            openfga_decision_id: format!("openfga_ar07_{ordinal}"),
            openfga_policy_revision: "openfga_ar07".to_owned(),
            opa_decision_id: format!("opa_ar07_{ordinal}"),
            opa_policy_revision: "opa_ar07".to_owned(),
        },
    }
}

async fn reset_dedicated_database(database_url: &str, expected_name: &str) {
    assert_eq!(
        env::var("AR07_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "AR07_ALLOW_DATABASE_RESET=1 is required"
    );
    let options = PgConnectOptions::from_str(database_url).expect("valid AR07 database URL");
    assert!(matches!(
        options.get_host(),
        "127.0.0.1" | "localhost" | "::1"
    ));
    assert_eq!(options.get_database(), Some(expected_name));
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect dedicated AR07 database");
    sqlx::raw_sql(
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; \
         CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;",
    )
    .execute(&pool)
    .await
    .expect("reset dedicated AR07 database");
}

async fn create_realtime_roles(primary_url: &str) {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(primary_url)
        .await
        .expect("connect AR07 role bootstrap");
    sqlx::raw_sql(
        r#"
        DO $roles$
        DECLARE
            role_name text;
        BEGIN
            FOREACH role_name IN ARRAY ARRAY[
                'trpg_application',
                'trpg_api_service',
                'trpg_canonical_service',
                'trpg_worker_service',
                'trpg_realtime_service'
            ]
            LOOP
                IF NOT EXISTS (
                    SELECT 1 FROM pg_roles WHERE rolname = role_name
                ) THEN
                    EXECUTE format('CREATE ROLE %I NOLOGIN', role_name);
                END IF;
            END LOOP;
            IF NOT EXISTS (
                SELECT 1 FROM pg_roles WHERE rolname = 'trpg_ar07_realtime_login'
            ) THEN
                CREATE ROLE trpg_ar07_realtime_login
                    LOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE
                    NOREPLICATION NOBYPASSRLS PASSWORD 'ar07_realtime_password';
            END IF;
        END;
        $roles$;
        ALTER ROLE trpg_ar07_realtime_login PASSWORD 'ar07_realtime_password';
        GRANT trpg_realtime_service TO trpg_ar07_realtime_login;
        "#,
    )
    .execute(&pool)
    .await
    .expect("bootstrap AR07 realtime roles");
}

async fn realtime_pool(primary_url: &str) -> sqlx::PgPool {
    let options = PgConnectOptions::from_str(primary_url)
        .expect("valid primary URL")
        .username("trpg_ar07_realtime_login")
        .password("ar07_realtime_password");
    PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .expect("connect through least-privilege realtime login")
}

async fn visible_types(
    session: &RealtimeIdentitySession,
    events: &[trpg_data_eventing::event_store_sqlx_outbox_projection::CanonicalReplayEvent],
    now: u64,
) -> Vec<String> {
    let mut visible = Vec::new();
    for event in events {
        let campaign = EntityId::new(&event.campaign_id).expect("valid event campaign");
        let visibility = Visibility::try_from_parts(
            &event.visibility_label,
            (event.visibility_subject != "not_applicable")
                .then_some(event.visibility_subject.as_str()),
        )
        .expect("valid canonical visibility");
        if session
            .can_view(&campaign, &visibility, now)
            .await
            .expect("live visibility decision")
        {
            visible.push(event.event_type.clone());
        }
    }
    visible
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn production_realtime_identity_filters_canonical_replay_and_nats_only_wakes() {
    let primary_url = required("AR07_DATABASE_URL");
    let witness_url = required("AR07_WITNESS_DATABASE_URL");
    let nats_url = required("AR07_NATS_URL");
    let primary_name = required("AR07_DATABASE_NAME");
    let witness_name = required("AR07_WITNESS_DATABASE_NAME");
    reset_dedicated_database(&primary_url, &primary_name).await;
    reset_dedicated_database(&witness_url, &witness_name).await;
    create_realtime_roles(&primary_url).await;

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
        &realtime_database_url(&primary_url),
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
    let realtime_identity = PersistentRealtimeIdentity::new(realtime_pool(&primary_url).await);
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
