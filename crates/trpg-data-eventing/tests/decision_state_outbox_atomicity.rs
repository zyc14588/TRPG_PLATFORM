use std::env;
use std::str::FromStr;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::persistence_postgresql::{
    AcceptInviteRequest, AuthorityContractSnapshot, CoreCommandMetadata, CoreDomainRepository,
    CoreDomainRepositoryError, CreateCampaignRequest, CreateCharacterRequest,
    ImportScenarioRequest, InvestigationExecutionRecord, IssueInviteRequest, MembershipRole,
    PlayerActionDiceRecord, PlayerActionIntentRecord, SanityExecutionRecord, StartSessionRequest,
    SubmitPlayerActionRequest,
};
use trpg_ruleset_coc7::character_combat_san_chase::parse_scenario_yaml;
use trpg_shared_kernel::EventActorOriginWire;

const INTEGRITY_KEY: &[u8; 32] = &[0x27; 32];
const PAYLOAD_KEY: &[u8; 32] = &[0x38; 32];
const CAMPAIGN_ID: &str = "campaign_p07_atomic";
const AUTHORITY_ID: &str = "authority_campaign_p07_atomic_1";
const KEEPER_ID: &str = "keeper_p07_atomic";
const PLAYER_ID: &str = "player_p07_atomic";
const CHARACTER_ID: &str = "character_p07_atomic";
const ACTION_ID: &str = "action_p07_atomic";
const NOW_MS: u64 = 2_400_000_000_000;

async fn reset_database(url: &str, expected: &str, witness: bool) -> PgPool {
    assert_eq!(
        env::var("P07_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "P07 tests require explicit dedicated-database reset authorization"
    );
    let options = PgConnectOptions::from_str(url).expect("valid P07 PostgreSQL URL");
    assert!(matches!(
        options.get_host(),
        "localhost" | "127.0.0.1" | "::1"
    ));
    assert_eq!(options.get_database(), Some(expected));
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect_with(options)
        .await
        .expect("connect dedicated P07 PostgreSQL");
    let sql = if witness {
        "DROP SCHEMA public CASCADE; CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;"
    } else {
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;"
    };
    sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    pool
}

#[allow(clippy::too_many_arguments)]
fn metadata(
    actor_id: &str,
    actor_role: &str,
    stream_id: &str,
    resource_type: &str,
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
        authenticated_actor_id: "workflow_p07_atomic".to_owned(),
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: AUTHORITY_ID.to_owned(),
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
            actor_id: "workflow_p07_atomic".to_owned(),
            actor_origin: "workload".to_owned(),
            authentication_reference: "workflow_p07_atomic".to_owned(),
            resource_type: resource_type.to_owned(),
            resource_id: stream_id.to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "workflow".to_owned(),
            openfga_decision_id: format!("openfga_{suffix}"),
            openfga_policy_revision: "p07-lower-layer-fixture-v1".to_owned(),
            opa_decision_id: format!("opa_{suffix}"),
            opa_policy_revision: "p07-lower-layer-fixture-v1".to_owned(),
        },
    }
}

fn authority() -> AuthorityContractSnapshot {
    AuthorityContractSnapshot {
        contract_id: AUTHORITY_ID.to_owned(),
        authority_mode: "HUMAN_KP".to_owned(),
        authority_owner: KEEPER_ID.to_owned(),
        ruleset_version: "coc7-1".to_owned(),
        house_rules_version: "none-1".to_owned(),
        scenario_version: "tutorial-0.1.0".to_owned(),
        prompt_version: "p07-1".to_owned(),
        agent_pack_version: "none-1".to_owned(),
        tool_schema_version: "tools-1".to_owned(),
        safety_profile_version: "safety-1".to_owned(),
        ai_provider_snapshot: "not_applicable".to_owned(),
        model_route_snapshot: "not_applicable".to_owned(),
        character_sheet_template_version: "coc7-sheet-1".to_owned(),
    }
}

fn character_sheet() -> String {
    serde_json::json!({
        "name": "Evelyn Hart",
        "age": 31,
        "occupation": "Investigative journalist",
        "era": "1920s",
        "birthplace": "Brisbane",
        "characteristics": {
            "strength": 50,
            "dexterity": 60,
            "power": 65,
            "constitution": 55,
            "size": 50,
            "appearance": 55,
            "intelligence": 70,
            "education": 75,
            "luck": 60
        },
        "skills": {
            "Library Use": 70,
            "Psychology": 55
        },
        "backstory_anchors": [
            "Protects confidential sources",
            "Distrusts official explanations"
        ]
    })
    .to_string()
}

async fn seed_tutorial(repository: &CoreDomainRepository, primary: &PgPool) {
    for (id, login) in [(KEEPER_ID, "keeper-p07"), (PLAYER_ID, "player-p07")] {
        sqlx::query(
            "INSERT INTO public.users \
             (user_id, login_normalized, password_hash, global_role) \
             VALUES ($1, $2, 'not-used-by-p07-test', 'USER')",
        )
        .bind(id)
        .bind(login)
        .execute(primary)
        .await
        .unwrap();
    }

    repository
        .create_campaign(
            &metadata(
                KEEPER_ID,
                "human_keeper",
                CAMPAIGN_ID,
                "campaign",
                0,
                "p07_campaign",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &CreateCampaignRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                title: "P07 atomic tutorial".to_owned(),
                room_id: "room_p07_atomic".to_owned(),
                room_name: "P07 table".to_owned(),
                created_at_unix_ms: NOW_MS,
                authority: authority(),
            },
        )
        .await
        .unwrap();
    let invite = repository
        .issue_invite(
            &metadata(
                KEEPER_ID,
                "human_keeper",
                "invite_p07_atomic",
                "campaign_invite",
                0,
                "p07_invite",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            &IssueInviteRequest {
                invite_id: "invite_p07_atomic".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                invited_user_id: PLAYER_ID.to_owned(),
                role: MembershipRole::Player,
                expires_at_unix_ms: NOW_MS + 60_000,
            },
        )
        .await
        .unwrap();
    repository
        .accept_invite(
            &metadata(
                PLAYER_ID,
                "investigator",
                "invite_p07_atomic",
                "campaign_invite",
                1,
                "p07_accept",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &AcceptInviteRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                invite_id: "invite_p07_atomic".to_owned(),
                accepting_user_id: PLAYER_ID.to_owned(),
                raw_token: invite.raw_token,
            },
        )
        .await
        .unwrap();
    repository
        .create_character(
            &metadata(
                PLAYER_ID,
                "investigator",
                CHARACTER_ID,
                "character",
                0,
                "p07_character",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &CreateCharacterRequest {
                character_id: CHARACTER_ID.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: PLAYER_ID.to_owned(),
                display_name: "Evelyn Hart".to_owned(),
                sheet_version_id: "sheet_p07_atomic_v1".to_owned(),
                sheet_json: character_sheet(),
            },
        )
        .await
        .unwrap();
    repository
        .submit_character(
            &metadata(
                PLAYER_ID,
                "investigator",
                CHARACTER_ID,
                "character",
                1,
                "p07_character_submit",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            CAMPAIGN_ID,
            CHARACTER_ID,
        )
        .await
        .unwrap();
    repository
        .approve_character_initial_version(
            &metadata(
                KEEPER_ID,
                "human_keeper",
                CHARACTER_ID,
                "character",
                2,
                "p07_character_approve",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            CHARACTER_ID,
        )
        .await
        .unwrap();

    let tutorial = parse_scenario_yaml(include_str!(
        "../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml"
    ))
    .unwrap();
    repository
        .import_scenario(
            &metadata(
                KEEPER_ID,
                "human_keeper",
                "scenario_p07_atomic",
                "scenario",
                0,
                "p07_scenario",
                "keeper_only",
                "not_applicable",
                "imported_source",
            ),
            &ImportScenarioRequest {
                scenario_id: "scenario_p07_atomic".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                ruleset_id: tutorial.ruleset_id,
                format_version: tutorial.format_version,
                content_hash: tutorial.content_hash,
                document_json: tutorial.canonical_json,
            },
        )
        .await
        .unwrap();
    repository
        .start_session(
            &metadata(
                KEEPER_ID,
                "human_keeper",
                "session_p07_atomic",
                "session",
                0,
                "p07_session",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: "session_p07_atomic".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p07_atomic".to_owned(),
                scenario_id: "scenario_p07_atomic".to_owned(),
                scene_id: "scene_p07_atomic".to_owned(),
                scene_key: "scene_archive_front".to_owned(),
                scene_name: "灰港市政档案室前厅".to_owned(),
                started_at_unix_ms: NOW_MS + 2_000,
            },
        )
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn decision_state_and_outbox_commit_or_rollback_together() {
    let primary_url = env::var("P07_DATABASE_URL").expect("P07_DATABASE_URL required");
    let witness_url =
        env::var("P07_WITNESS_DATABASE_URL").expect("P07_WITNESS_DATABASE_URL required");
    let primary_name = env::var("P07_RESET_DATABASE").unwrap();
    let witness_name = env::var("P07_WITNESS_RESET_DATABASE").unwrap();
    let primary = reset_database(&primary_url, &primary_name, false).await;
    reset_database(&witness_url, &witness_name, true)
        .await
        .close()
        .await;

    let store = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "p07-atomic-integrity-key",
        INTEGRITY_KEY,
        "p07-atomic-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .unwrap();
    store
        .prepare_for_service()
        .await
        .expect("P07 migration chain must apply to an empty database");
    let repository = CoreDomainRepository::new(primary.clone(), store);
    seed_tutorial(&repository, &primary).await;

    let submit_metadata = metadata(
        PLAYER_ID,
        "investigator",
        ACTION_ID,
        "player_action",
        0,
        "p07_action_submit",
        "party_visible",
        "not_applicable",
        "user_statement",
    );
    let submission = SubmitPlayerActionRequest {
        action_id: ACTION_ID.to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        character_id: CHARACTER_ID.to_owned(),
        scene_id: "scene_p07_atomic".to_owned(),
        submitted_by: PLAYER_ID.to_owned(),
        submitted_at_unix_ms: NOW_MS + 3_000,
        intent: PlayerActionIntentRecord::Investigation {
            skill_name: "Library Use".to_owned(),
            clue_id: "clue_wrong_signature".to_owned(),
            clue_importance: "CORE".to_owned(),
            adjustment: "NONE".to_owned(),
        },
    };
    let submitted = repository
        .submit_player_action(&submit_metadata, &submission)
        .await
        .expect("submit pending P07 action");
    let submitted_retry = repository
        .submit_player_action(&submit_metadata, &submission)
        .await
        .expect("exact submission retry");
    assert_eq!(
        submitted_retry.last_event_sequence,
        submitted.last_event_sequence
    );

    let confirm_metadata = metadata(
        KEEPER_ID,
        "human_keeper",
        ACTION_ID,
        "player_action",
        1,
        "p07_action_confirm",
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    );
    let execution = InvestigationExecutionRecord {
        action_id: ACTION_ID.to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        character_id: CHARACTER_ID.to_owned(),
        decision_id: "decision_p07_atomic".to_owned(),
        tool_execution_id: "tool_execution_p07_atomic".to_owned(),
        confirmed_by: KEEPER_ID.to_owned(),
        resolved_at_unix_ms: NOW_MS + 4_000,
        dice: PlayerActionDiceRecord {
            roll_id: "dice_p07_atomic".to_owned(),
            target_value: 70,
            rolled_value: 42,
            success_level: "REGULAR".to_owned(),
            selected_tens_digit: 4,
            ones_digit: 2,
            adjustment: "NONE".to_owned(),
        },
        skill_name: "Library Use".to_owned(),
        clue_record_id: "clue_result_p07_atomic".to_owned(),
        clue_id: "clue_wrong_signature".to_owned(),
        clue_importance: "CORE".to_owned(),
        clue_outcome: "REVEALED".to_owned(),
        clue_cost: None,
        revealed_to_party: true,
    };

    sqlx::raw_sql(
        r#"
        CREATE FUNCTION public.reject_p07_decision_for_test()
        RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            RAISE EXCEPTION 'injected P07 projection failure';
        END;
        $$;
        CREATE TRIGGER zz_reject_p07_decision_for_test
        BEFORE INSERT ON public.decision_records
        FOR EACH ROW EXECUTE FUNCTION public.reject_p07_decision_for_test();
        "#,
    )
    .execute(&primary)
    .await
    .unwrap();
    assert!(matches!(
        repository
            .commit_investigation_execution(&confirm_metadata, &execution)
            .await,
        Err(CoreDomainRepositoryError::Canonical(_))
    ));

    let confirmation_events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.event_store WHERE command_id = $1")
            .bind(&confirm_metadata.command_id)
            .fetch_one(&primary)
            .await
            .unwrap();
    let confirmation_outbox: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_outbox \
         WHERE idempotency_key LIKE $1",
    )
    .bind(format!("outbox:{}:%", confirm_metadata.idempotency_key))
    .fetch_one(&primary)
    .await
    .unwrap();
    let decision_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM public.decision_records")
        .fetch_one(&primary)
        .await
        .unwrap();
    let action_state: String =
        sqlx::query_scalar("SELECT state FROM public.player_actions WHERE action_id = $1")
            .bind(ACTION_ID)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(confirmation_events, 0);
    assert_eq!(confirmation_outbox, 0);
    assert_eq!(decision_rows, 0);
    assert_eq!(action_state, "AWAITING_HUMAN_CONFIRMATION");

    sqlx::raw_sql(
        r#"
        DROP TRIGGER zz_reject_p07_decision_for_test
            ON public.decision_records;
        DROP FUNCTION public.reject_p07_decision_for_test();
        "#,
    )
    .execute(&primary)
    .await
    .unwrap();
    let committed = repository
        .commit_investigation_execution(&confirm_metadata, &execution)
        .await
        .expect("exact retry succeeds after transient projection failure");
    let retried = repository
        .commit_investigation_execution(&confirm_metadata, &execution)
        .await
        .expect("committed exact retry is idempotent");
    assert_eq!(retried.last_event_sequence, committed.last_event_sequence);

    let counts = sqlx::query(
        r#"
        SELECT
          (SELECT count(*) FROM public.event_store
            WHERE command_id = $1) AS events,
          (SELECT count(*) FROM public.event_outbox
            WHERE commit_id = $2) AS outbox,
          (SELECT count(*) FROM public.decision_records) AS decisions,
          (SELECT count(*) FROM public.dice_rolls) AS dice,
          (SELECT count(*) FROM public.clues) AS clues
        "#,
    )
    .bind(&confirm_metadata.command_id)
    .bind(&confirm_metadata.commit_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(counts.get::<i64, _>("events"), 4);
    assert_eq!(counts.get::<i64, _>("outbox"), 4);
    assert_eq!(counts.get::<i64, _>("decisions"), 1);
    assert_eq!(counts.get::<i64, _>("dice"), 1);
    assert_eq!(counts.get::<i64, _>("clues"), 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM public.player_actions WHERE action_id = $1",
        )
        .bind(ACTION_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        "RESOLVED"
    );

    let direct_write_privilege: bool = sqlx::query_scalar(
        "SELECT has_table_privilege(\
         'trpg_canonical_service', 'public.decision_records', 'INSERT')",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(
        !direct_write_privilege,
        "canonical role gets only the guarded projection function"
    );

    let sanity_action_id = "action_p07_atomic_sanity";
    let sanity_submit_metadata = metadata(
        PLAYER_ID,
        "investigator",
        sanity_action_id,
        "player_action",
        0,
        "p07_sanity_submit",
        "private_to_player",
        PLAYER_ID,
        "user_statement",
    );
    repository
        .submit_player_action(
            &sanity_submit_metadata,
            &SubmitPlayerActionRequest {
                action_id: sanity_action_id.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                scene_id: "scene_p07_atomic".to_owned(),
                submitted_by: PLAYER_ID.to_owned(),
                submitted_at_unix_ms: NOW_MS + 5_000,
                intent: PlayerActionIntentRecord::SanityCheck {
                    success_loss: 0,
                    failure_loss: 3,
                    day_key: "tutorial_day_1".to_owned(),
                },
            },
        )
        .await
        .expect("submit pending SAN action");
    let sanity_confirm_metadata = metadata(
        KEEPER_ID,
        "human_keeper",
        sanity_action_id,
        "player_action",
        1,
        "p07_sanity_confirm",
        "private_to_player",
        PLAYER_ID,
        "human_keeper_statement",
    );
    repository
        .commit_sanity_execution(
            &sanity_confirm_metadata,
            &SanityExecutionRecord {
                action_id: sanity_action_id.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                decision_id: "decision_p07_atomic_sanity".to_owned(),
                tool_execution_id: "tool_execution_p07_atomic_sanity".to_owned(),
                confirmed_by: KEEPER_ID.to_owned(),
                resolved_at_unix_ms: NOW_MS + 6_000,
                dice: PlayerActionDiceRecord {
                    roll_id: "dice_p07_atomic_sanity".to_owned(),
                    target_value: 65,
                    rolled_value: 80,
                    success_level: "FAILURE".to_owned(),
                    selected_tens_digit: 8,
                    ones_digit: 0,
                    adjustment: "NONE".to_owned(),
                },
                sanity_event_id: "sanity_event_p07_atomic".to_owned(),
                sheet_version_id: "sheet_p07_atomic_v2".to_owned(),
                day_key: "tutorial_day_1".to_owned(),
                day_start_sanity: 65,
                sanity_before: 65,
                sanity_after: 62,
                sanity_loss: 3,
                day_loss: 3,
                indefinite_threshold: 13,
                madness_state: "STABLE".to_owned(),
            },
        )
        .await
        .expect("SAN decision, dice, sheet, event and outbox commit atomically");
    let sanity = sqlx::query(
        r#"
        SELECT
          s.day_start_sanity,
          s.sanity_before,
          s.sanity_after,
          s.day_loss,
          s.indefinite_threshold,
          s.visibility_label::TEXT AS visibility_label,
          d.random_source,
          csv.sheet_json -> 'sanity_state' ->> 'day_key' AS day_key,
          csv.sheet_json -> 'sanity_state' ->> 'current_sanity'
              AS current_sanity,
          (SELECT count(*) FROM public.event_store
            WHERE command_id = $1) AS events,
          (SELECT count(*) FROM public.event_outbox
            WHERE commit_id = $2) AS outbox
        FROM public.sanity_events s
        JOIN public.dice_rolls d ON d.action_id = s.action_id
        JOIN public.character_sheet_versions csv
          ON csv.sheet_version_id = s.sheet_version_id
        WHERE s.action_id = $3
        "#,
    )
    .bind(&sanity_confirm_metadata.command_id)
    .bind(&sanity_confirm_metadata.commit_id)
    .bind(sanity_action_id)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(sanity.get::<i16, _>("day_start_sanity"), 65);
    assert_eq!(sanity.get::<i16, _>("sanity_before"), 65);
    assert_eq!(sanity.get::<i16, _>("sanity_after"), 62);
    assert_eq!(sanity.get::<i16, _>("day_loss"), 3);
    assert_eq!(sanity.get::<i16, _>("indefinite_threshold"), 13);
    assert_eq!(
        sanity.get::<String, _>("visibility_label"),
        "private_to_player"
    );
    assert_eq!(sanity.get::<String, _>("random_source"), "SERVER_OS_CSPRNG");
    assert_eq!(sanity.get::<String, _>("day_key"), "tutorial_day_1");
    assert_eq!(sanity.get::<String, _>("current_sanity"), "62");
    assert_eq!(sanity.get::<i64, _>("events"), 3);
    assert_eq!(sanity.get::<i64, _>("outbox"), 3);
}
