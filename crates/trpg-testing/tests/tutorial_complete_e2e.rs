use std::collections::BTreeSet;
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
    ImportScenarioRequest, InvestigationExecutionRecord, IssueInviteRequest,
    PlayerActionDiceRecord, PlayerActionIntentRecord, RecordCampaignForkRequest,
    RecordChaseStateRequest, RecordCombatStateRequest, RecordEndingRequest, RecordGrowthRequest,
    RequestReconsiderationRequest, ResolveReconsiderationRequest, ReviewReconsiderationRequest,
    SanityExecutionRecord, StartSessionRequest, SubmitPlayerActionRequest, SwitchSceneRequest,
};
use trpg_domain_core::domain_entities_value_objects::{
    MembershipRole, ReconsiderationOutcome, SessionState,
};
use trpg_ruleset_coc7::character_combat_san_chase::parse_scenario_yaml;
use trpg_ruleset_coc7::chase_state_machine::{
    ChaseObstacle, ChaseParticipant, ChaseRole, ChaseState, ChaseStatus,
};
use trpg_ruleset_coc7::combat_state_machine::{
    CombatActionKind, CombatCondition, CombatDamageFormula, CombatDefense, CombatHealth,
    CombatSkillTargets, CombatState, CombatStatus, CombatWeapon, CombatWeaponLoadout,
    CombatantState,
};
use trpg_ruleset_coc7::dice_roll_contract::{
    server_roll_skill_check, server_roll_skill_growth, success_level, DiceAdjustment,
    ServerDiceRoll, SuccessLevel,
};
use trpg_shared_kernel::{
    server_damage_roll, server_percentile_roll, EventActorOriginWire, ServerDamageRoll,
    ServerPercentileRoll,
};

const TUTORIAL: &str =
    include_str!("../../../fixtures/scenarios/tutorial_mist_archive.scenario.yaml");
const INTEGRITY_KEY: &[u8; 32] = &[0x58; 32];
const PAYLOAD_KEY: &[u8; 32] = &[0x69; 32];
const CAMPAIGN_ID: &str = "campaign_p08_tutorial";
const CHILD_CAMPAIGN_ID: &str = "campaign_p08_tutorial_fork";
const AUTHORITY_ID: &str = "authority_campaign_p08_tutorial_1";
const CHILD_AUTHORITY_ID: &str = "authority_contract_campaign_p08_tutorial_fork_1";
const KEEPER_ID: &str = "keeper_p08_tutorial";
const PLAYER_ID: &str = "player_p08_tutorial";
const CHARACTER_ID: &str = "character_p08_evelyn";
const SESSION_ID: &str = "session_p08_tutorial";
const NOW_MS: u64 = 2_800_000_000_000;

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

fn damage_with_value(dice_count: u8, die_sides: u8, flat_bonus: i8, value: u8) -> ServerDamageRoll {
    loop {
        let roll = server_damage_roll(dice_count, die_sides, flat_bonus).unwrap();
        if roll.value() == value {
            return roll;
        }
    }
}

fn weapon_loadout(melee_bonus: i8, firearm_bonus: i8) -> CombatWeaponLoadout {
    CombatWeaponLoadout::new(
        CombatWeapon::new(
            "selected_melee_weapon",
            CombatDamageFormula::new(1, 6, melee_bonus).unwrap(),
        )
        .unwrap(),
        CombatWeapon::new(
            "selected_firearm",
            CombatDamageFormula::new(1, 6, firearm_bonus).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn tutorial_combat_state(combat_id: &str, character_health: CombatHealth) -> CombatState {
    CombatState::start(
        combat_id,
        vec![
            CombatantState::new(
                CHARACTER_ID,
                70,
                character_health,
                1,
                CombatSkillTargets::new(45, 35, 40, 30, 10).unwrap(),
                weapon_loadout(1, 5),
            )
            .unwrap(),
            CombatantState::new(
                "npc_marta",
                80,
                CombatHealth::new(8, 8, CombatCondition::Able).unwrap(),
                0,
                CombatSkillTargets::new(60, 80, 40, 30, 10).unwrap(),
                weapon_loadout(0, 5),
            )
            .unwrap(),
        ],
    )
    .expect("start Tutorial combat aggregate")
}

async fn reset_database(url: &str, expected_database: &str, witness: bool) -> PgPool {
    assert_eq!(
        env::var("P08_ALLOW_DATABASE_RESET").as_deref(),
        Ok("1"),
        "P08 E2E requires explicit dedicated-database reset authorization"
    );
    let options = PgConnectOptions::from_str(url).expect("valid P08 PostgreSQL URL");
    assert!(
        matches!(options.get_host(), "localhost" | "127.0.0.1" | "::1"),
        "P08 E2E refuses to reset a non-local database"
    );
    assert_eq!(
        options.get_database(),
        Some(expected_database),
        "P08 E2E refuses to reset a non-dedicated database"
    );
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect_with(options)
        .await
        .expect("connect dedicated P08 PostgreSQL");
    let reset_sql = if witness {
        "DROP SCHEMA public CASCADE; CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;"
    } else {
        "DROP SCHEMA IF EXISTS core_domain CASCADE; \
         DROP SCHEMA public CASCADE; CREATE SCHEMA public; \
         GRANT ALL ON SCHEMA public TO public;"
    };
    sqlx::raw_sql(reset_sql)
        .execute(&pool)
        .await
        .expect("reset dedicated P08 schemas");
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
        prompt_version: "p08-1".to_owned(),
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
    authority_id: &str,
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
        authenticated_actor_id: "workflow_p08_tutorial".to_owned(),
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
            actor_id: "workflow_p08_tutorial".to_owned(),
            actor_origin: "workload".to_owned(),
            authentication_reference: "workflow_p08_tutorial".to_owned(),
            resource_type: resource_type.to_owned(),
            resource_id: stream_id.to_owned(),
            action: "write_official_state".to_owned(),
            requested_role: "workflow".to_owned(),
            openfga_decision_id: format!("openfga_{suffix}"),
            openfga_policy_revision: "p08-e2e-formal-decision-v1".to_owned(),
            opa_decision_id: format!("opa_{suffix}"),
            opa_policy_revision: "p08-e2e-formal-decision-v1".to_owned(),
        },
    }
}

fn character_sheet() -> String {
    serde_json::json!({
        "name": "Evelyn Hart",
        "age": 31,
        "occupation": "Investigative journalist",
        "era": "1920s",
        "characteristics": {
            "power": 65,
            "dexterity": 60
        },
        "skills": {
            "Library Use": 70,
            "Psychology": 55
        },
        "combat_profile": {
            "dexterity": 70,
            "skill_targets": {
                "melee": 45,
                "firearm": 35,
                "dodge": 40,
                "first_aid": 30,
                "medicine": 10
            },
            "weapon_loadout": {
                "melee": {
                    "weapon_id": "selected_melee_weapon",
                    "damage_formula": {
                        "dice_count": 1,
                        "die_sides": 6,
                        "flat_bonus": 1
                    }
                },
                "firearm": {
                    "weapon_id": "selected_firearm",
                    "damage_formula": {
                        "dice_count": 1,
                        "die_sides": 6,
                        "flat_bonus": 5
                    }
                }
            },
            "current_hp": 10,
            "max_hp": 10,
            "armor": 1,
            "condition": "ABLE"
        },
        "chase_profile": {
            "role": "QUARRY",
            "movement_rate": 8
        },
        "backstory_anchors": [
            "Protects confidential sources",
            "Distrusts official explanations"
        ]
    })
    .to_string()
}

fn success_level_name(level: SuccessLevel) -> &'static str {
    match level {
        SuccessLevel::Critical => "CRITICAL",
        SuccessLevel::Extreme => "EXTREME",
        SuccessLevel::Hard => "HARD",
        SuccessLevel::Regular => "REGULAR",
        SuccessLevel::Failure => "FAILURE",
        SuccessLevel::Fumble => "FUMBLE",
    }
}

fn server_dice_record(server_roll: &ServerDiceRoll) -> PlayerActionDiceRecord {
    let outcome = server_roll.outcome();
    let adjustment = match outcome.adjustment {
        DiceAdjustment::None => "NONE",
        DiceAdjustment::Bonus => "BONUS",
        DiceAdjustment::Penalty => "PENALTY",
    };
    PlayerActionDiceRecord {
        roll_id: server_roll.roll_id().to_owned(),
        target_value: outcome.target,
        rolled_value: outcome.roll,
        success_level: success_level_name(outcome.success_level).to_owned(),
        selected_tens_digit: outcome.selected_tens_digit,
        ones_digit: outcome.ones_digit,
        adjustment: adjustment.to_owned(),
    }
}

fn succeeded(level: SuccessLevel) -> bool {
    matches!(
        level,
        SuccessLevel::Critical | SuccessLevel::Extreme | SuccessLevel::Hard | SuccessLevel::Regular
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
            &metadata(
                authority_id,
                KEEPER_ID,
                "human_keeper",
                campaign_id,
                "campaign",
                0,
                suffix,
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &CreateCampaignRequest {
                campaign_id: campaign_id.to_owned(),
                owner_user_id: KEEPER_ID.to_owned(),
                title: format!("P08 Tutorial {suffix}"),
                room_id: room_id.to_owned(),
                room_name: "Tutorial table".to_owned(),
                created_at_unix_ms: if campaign_id == CAMPAIGN_ID {
                    NOW_MS
                } else {
                    NOW_MS + 1
                },
                authority: authority(authority_id),
            },
        )
        .await
        .expect("create event-backed Campaign")
        .last_event_sequence
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tutorial_runs_through_real_repository_event_store_outbox_and_witness() {
    let primary_url =
        env::var("P08_DATABASE_URL").expect("P08_DATABASE_URL is required for the real E2E gate");
    let witness_url = env::var("P08_WITNESS_DATABASE_URL")
        .expect("P08_WITNESS_DATABASE_URL is required for the independent witness gate");
    let primary_database =
        env::var("P08_RESET_DATABASE").expect("P08_RESET_DATABASE must name the dedicated DB");
    let witness_database = env::var("P08_WITNESS_RESET_DATABASE")
        .expect("P08_WITNESS_RESET_DATABASE must name the dedicated witness DB");
    let primary = reset_database(&primary_url, &primary_database, false).await;
    let witness = reset_database(&witness_url, &witness_database, true).await;
    witness.close().await;

    let canonical = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "p08-tutorial-integrity-key",
        INTEGRITY_KEY,
        "p08-tutorial-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .expect("connect independent primary and Witness services");
    canonical
        .prepare_for_service()
        .await
        .expect("apply the full forward migration chain");
    let integrity_verifier = canonical.clone();
    let repository = CoreDomainRepository::new(primary.clone(), canonical);

    for (user_id, login) in [
        (KEEPER_ID, "keeper-p08-tutorial"),
        (PLAYER_ID, "player-p08-tutorial"),
    ] {
        sqlx::query(
            "INSERT INTO public.users \
             (user_id, login_normalized, password_hash, global_role) \
             VALUES ($1, $2, 'not-used-by-p08-e2e', 'USER')",
        )
        .bind(user_id)
        .bind(login)
        .execute(&primary)
        .await
        .expect("seed an identity referenced by the production repository");
    }

    let campaign_event_sequence = create_campaign(
        &repository,
        CAMPAIGN_ID,
        AUTHORITY_ID,
        "room_p08_tutorial",
        "p08_campaign_create",
    )
    .await;
    let invite = repository
        .issue_invite(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "invite_p08_tutorial",
                "campaign_invite",
                0,
                "p08_invite_issue",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            &IssueInviteRequest {
                invite_id: "invite_p08_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                invited_user_id: PLAYER_ID.to_owned(),
                role: MembershipRole::Player,
                expires_at_unix_ms: NOW_MS + 60_000,
            },
        )
        .await
        .expect("issue a real single-use Campaign invite");
    repository
        .accept_invite(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "invite_p08_tutorial",
                "campaign_invite",
                1,
                "p08_invite_accept",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &AcceptInviteRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                invite_id: "invite_p08_tutorial".to_owned(),
                accepting_user_id: PLAYER_ID.to_owned(),
                raw_token: invite.raw_token,
            },
        )
        .await
        .expect("accept the invite into durable membership");
    repository
        .create_character(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                CHARACTER_ID,
                "character",
                0,
                "p08_character_create",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &CreateCharacterRequest {
                character_id: CHARACTER_ID.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                owner_user_id: PLAYER_ID.to_owned(),
                display_name: "Evelyn Hart".to_owned(),
                sheet_version_id: "sheet_p08_evelyn_v1".to_owned(),
                sheet_json: character_sheet(),
            },
        )
        .await
        .expect("create the Tutorial investigator");
    repository
        .submit_character(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                CHARACTER_ID,
                "character",
                1,
                "p08_character_submit",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            CAMPAIGN_ID,
            CHARACTER_ID,
        )
        .await
        .expect("submit the Tutorial investigator");
    repository
        .approve_character_initial_version(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                CHARACTER_ID,
                "character",
                2,
                "p08_character_approve",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            CHARACTER_ID,
        )
        .await
        .expect("approve and lock the initial Character Sheet");

    let scenario = parse_scenario_yaml(TUTORIAL).expect("validate the actual Tutorial Scenario");
    assert_eq!(scenario.opening_scene_id, "scene_archive_front");
    assert!(scenario
        .encounter_ids
        .contains(&"encounter_basement_confrontation".to_owned()));
    assert!(scenario
        .encounter_ids
        .contains(&"encounter_archive_escape".to_owned()));
    assert!(scenario
        .ending_ids
        .contains(&"ending_expose_marta".to_owned()));
    assert!(scenario.growth_skills.contains(&"Library Use".to_owned()));
    repository
        .import_scenario(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "scenario_p08_tutorial",
                "scenario",
                0,
                "p08_scenario_import",
                "keeper_only",
                "not_applicable",
                "imported_source",
            ),
            &ImportScenarioRequest {
                scenario_id: "scenario_p08_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                ruleset_id: scenario.ruleset_id,
                format_version: scenario.format_version,
                content_hash: scenario.content_hash,
                document_json: scenario.canonical_json,
            },
        )
        .await
        .expect("import the validated Tutorial document");
    repository
        .start_session(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                SESSION_ID,
                "session",
                0,
                "p08_session_start",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: SESSION_ID.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p08_tutorial".to_owned(),
                scenario_id: "scenario_p08_tutorial".to_owned(),
                scene_id: "scene_p08_front".to_owned(),
                scene_key: "scene_archive_front".to_owned(),
                scene_name: "灰港市政档案室前厅".to_owned(),
                started_at_unix_ms: NOW_MS + 2_000,
            },
        )
        .await
        .expect("start the real Tutorial Session");

    repository
        .submit_player_action(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "action_p08_investigation",
                "player_action",
                0,
                "p08_investigation_submit",
                "party_visible",
                "not_applicable",
                "user_statement",
            ),
            &SubmitPlayerActionRequest {
                action_id: "action_p08_investigation".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                scene_id: "scene_p08_front".to_owned(),
                submitted_by: PLAYER_ID.to_owned(),
                submitted_at_unix_ms: NOW_MS + 3_000,
                intent: PlayerActionIntentRecord::Investigation {
                    skill_name: "Library Use".to_owned(),
                    clue_id: "clue_wrong_signature".to_owned(),
                    clue_importance: "CORE".to_owned(),
                    adjustment: "NONE".to_owned(),
                },
            },
        )
        .await
        .expect("submit a real investigation action");
    let investigation_roll =
        server_roll_skill_check(70, DiceAdjustment::None).expect("server investigation roll");
    let investigation_succeeded = succeeded(investigation_roll.outcome().success_level);
    repository
        .commit_investigation_execution(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "action_p08_investigation",
                "player_action",
                1,
                "p08_investigation_confirm",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &InvestigationExecutionRecord {
                action_id: "action_p08_investigation".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                decision_id: "decision_p08_investigation".to_owned(),
                tool_execution_id: "tool_execution_p08_investigation".to_owned(),
                confirmed_by: KEEPER_ID.to_owned(),
                resolved_at_unix_ms: NOW_MS + 4_000,
                dice: server_dice_record(&investigation_roll),
                skill_name: "Library Use".to_owned(),
                clue_record_id: "clue_result_p08_wrong_signature".to_owned(),
                clue_id: "clue_wrong_signature".to_owned(),
                clue_importance: "CORE".to_owned(),
                clue_outcome: if investigation_succeeded {
                    "REVEALED"
                } else {
                    "REVEALED_WITH_COST"
                }
                .to_owned(),
                clue_cost: (!investigation_succeeded).then_some("time_or_complication".to_owned()),
                revealed_to_party: true,
            },
        )
        .await
        .expect("commit investigation Decision, Dice and Clue atomically");

    repository
        .submit_player_action(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "action_p08_sanity",
                "player_action",
                0,
                "p08_sanity_submit",
                "private_to_player",
                PLAYER_ID,
                "user_statement",
            ),
            &SubmitPlayerActionRequest {
                action_id: "action_p08_sanity".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                scene_id: "scene_p08_front".to_owned(),
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
        .expect("submit a real SAN action");
    let sanity_roll = server_roll_skill_check(65, DiceAdjustment::None).expect("server SAN roll");
    let sanity_loss = if succeeded(sanity_roll.outcome().success_level) {
        0
    } else {
        3
    };
    repository
        .commit_sanity_execution(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "action_p08_sanity",
                "player_action",
                1,
                "p08_sanity_confirm",
                "private_to_player",
                PLAYER_ID,
                "human_keeper_statement",
            ),
            &SanityExecutionRecord {
                action_id: "action_p08_sanity".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                decision_id: "decision_p08_sanity".to_owned(),
                tool_execution_id: "tool_execution_p08_sanity".to_owned(),
                confirmed_by: KEEPER_ID.to_owned(),
                resolved_at_unix_ms: NOW_MS + 6_000,
                dice: server_dice_record(&sanity_roll),
                sanity_event_id: "sanity_event_p08".to_owned(),
                sheet_version_id: "sheet_p08_evelyn_v2".to_owned(),
                day_key: "tutorial_day_1".to_owned(),
                day_start_sanity: 65,
                sanity_before: 65,
                sanity_after: 65 - sanity_loss,
                sanity_loss,
                day_loss: sanity_loss,
                indefinite_threshold: 13,
                madness_state: "STABLE".to_owned(),
            },
        )
        .await
        .expect("commit SAN Decision, Dice, event and new Sheet atomically");

    let wrong_scene_combat = tutorial_combat_state(
        "combat_p08_wrong_scene",
        CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
    );
    assert!(matches!(
        repository
            .record_combat_state(
                &metadata(
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "combat_p08_wrong_scene",
                    "combat_state",
                    0,
                    "p08_combat_wrong_scene",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordCombatStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: SESSION_ID.to_owned(),
                    state_json: wrong_scene_combat.persistence_json().unwrap(),
                    attacker_roll: None,
                    defender_roll: None,
                    damage_roll: None,
                    medical_roll: None,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "combat_participant_authority"
        ))
    ));

    repository
        .switch_scene(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                SESSION_ID,
                "session",
                1,
                "p08_scene_switch",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &SwitchSceneRequest {
                session_id: SESSION_ID.to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                next_scene_id: "scene_p08_basement".to_owned(),
                next_scene_key: "scene_basement".to_owned(),
                next_scene_name: "地下盐窖".to_owned(),
                switched_at_unix_ms: NOW_MS + 7_000,
            },
        )
        .await
        .expect("switch into the confrontation scene");

    let mut combat = tutorial_combat_state(
        "combat_p08_tutorial",
        CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
    );
    repository
        .record_combat_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_tutorial",
                "combat_state",
                0,
                "p08_combat_start",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                state_json: combat.persistence_json().unwrap(),
                attacker_roll: None,
                defender_roll: None,
                damage_roll: None,
                medical_roll: None,
            },
        )
        .await
        .expect("persist combat start");
    let combat_attack = percentile_with_result(80, true);
    let combat_damage = damage_with_value(1, 6, 5, 6);
    let damage = combat
        .apply_damage(
            CHARACTER_ID,
            CombatActionKind::Firearm,
            CombatDefense::None,
            &combat_attack,
            None,
            Some(&combat_damage),
        )
        .unwrap();
    assert_eq!(damage.condition, CombatCondition::MajorWound);
    repository
        .record_combat_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_tutorial",
                "combat_state",
                1,
                "p08_combat_damage",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                state_json: combat.persistence_json().unwrap(),
                attacker_roll: Some(combat_attack),
                defender_roll: None,
                damage_roll: Some(combat_damage),
                medical_roll: None,
            },
        )
        .await
        .expect("persist combat damage transition");
    combat.end().unwrap();
    assert_eq!(combat.status(), CombatStatus::Ended);
    let combat_end_metadata = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "combat_p08_tutorial",
        "combat_state",
        2,
        "p08_combat_end",
        "party_visible",
        "not_applicable",
        "rules_engine_decision",
    );
    let combat_end_request = RecordCombatStateRequest {
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: SESSION_ID.to_owned(),
        state_json: combat.persistence_json().unwrap(),
        attacker_roll: None,
        defender_roll: None,
        damage_roll: None,
        medical_roll: None,
    };
    let combat_end_receipt = repository
        .record_combat_state(&combat_end_metadata, &combat_end_request)
        .await
        .expect("persist terminal combat state");
    assert_eq!(
        repository
            .record_combat_state(&combat_end_metadata, &combat_end_request)
            .await
            .expect("return the persisted combat receipt on exact retry"),
        combat_end_receipt
    );

    let forged_chase = ChaseState::start(
        "chase_p08_forged_start",
        vec![
            ChaseParticipant::new(CHARACTER_ID, ChaseRole::Pursuer, 20).unwrap(),
            ChaseParticipant::new("npc_marta", ChaseRole::Quarry, 1).unwrap(),
        ],
        4,
    )
    .expect("construct an internally valid but unauthorized chase");
    assert!(matches!(
        repository
            .record_chase_state(
                &metadata(
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "chase_p08_forged_start",
                    "chase_state",
                    0,
                    "p08_chase_forged_start",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordChaseStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: SESSION_ID.to_owned(),
                    state_json: forged_chase.persistence_json().unwrap(),
                    participant_rolls: Vec::new(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "chase_participant_authority"
        ))
    ));

    let mut chase = ChaseState::start(
        "chase_p08_tutorial",
        vec![
            ChaseParticipant::new(CHARACTER_ID, ChaseRole::Quarry, 8).unwrap(),
            ChaseParticipant::new("npc_marta", ChaseRole::Pursuer, 8).unwrap(),
        ],
        2,
    )
    .expect("start rules-engine chase aggregate");
    repository
        .record_chase_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "chase_p08_tutorial",
                "chase_state",
                0,
                "p08_chase_start",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordChaseStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                state_json: chase.persistence_json().unwrap(),
                participant_rolls: Vec::new(),
            },
        )
        .await
        .expect("persist chase start");
    let chase_rolls = vec![
        percentile_with_result(40, false),
        percentile_with_result(40, true),
    ];
    let chase_obstacle = ChaseObstacle::new("obstacle_collapsing_salt", 1).unwrap();
    chase.advance(&chase_rolls, Some(&chase_obstacle)).unwrap();
    assert_eq!(chase.status(), ChaseStatus::Caught);
    let chase_end_metadata = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "chase_p08_tutorial",
        "chase_state",
        1,
        "p08_chase_caught",
        "party_visible",
        "not_applicable",
        "rules_engine_decision",
    );
    let chase_end_request = RecordChaseStateRequest {
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: SESSION_ID.to_owned(),
        state_json: chase.persistence_json().unwrap(),
        participant_rolls: chase_rolls,
    };
    let chase_end_receipt = repository
        .record_chase_state(&chase_end_metadata, &chase_end_request)
        .await
        .expect("persist terminal chase state");
    assert_eq!(
        repository
            .record_chase_state(&chase_end_metadata, &chase_end_request)
            .await
            .expect("return the persisted chase receipt on exact retry"),
        chase_end_receipt
    );
    assert!(
        chase
            .advance(
                &[
                    percentile_with_result(40, true),
                    percentile_with_result(40, false),
                ],
                None,
            )
            .is_err(),
        "a terminal chase cannot resume under the same ID"
    );

    repository
        .change_session_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                SESSION_ID,
                "session",
                2,
                "p08_session_end",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            SESSION_ID,
            SessionState::Ended,
            NOW_MS + 8_000,
        )
        .await
        .expect("end the Tutorial Session");
    let gameplay_events_before_ended_session_write: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE event_type IN ('CombatStateRecorded', 'ChaseStateRecorded')",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        repository
            .record_combat_state(&combat_end_metadata, &combat_end_request)
            .await
            .expect("return the combat receipt when exact retry happens after session end"),
        combat_end_receipt
    );
    assert_eq!(
        repository
            .record_chase_state(&chase_end_metadata, &chase_end_request)
            .await
            .expect("return the chase receipt when exact retry happens after session end"),
        chase_end_receipt
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE event_type IN ('CombatStateRecorded', 'ChaseStateRecorded')",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        gameplay_events_before_ended_session_write,
        "exact retries after session end must return prior receipts without appending"
    );
    let blocked_combat = CombatState::start(
        "combat_p08_after_ending",
        vec![
            CombatantState::new(
                CHARACTER_ID,
                70,
                CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
                1,
                CombatSkillTargets::new(45, 35, 40, 30, 10).unwrap(),
                weapon_loadout(1, 5),
            )
            .unwrap(),
            CombatantState::new(
                "npc_marta",
                80,
                CombatHealth::new(8, 8, CombatCondition::Able).unwrap(),
                0,
                CombatSkillTargets::new(60, 80, 40, 30, 10).unwrap(),
                weapon_loadout(0, 5),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    assert!(matches!(
        repository
            .record_combat_state(
                &metadata(
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "combat_p08_after_ending",
                    "combat_state",
                    0,
                    "p08_combat_after_ending",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordCombatStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: SESSION_ID.to_owned(),
                    state_json: blocked_combat.persistence_json().unwrap(),
                    attacker_roll: None,
                    defender_roll: None,
                    damage_roll: None,
                    medical_roll: None,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "gameplay_session_state"
        ))
    ));
    let blocked_chase = ChaseState::start(
        "chase_p08_after_ending",
        vec![
            ChaseParticipant::new(CHARACTER_ID, ChaseRole::Quarry, 8).unwrap(),
            ChaseParticipant::new("npc_marta", ChaseRole::Pursuer, 8).unwrap(),
        ],
        2,
    )
    .unwrap();
    assert!(matches!(
        repository
            .record_chase_state(
                &metadata(
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "chase_p08_after_ending",
                    "chase_state",
                    0,
                    "p08_chase_after_ending",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordChaseStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: SESSION_ID.to_owned(),
                    state_json: blocked_chase.persistence_json().unwrap(),
                    participant_rolls: Vec::new(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "gameplay_session_state"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE event_type IN ('CombatStateRecorded', 'ChaseStateRecorded')",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        gameplay_events_before_ended_session_write,
        "an ended session must reject combat and chase before canonical append"
    );
    repository
        .start_session(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p08_later",
                "session",
                0,
                "p08_later_session_start",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: "session_p08_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p08_tutorial".to_owned(),
                scenario_id: "scenario_p08_tutorial".to_owned(),
                scene_id: "scene_p08_later".to_owned(),
                scene_key: "scene_basement".to_owned(),
                scene_name: "地下盐窖重访".to_owned(),
                started_at_unix_ms: NOW_MS + 10_000,
            },
        )
        .await
        .expect("start an interleaved later session before the source conclusion");
    let healed_later_combat = CombatState::start(
        "combat_p08_healed_later",
        vec![
            CombatantState::new(
                CHARACTER_ID,
                70,
                CombatHealth::new(10, 10, CombatCondition::Able).unwrap(),
                1,
                CombatSkillTargets::new(45, 35, 40, 30, 10).unwrap(),
                weapon_loadout(1, 5),
            )
            .unwrap(),
            CombatantState::new(
                "npc_marta",
                80,
                CombatHealth::new(8, 8, CombatCondition::Able).unwrap(),
                0,
                CombatSkillTargets::new(60, 80, 40, 30, 10).unwrap(),
                weapon_loadout(0, 5),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    assert!(matches!(
        repository
            .record_combat_state(
                &metadata(
                    AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "combat_p08_healed_later",
                    "combat_state",
                    0,
                    "p08_healed_later_combat",
                    "party_visible",
                    "not_applicable",
                    "rules_engine_decision",
                ),
                &RecordCombatStateRequest {
                    campaign_id: CAMPAIGN_ID.to_owned(),
                    session_id: "session_p08_later".to_owned(),
                    state_json: healed_later_combat.persistence_json().unwrap(),
                    attacker_roll: None,
                    defender_roll: None,
                    damage_roll: None,
                    medical_roll: None,
                },
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "combat_participant_authority"
        ))
    ));
    let mut unfinished_later_combat = CombatState::start(
        "combat_p08_unfinished_later",
        vec![
            CombatantState::new(
                CHARACTER_ID,
                70,
                CombatHealth::new(5, 10, CombatCondition::MajorWound).unwrap(),
                1,
                CombatSkillTargets::new(45, 35, 40, 30, 10).unwrap(),
                weapon_loadout(1, 5),
            )
            .unwrap(),
            CombatantState::new(
                "npc_marta",
                80,
                CombatHealth::new(8, 8, CombatCondition::Able).unwrap(),
                0,
                CombatSkillTargets::new(60, 80, 40, 30, 10).unwrap(),
                weapon_loadout(0, 5),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    repository
        .record_combat_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_unfinished_later",
                "combat_state",
                0,
                "p08_unfinished_later_combat",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_later".to_owned(),
                state_json: unfinished_later_combat.persistence_json().unwrap(),
                attacker_roll: None,
                defender_roll: None,
                damage_roll: None,
                medical_roll: None,
            },
        )
        .await
        .expect("persist an unfinished combat before ending the later session");
    let mut unfinished_later_chase = ChaseState::start(
        "chase_p08_unfinished_later",
        vec![
            ChaseParticipant::new(CHARACTER_ID, ChaseRole::Quarry, 8).unwrap(),
            ChaseParticipant::new("npc_marta", ChaseRole::Pursuer, 8).unwrap(),
        ],
        2,
    )
    .unwrap();
    repository
        .record_chase_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "chase_p08_unfinished_later",
                "chase_state",
                0,
                "p08_unfinished_later_chase",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordChaseStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_later".to_owned(),
                state_json: unfinished_later_chase.persistence_json().unwrap(),
                participant_rolls: Vec::new(),
            },
        )
        .await
        .expect("persist an unfinished chase before ending the later session");
    let later_session_end_metadata = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "session_p08_later",
        "session",
        1,
        "p08_later_session_end",
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    );
    assert!(matches!(
        repository
            .change_session_state(
                &later_session_end_metadata,
                CAMPAIGN_ID,
                "session_p08_later",
                SessionState::Ended,
                NOW_MS + 11_000,
            )
            .await,
        Err(CoreDomainRepositoryError::InvalidInput(
            "session_gameplay_not_terminal"
        ))
    ));
    unfinished_later_combat.end().unwrap();
    repository
        .record_combat_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_unfinished_later",
                "combat_state",
                1,
                "p08_finish_later_combat",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordCombatStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_later".to_owned(),
                state_json: unfinished_later_combat.persistence_json().unwrap(),
                attacker_roll: None,
                defender_roll: None,
                damage_roll: None,
                medical_roll: None,
            },
        )
        .await
        .expect("finish the later Combat before ending its Session");
    let later_chase_rolls = vec![
        percentile_with_result(40, false),
        percentile_with_result(40, true),
    ];
    let later_chase_obstacle = ChaseObstacle::new("obstacle_later_collapse", 1).unwrap();
    unfinished_later_chase
        .advance(&later_chase_rolls, Some(&later_chase_obstacle))
        .unwrap();
    assert_eq!(unfinished_later_chase.status(), ChaseStatus::Caught);
    repository
        .record_chase_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "chase_p08_unfinished_later",
                "chase_state",
                1,
                "p08_finish_later_chase",
                "party_visible",
                "not_applicable",
                "rules_engine_decision",
            ),
            &RecordChaseStateRequest {
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_later".to_owned(),
                state_json: unfinished_later_chase.persistence_json().unwrap(),
                participant_rolls: later_chase_rolls,
            },
        )
        .await
        .expect("finish the later Chase before ending its Session");
    repository
        .change_session_state(
            &later_session_end_metadata,
            CAMPAIGN_ID,
            "session_p08_later",
            SessionState::Ended,
            NOW_MS + 11_000,
        )
        .await
        .expect("end the interleaved later Session after gameplay is terminal");
    let invalid_ending = repository
        .record_ending(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "ending_event_p08_invalid",
                "ending",
                0,
                "p08_ending_invalid",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordEndingRequest {
                ending_event_id: "ending_event_p08_invalid".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                ending_id: "ending_not_in_scenario".to_owned(),
                summary: "This ending is not defined by the scenario.".to_owned(),
                ended_at_unix_ms: NOW_MS + 9_000,
            },
        )
        .await;
    assert!(matches!(
        invalid_ending,
        Err(CoreDomainRepositoryError::InvalidInput(
            "ending_id_not_defined"
        ))
    ));
    let ending_metadata = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "ending_event_p08_tutorial",
        "ending",
        0,
        "p08_ending",
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    );
    let ending_request = RecordEndingRequest {
        ending_event_id: "ending_event_p08_tutorial".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: SESSION_ID.to_owned(),
        ending_id: "ending_expose_marta".to_owned(),
        summary: "The investigators expose Marta and preserve the archive.".to_owned(),
        ended_at_unix_ms: NOW_MS + 9_000,
    };
    let ending_receipt = repository
        .record_ending(&ending_metadata, &ending_request)
        .await
        .expect("record an allowed Tutorial ending");
    assert_eq!(
        repository
            .record_ending(&ending_metadata, &ending_request)
            .await
            .expect("return the persisted ending receipt on exact retry"),
        ending_receipt
    );
    let ending_events_before_duplicate: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'EndingRecorded'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let duplicate_session_ending = repository
        .record_ending(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "ending_event_p08_duplicate",
                "ending",
                0,
                "p08_ending_duplicate",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordEndingRequest {
                ending_event_id: "ending_event_p08_duplicate".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                ending_id: "ending_expose_marta".to_owned(),
                summary: "A conflicting second canonical ending.".to_owned(),
                ended_at_unix_ms: NOW_MS + 9_001,
            },
        )
        .await;
    assert!(matches!(
        duplicate_session_ending,
        Err(CoreDomainRepositoryError::Integrity(
            "ending_session_already_recorded"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store WHERE event_type = 'EndingRecorded'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        ending_events_before_duplicate,
        "a semantic duplicate ending must be rejected before canonical append"
    );
    let growth_events_before_unawarded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'CharacterGrowthApplied'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let unawarded_roll =
        server_roll_skill_growth(50).expect("server-owned unawarded growth evidence");
    let unawarded_growth = repository
        .record_growth(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "growth_event_p08_unawarded",
                "growth",
                0,
                "p08_growth_unawarded",
                "private_to_player",
                PLAYER_ID,
                "rules_engine_decision",
            ),
            &RecordGrowthRequest {
                growth_event_id: "growth_event_p08_unawarded".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                ending_event_id: "ending_event_p08_tutorial".to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                source_sheet_version_id: "sheet_p08_evelyn_v2".to_owned(),
                new_sheet_version_id: "sheet_p08_evelyn_v3_unawarded".to_owned(),
                skill_name: "Dodge".to_owned(),
                growth_rolls: unawarded_roll.evidence().clone(),
            },
        )
        .await;
    assert!(matches!(
        unawarded_growth,
        Err(CoreDomainRepositoryError::InvalidInput(
            "growth_skill_not_awarded"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE event_type = 'CharacterGrowthApplied'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        growth_events_before_unawarded,
        "an unawarded skill must be rejected before canonical append"
    );
    let growth_roll = server_roll_skill_growth(70).expect("server-owned COC7 growth rolls");
    let growth_after = growth_roll.outcome().skill_after;
    let growth_metadata = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "growth_event_p08_tutorial",
        "growth",
        0,
        "p08_growth",
        "private_to_player",
        PLAYER_ID,
        "rules_engine_decision",
    );
    let growth_request = RecordGrowthRequest {
        growth_event_id: "growth_event_p08_tutorial".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: SESSION_ID.to_owned(),
        ending_event_id: "ending_event_p08_tutorial".to_owned(),
        character_id: CHARACTER_ID.to_owned(),
        source_sheet_version_id: "sheet_p08_evelyn_v2".to_owned(),
        new_sheet_version_id: "sheet_p08_evelyn_v3".to_owned(),
        skill_name: "Library Use".to_owned(),
        growth_rolls: growth_roll.evidence().clone(),
    };
    let growth_receipt = repository
        .record_growth(&growth_metadata, &growth_request)
        .await
        .expect("apply server-generated growth to a new locked Sheet version");
    assert_eq!(
        repository
            .record_growth(&growth_metadata, &growth_request)
            .await
            .expect("return the persisted growth receipt on exact retry"),
        growth_receipt
    );
    let growth_events_before_duplicate: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'CharacterGrowthApplied'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let duplicate_growth_roll =
        server_roll_skill_growth(growth_after).expect("server-owned duplicate growth evidence");
    let duplicate_skill_growth = repository
        .record_growth(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "growth_event_p08_duplicate",
                "growth",
                0,
                "p08_growth_duplicate",
                "private_to_player",
                PLAYER_ID,
                "rules_engine_decision",
            ),
            &RecordGrowthRequest {
                growth_event_id: "growth_event_p08_duplicate".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: SESSION_ID.to_owned(),
                ending_event_id: "ending_event_p08_tutorial".to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                source_sheet_version_id: "sheet_p08_evelyn_v3".to_owned(),
                new_sheet_version_id: "sheet_p08_evelyn_v4_duplicate".to_owned(),
                skill_name: "Library Use".to_owned(),
                growth_rolls: duplicate_growth_roll.evidence().clone(),
            },
        )
        .await;
    assert!(matches!(
        duplicate_skill_growth,
        Err(CoreDomainRepositoryError::Integrity(
            "growth_skill_already_recorded"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE event_type = 'CharacterGrowthApplied'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        growth_events_before_duplicate,
        "semantic duplicate growth must be rejected before canonical append"
    );

    repository
        .request_reconsideration(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "reconsideration_p08_tutorial",
                "reconsideration",
                0,
                "p08_reconsideration_request",
                "party_visible",
                "not_applicable",
                "user_statement",
            ),
            &RequestReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                original_event_sequence: campaign_event_sequence,
                requested_by: PLAYER_ID.to_owned(),
                reason: "Review the opening archive ruling".to_owned(),
            },
        )
        .await
        .expect("append a reconsideration request without rewriting history");
    repository
        .review_reconsideration(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_tutorial",
                "reconsideration",
                1,
                "p08_reconsideration_review",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ReviewReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                review_event_id: "review_event_p08_tutorial".to_owned(),
                review_summary: "The first ruling omitted the recovered signature".to_owned(),
            },
        )
        .await
        .expect("append the reconsideration review");
    repository
        .resolve_reconsideration(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_tutorial",
                "reconsideration",
                2,
                "p08_reconsideration_resolve",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ResolveReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_tutorial".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                resolution_event_id: "resolution_event_p08_tutorial".to_owned(),
                outcome: ReconsiderationOutcome::Corrected,
                resolution: "Append a corrected ruling that admits the signature".to_owned(),
                corrected_event_type: Some("RulingCorrected".to_owned()),
                corrected_payload_json: Some(
                    r#"{"ruling":"signature admitted","supersedes_sequence":1}"#.to_owned(),
                ),
            },
        )
        .await
        .expect("append the correction while retaining the original event");

    repository
        .record_ending(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "ending_event_p08_later",
                "ending",
                0,
                "p08_later_ending",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordEndingRequest {
                ending_event_id: "ending_event_p08_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_later".to_owned(),
                ending_id: "ending_expose_marta".to_owned(),
                summary: "A later session confirms the archive findings.".to_owned(),
                ended_at_unix_ms: NOW_MS + 12_000,
            },
        )
        .await
        .expect("record the later scenario-defined ending");
    let later_growth_roll =
        server_roll_skill_growth(growth_after).expect("server-owned later growth rolls");
    let later_growth_after = later_growth_roll.outcome().skill_after;
    repository
        .record_growth(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "growth_event_p08_later",
                "growth",
                0,
                "p08_later_growth",
                "private_to_player",
                PLAYER_ID,
                "rules_engine_decision",
            ),
            &RecordGrowthRequest {
                growth_event_id: "growth_event_p08_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                session_id: "session_p08_later".to_owned(),
                ending_event_id: "ending_event_p08_later".to_owned(),
                character_id: CHARACTER_ID.to_owned(),
                source_sheet_version_id: "sheet_p08_evelyn_v3".to_owned(),
                new_sheet_version_id: "sheet_p08_evelyn_v4".to_owned(),
                skill_name: "Library Use".to_owned(),
                growth_rolls: later_growth_roll.evidence().clone(),
            },
        )
        .await
        .expect("apply a later growth that is outside the source-session cutoff");

    repository
        .request_reconsideration(
            &metadata(
                AUTHORITY_ID,
                PLAYER_ID,
                "investigator",
                "reconsideration_p08_after_later",
                "reconsideration",
                0,
                "p08_reconsideration_after_later_request",
                "party_visible",
                "not_applicable",
                "user_statement",
            ),
            &RequestReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_after_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                original_event_sequence: campaign_event_sequence,
                requested_by: PLAYER_ID.to_owned(),
                reason: "Confirm the old campaign ruling after later play".to_owned(),
            },
        )
        .await
        .expect("append a late reconsideration of source-session history");
    repository
        .review_reconsideration(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_after_later",
                "reconsideration",
                1,
                "p08_reconsideration_after_later_review",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ReviewReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_after_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                review_event_id: "review_event_p08_after_later".to_owned(),
                review_summary: "Later play does not change the original ruling".to_owned(),
            },
        )
        .await
        .expect("review the late reconsideration");
    repository
        .resolve_reconsideration(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "reconsideration_p08_after_later",
                "reconsideration",
                2,
                "p08_reconsideration_after_later_resolve",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &ResolveReconsiderationRequest {
                reconsideration_id: "reconsideration_p08_after_later".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                resolution_event_id: "resolution_event_p08_after_later".to_owned(),
                outcome: ReconsiderationOutcome::Upheld,
                resolution: "The original campaign ruling remains valid".to_owned(),
                corrected_event_type: None,
                corrected_payload_json: None,
            },
        )
        .await
        .expect("resolve the late reconsideration without widening the source cutoff");

    repository
        .start_session(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p08_concurrency",
                "session",
                0,
                "p08_concurrent_session_start",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            &StartSessionRequest {
                session_id: "session_p08_concurrency".to_owned(),
                campaign_id: CAMPAIGN_ID.to_owned(),
                room_id: "room_p08_tutorial".to_owned(),
                scenario_id: "scenario_p08_tutorial".to_owned(),
                scene_id: "scene_p08_concurrency".to_owned(),
                scene_key: "scene_concurrent_conclusion".to_owned(),
                scene_name: "并发结算验证".to_owned(),
                started_at_unix_ms: NOW_MS + 13_000,
            },
        )
        .await
        .expect("start a dedicated concurrent conclusion session");
    repository
        .change_session_state(
            &metadata(
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "session_p08_concurrency",
                "session",
                1,
                "p08_concurrent_session_end",
                "party_visible",
                "not_applicable",
                "human_keeper_statement",
            ),
            CAMPAIGN_ID,
            "session_p08_concurrency",
            SessionState::Ended,
            NOW_MS + 14_000,
        )
        .await
        .expect("end the dedicated concurrent conclusion session");
    let ending_count_before_race: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'EndingRecorded'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let ending_race_metadata_a = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "ending_event_p08_race_a",
        "ending",
        0,
        "p08_ending_race_a",
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    );
    let ending_race_metadata_b = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "ending_event_p08_race_b",
        "ending",
        0,
        "p08_ending_race_b",
        "party_visible",
        "not_applicable",
        "human_keeper_statement",
    );
    let ending_race_request_a = RecordEndingRequest {
        ending_event_id: "ending_event_p08_race_a".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: "session_p08_concurrency".to_owned(),
        ending_id: "ending_expose_marta".to_owned(),
        summary: "Concurrent ending candidate A.".to_owned(),
        ended_at_unix_ms: NOW_MS + 15_000,
    };
    let ending_race_request_b = RecordEndingRequest {
        ending_event_id: "ending_event_p08_race_b".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: "session_p08_concurrency".to_owned(),
        ending_id: "ending_expose_marta".to_owned(),
        summary: "Concurrent ending candidate B.".to_owned(),
        ended_at_unix_ms: NOW_MS + 15_001,
    };
    let (ending_race_a, ending_race_b) = tokio::join!(
        repository.record_ending(&ending_race_metadata_a, &ending_race_request_a),
        repository.record_ending(&ending_race_metadata_b, &ending_race_request_b),
    );
    assert_eq!(
        usize::from(ending_race_a.is_ok()) + usize::from(ending_race_b.is_ok()),
        1,
        "the session advisory lock must allow exactly one ending"
    );
    let ending_race_failure = if ending_race_a.is_err() {
        &ending_race_a
    } else {
        &ending_race_b
    };
    assert!(matches!(
        ending_race_failure,
        Err(CoreDomainRepositoryError::Integrity(
            "ending_session_already_recorded"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store WHERE event_type = 'EndingRecorded'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        ending_count_before_race + 1,
        "the losing concurrent ending must not append canonical history"
    );
    let concurrent_ending_event_id: String = sqlx::query_scalar(
        "SELECT ending_event_id FROM public.ending_events \
         WHERE session_id = 'session_p08_concurrency'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();

    let growth_count_before_race: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store WHERE event_type = 'CharacterGrowthApplied'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    let concurrent_library_roll =
        server_roll_skill_growth(later_growth_after).expect("concurrent Library Use rolls");
    let concurrent_psychology_roll =
        server_roll_skill_growth(55).expect("concurrent Psychology rolls");
    let growth_race_metadata_a = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "growth_event_p08_race_library",
        "growth",
        0,
        "p08_growth_race_library",
        "private_to_player",
        PLAYER_ID,
        "rules_engine_decision",
    );
    let growth_race_metadata_b = metadata(
        AUTHORITY_ID,
        KEEPER_ID,
        "human_keeper",
        "growth_event_p08_race_psychology",
        "growth",
        0,
        "p08_growth_race_psychology",
        "private_to_player",
        PLAYER_ID,
        "rules_engine_decision",
    );
    let growth_race_request_a = RecordGrowthRequest {
        growth_event_id: "growth_event_p08_race_library".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: "session_p08_concurrency".to_owned(),
        ending_event_id: concurrent_ending_event_id.clone(),
        character_id: CHARACTER_ID.to_owned(),
        source_sheet_version_id: "sheet_p08_evelyn_v4".to_owned(),
        new_sheet_version_id: "sheet_p08_evelyn_v5_library".to_owned(),
        skill_name: "Library Use".to_owned(),
        growth_rolls: concurrent_library_roll.evidence().clone(),
    };
    let growth_race_request_b = RecordGrowthRequest {
        growth_event_id: "growth_event_p08_race_psychology".to_owned(),
        campaign_id: CAMPAIGN_ID.to_owned(),
        session_id: "session_p08_concurrency".to_owned(),
        ending_event_id: concurrent_ending_event_id,
        character_id: CHARACTER_ID.to_owned(),
        source_sheet_version_id: "sheet_p08_evelyn_v4".to_owned(),
        new_sheet_version_id: "sheet_p08_evelyn_v5_psychology".to_owned(),
        skill_name: "Psychology".to_owned(),
        growth_rolls: concurrent_psychology_roll.evidence().clone(),
    };
    let (growth_race_a, growth_race_b) = tokio::join!(
        repository.record_growth(&growth_race_metadata_a, &growth_race_request_a),
        repository.record_growth(&growth_race_metadata_b, &growth_race_request_b),
    );
    assert_eq!(
        usize::from(growth_race_a.is_ok()) + usize::from(growth_race_b.is_ok()),
        1,
        "the character advisory lock must serialize growth from one source sheet"
    );
    let growth_race_failure = if growth_race_a.is_err() {
        &growth_race_a
    } else {
        &growth_race_b
    };
    assert!(matches!(
        growth_race_failure,
        Err(CoreDomainRepositoryError::NotFound("growth_source"))
            | Err(CoreDomainRepositoryError::Integrity(
                "growth_source_not_current"
            ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE event_type = 'CharacterGrowthApplied'",
        )
        .fetch_one(&primary)
        .await
        .unwrap(),
        growth_count_before_race + 1,
        "the losing concurrent growth must not append canonical history"
    );

    create_campaign(
        &repository,
        CHILD_CAMPAIGN_ID,
        CHILD_AUTHORITY_ID,
        "room_p08_tutorial_fork",
        "p08_child_campaign_create",
    )
    .await;
    let snapshot = repository
        .preview_campaign_fork(CAMPAIGN_ID, SESSION_ID, KEEPER_ID)
        .await
        .expect("compute the canonical public fork snapshot");
    assert!(
        snapshot
            .canonical_snapshot_json
            .contains("ReconsiderationCorrected"),
        "the fork must include a completed correction chain for source-session history"
    );
    assert!(
        snapshot
            .canonical_snapshot_json
            .contains("reconsideration_p08_after_later")
            && snapshot
                .canonical_snapshot_json
                .contains("ReconsiderationUpheld"),
        "a relevant late reconsideration chain must be included separately"
    );
    assert!(
        !snapshot
            .canonical_snapshot_json
            .contains("session_p08_later")
            && !snapshot
                .canonical_snapshot_json
                .contains("ending_event_p08_later"),
        "a late reconsideration must not widen the base cutoff to unrelated later-session events"
    );
    assert!(!snapshot.canonical_snapshot_json.contains("keeper_note"));
    assert!(!snapshot.canonical_snapshot_json.contains("private_message"));
    assert!(!snapshot.canonical_snapshot_json.contains("ai_internal"));
    let snapshot_json: serde_json::Value =
        serde_json::from_str(&snapshot.canonical_snapshot_json).unwrap();
    let fork_characters = snapshot_json
        .pointer("/state/character_state")
        .and_then(serde_json::Value::as_array)
        .expect("fork snapshot characters");
    let fork_growth_awards = snapshot_json
        .pointer("/state/conclusion_state/0/growth_awards")
        .and_then(serde_json::Value::as_array)
        .expect("fork snapshot must carry the source ending's growth awards");
    assert_eq!(
        fork_growth_awards
            .iter()
            .filter_map(|award| { award.get("skill_name").and_then(serde_json::Value::as_str) })
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["Library Use", "Psychology"]),
        "growth authorization must be content-addressed into the fork snapshot"
    );
    assert_eq!(
        snapshot_json
            .pointer("/state/conclusion_state/0/consumed_growth_awards")
            .and_then(serde_json::Value::as_array)
            .expect("fork snapshot consumed Growth markers")
            .iter()
            .filter_map(|consumed| {
                Some((
                    consumed.get("character_id")?.as_str()?,
                    consumed.get("skill_name")?.as_str()?,
                ))
            })
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([(CHARACTER_ID, "Library Use")]),
        "the source snapshot must bind settled awards to the source character"
    );
    assert_eq!(
        fork_characters.len(),
        1,
        "an investigator updated after the cutoff must be replayed, not omitted"
    );
    assert_eq!(
        fork_characters[0]
            .pointer("/current_sheet/sheet_json/skills/Library Use")
            .and_then(serde_json::Value::as_u64),
        Some(u64::from(growth_after)),
        "the source-session fork must retain the sheet as of its canonical cutoff"
    );
    assert_eq!(
        fork_characters[0]
            .pointer("/current_sheet/sheet_json/sanity_state/current_sanity")
            .and_then(serde_json::Value::as_u64),
        Some(u64::from(65 - sanity_loss)),
        "the selected-session flat PlayerActionSubmitted payload and its dependent SAN loss must survive fork replay"
    );
    repository
        .record_campaign_fork(
            &metadata(
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "fork_p08_tutorial",
                "campaign_fork",
                0,
                "p08_fork",
                "keeper_only",
                "not_applicable",
                "human_keeper_statement",
            ),
            &RecordCampaignForkRequest {
                fork_id: "fork_p08_tutorial".to_owned(),
                parent_campaign_id: CAMPAIGN_ID.to_owned(),
                child_campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                source_session_id: SESSION_ID.to_owned(),
                snapshot_hash: snapshot.snapshot_hash.clone(),
                reason: "Preserve the corrected public branch".to_owned(),
                copy_scopes: snapshot.copy_scopes.clone(),
            },
        )
        .await
        .expect("materialize the fork as child-owned durable state");
    let source_after_fork = repository
        .preview_campaign_fork(CAMPAIGN_ID, SESSION_ID, KEEPER_ID)
        .await
        .expect("re-read the source after child materialization");
    assert_eq!(
        source_after_fork, snapshot,
        "fork materialization must not mutate the source Campaign"
    );

    let parent_projection = sqlx::query(
        r#"
        SELECT
          (SELECT status FROM public.combat_states
            WHERE combat_id = 'combat_p08_tutorial') AS combat_status,
          (SELECT state_json -> 'participants' -> 0 ->> 'condition'
             FROM public.combat_states
            WHERE combat_id = 'combat_p08_tutorial') AS combat_condition,
          (SELECT status FROM public.chase_states
            WHERE chase_id = 'chase_p08_tutorial') AS chase_status,
          (SELECT state FROM public.reconsiderations
            WHERE reconsideration_id = 'reconsideration_p08_tutorial')
              AS reconsideration_state,
          (SELECT outcome FROM public.reconsiderations
            WHERE reconsideration_id = 'reconsideration_p08_tutorial')
              AS reconsideration_outcome,
          (SELECT sheet_json -> 'skills' ->> 'Library Use'
             FROM public.character_sheet_versions
            WHERE sheet_version_id = 'sheet_p08_evelyn_v3') AS growth_skill,
          (SELECT random_source FROM public.growth_events
            WHERE growth_event_id = 'growth_event_p08_tutorial')
              AS growth_random_source,
          (SELECT server_roll_id FROM public.growth_events
            WHERE growth_event_id = 'growth_event_p08_tutorial')
              AS growth_server_roll_id,
          (SELECT increase_roll_id FROM public.growth_events
            WHERE growth_event_id = 'growth_event_p08_tutorial')
              AS growth_increase_roll_id
        "#,
    )
    .fetch_one(&primary)
    .await
    .expect("load the completed Tutorial read models");
    assert_eq!(parent_projection.get::<String, _>("combat_status"), "ENDED");
    assert_eq!(
        parent_projection.get::<String, _>("combat_condition"),
        "MAJOR_WOUND"
    );
    assert_eq!(parent_projection.get::<String, _>("chase_status"), "CAUGHT");
    assert_eq!(
        parent_projection.get::<String, _>("reconsideration_state"),
        "RESOLVED"
    );
    assert_eq!(
        parent_projection.get::<String, _>("reconsideration_outcome"),
        "CORRECTED"
    );
    assert_eq!(
        parent_projection
            .get::<String, _>("growth_skill")
            .parse::<u8>()
            .unwrap(),
        growth_after
    );
    assert_eq!(
        parent_projection.get::<String, _>("growth_random_source"),
        "SERVER_OS_CSPRNG"
    );
    assert_eq!(
        parent_projection.get::<String, _>("growth_server_roll_id"),
        growth_roll.evidence().improvement_check().roll_id()
    );
    assert_eq!(
        parent_projection
            .get::<Option<String>, _>("growth_increase_roll_id")
            .as_deref(),
        growth_roll.evidence().increase().map(|roll| roll.roll_id())
    );

    let child_counts: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
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
          (SELECT count(*) FROM public.combat_states WHERE campaign_id = $1),
          (SELECT count(*) FROM public.chase_states WHERE campaign_id = $1),
          (SELECT count(*) FROM public.ending_events WHERE campaign_id = $1)
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("load child fork materialization");
    assert_eq!(
        (
            child_counts.0,
            child_counts.1,
            child_counts.2,
            child_counts.3,
            child_counts.4
        ),
        (1, 1, 1, 2, 1),
        "fork must materialize real child-owned scenario, character, session, scenes and manifest"
    );
    assert!(child_counts.5 > 0);
    assert_eq!(
        (
            child_counts.6,
            child_counts.7,
            child_counts.8,
            child_counts.9
        ),
        (1, 1, 1, 1),
        "clue, combat, chase and conclusion copy scopes must be queryable in the child"
    );
    let child_visibility = sqlx::query(
        r#"
        SELECT
          (SELECT visibility_label::TEXT FROM public.scenarios
            WHERE campaign_id = $1) AS scenario_visibility,
          (SELECT visibility_label::TEXT FROM public.characters
            WHERE campaign_id = $1) AS character_visibility,
          (SELECT visibility_subject FROM public.characters
            WHERE campaign_id = $1) AS character_subject,
          (SELECT visibility_label::TEXT
             FROM public.character_sheet_versions
            WHERE campaign_id = $1) AS sheet_visibility,
          (SELECT visibility_subject
             FROM public.character_sheet_versions
            WHERE campaign_id = $1) AS sheet_subject,
          (SELECT visibility_label::TEXT FROM core_domain.sessions
            WHERE campaign_id = $1) AS session_visibility,
          (SELECT sheet_json -> 'skills' ->> 'Library Use'
             FROM public.character_sheet_versions
            WHERE campaign_id = $1) AS fork_growth_skill,
          (SELECT sheet_json #>> '{sanity_state,current_sanity}'
             FROM public.character_sheet_versions
            WHERE campaign_id = $1) AS fork_sanity,
          (SELECT sheet_json -> 'skills' ->> 'Library Use'
             FROM public.character_sheet_versions
            WHERE sheet_version_id = 'sheet_p08_evelyn_v4')
              AS current_parent_growth_skill
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("load fork visibility and cutoff state");
    assert_eq!(
        child_visibility.get::<String, _>("scenario_visibility"),
        "keeper_only"
    );
    assert_eq!(
        child_visibility.get::<String, _>("character_visibility"),
        "private_to_player"
    );
    assert_eq!(
        child_visibility.get::<String, _>("character_subject"),
        PLAYER_ID
    );
    assert_eq!(
        child_visibility.get::<String, _>("sheet_visibility"),
        "private_to_player"
    );
    assert_eq!(
        child_visibility.get::<String, _>("sheet_subject"),
        PLAYER_ID
    );
    assert_eq!(
        child_visibility.get::<String, _>("session_visibility"),
        "party_visible"
    );
    assert_eq!(
        child_visibility
            .get::<String, _>("fork_growth_skill")
            .parse::<u8>()
            .unwrap(),
        growth_after
    );
    assert_eq!(
        child_visibility
            .get::<String, _>("fork_sanity")
            .parse::<u8>()
            .unwrap(),
        65 - sanity_loss,
        "fork materialization must persist the selected-session SAN result"
    );
    assert_eq!(
        child_visibility
            .get::<String, _>("current_parent_growth_skill")
            .parse::<u8>()
            .unwrap(),
        later_growth_after
    );
    let child_growth_source = sqlx::query(
        r#"
        SELECT child_session.session_id,
               child_ending.ending_event_id,
               child_character.character_id,
               child_sheet.sheet_version_id,
               child_sheet.sheet_json,
               child_scenario.document_json
          FROM core_domain.sessions AS child_session
          JOIN public.scenarios AS child_scenario
            ON child_scenario.scenario_id = child_session.scenario_id
           AND child_scenario.campaign_id = child_session.campaign_id
          JOIN public.ending_events AS child_ending
            ON child_ending.session_id = child_session.session_id
           AND child_ending.campaign_id = child_session.campaign_id
          JOIN public.characters AS child_character
            ON child_character.campaign_id = child_session.campaign_id
          JOIN public.character_sheet_versions AS child_sheet
            ON child_sheet.character_id = child_character.character_id
           AND child_sheet.campaign_id = child_character.campaign_id
           AND child_sheet.version = child_character.current_sheet_version
         WHERE child_session.campaign_id = $1
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("load the child-owned conclusion and current character sheet");
    let child_scenario_document: serde_json::Value = child_growth_source.get("document_json");
    let child_awarded_skills = child_scenario_document
        .pointer("/endings/0/growth_awards")
        .and_then(serde_json::Value::as_array)
        .expect("materialized child scenario growth awards")
        .iter()
        .filter_map(|award| award.get("skill_name").and_then(serde_json::Value::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        child_awarded_skills,
        BTreeSet::from(["Library Use", "Psychology"]),
        "the child scenario must retain the source ending's complete award authorization"
    );
    let child_sheet_json: serde_json::Value = child_growth_source.get("sheet_json");
    let child_character_id: String = child_growth_source.get("character_id");
    assert_eq!(
        child_scenario_document
            .pointer("/endings/0/growth_awards")
            .and_then(serde_json::Value::as_array)
            .and_then(|awards| {
                awards.iter().find(|award| {
                    award.get("skill_name").and_then(serde_json::Value::as_str)
                        == Some("Library Use")
                })
            })
            .and_then(|award| award.get("consumed_by_character_ids"))
            .and_then(serde_json::Value::as_array)
            .expect("materialized consumed Library Use marker"),
        &[serde_json::Value::String(child_character_id.clone())],
        "the consumed source award must be rebound to the child-owned character ID"
    );
    let child_library_before = child_sheet_json
        .pointer("/skills/Library Use")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .expect("forked Library Use skill");
    let duplicate_child_library_roll = server_roll_skill_growth(child_library_before)
        .expect("server-owned duplicate child growth evidence");
    let child_growth_events_before_duplicate: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = $1 AND event_type = 'CharacterGrowthApplied'",
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert!(matches!(
        repository
            .record_growth(
                &metadata(
                    CHILD_AUTHORITY_ID,
                    KEEPER_ID,
                    "human_keeper",
                    "growth_event_p08_child_duplicate_library",
                    "growth",
                    0,
                    "p08_child_duplicate_library",
                    "private_to_player",
                    PLAYER_ID,
                    "rules_engine_decision",
                ),
                &RecordGrowthRequest {
                    growth_event_id: "growth_event_p08_child_duplicate_library".to_owned(),
                    campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                    session_id: child_growth_source.get("session_id"),
                    ending_event_id: child_growth_source.get("ending_event_id"),
                    character_id: child_character_id.clone(),
                    source_sheet_version_id: child_growth_source.get("sheet_version_id"),
                    new_sheet_version_id: "sheet_p08_child_duplicate_library".to_owned(),
                    skill_name: "Library Use".to_owned(),
                    growth_rolls: duplicate_child_library_roll.evidence().clone(),
                },
            )
            .await,
        Err(CoreDomainRepositoryError::Integrity(
            "growth_skill_already_recorded"
        ))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM public.event_store \
             WHERE campaign_id = $1 AND event_type = 'CharacterGrowthApplied'",
        )
        .bind(CHILD_CAMPAIGN_ID)
        .fetch_one(&primary)
        .await
        .unwrap(),
        child_growth_events_before_duplicate,
        "a consumed source award must be rejected before child canonical append"
    );
    let child_psychology_before = child_sheet_json
        .pointer("/skills/Psychology")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .expect("forked Psychology skill");
    let child_growth_roll = server_roll_skill_growth(child_psychology_before)
        .expect("server-owned child fork growth evidence");
    let child_psychology_after = child_growth_roll.outcome().skill_after;
    repository
        .record_growth(
            &metadata(
                CHILD_AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "growth_event_p08_child_psychology",
                "growth",
                0,
                "p08_child_growth_after_fork",
                "private_to_player",
                PLAYER_ID,
                "rules_engine_decision",
            ),
            &RecordGrowthRequest {
                growth_event_id: "growth_event_p08_child_psychology".to_owned(),
                campaign_id: CHILD_CAMPAIGN_ID.to_owned(),
                session_id: child_growth_source.get("session_id"),
                ending_event_id: child_growth_source.get("ending_event_id"),
                character_id: child_growth_source.get("character_id"),
                source_sheet_version_id: child_growth_source.get("sheet_version_id"),
                new_sheet_version_id: "sheet_p08_child_psychology_growth".to_owned(),
                skill_name: "Psychology".to_owned(),
                growth_rolls: child_growth_roll.evidence().clone(),
            },
        )
        .await
        .expect("settle an unconsumed ending award entirely inside the forked campaign");
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT sheet_json -> 'skills' ->> 'Psychology' \
             FROM public.character_sheet_versions \
             WHERE sheet_version_id = 'sheet_p08_child_psychology_growth'",
        )
        .fetch_one(&primary)
        .await
        .expect("load the child growth result")
        .parse::<u8>()
        .unwrap(),
        child_psychology_after,
        "a fork created before this award is settled must remain growth-capable"
    );
    let materialized_visibility: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT DISTINCT visibility_label, visibility_subject, data_subject_id
          FROM public.event_store
         WHERE campaign_id = $1
           AND event_type = 'CampaignForkMaterialized'
         ORDER BY visibility_label, visibility_subject, data_subject_id
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_all(&primary)
    .await
    .expect("load per-event fork visibility");
    assert_eq!(
        materialized_visibility,
        vec![
            (
                "keeper_only".to_owned(),
                "not_applicable".to_owned(),
                "not_applicable".to_owned()
            ),
            (
                "party_visible".to_owned(),
                "not_applicable".to_owned(),
                "not_applicable".to_owned()
            ),
            (
                "private_to_player".to_owned(),
                PLAYER_ID.to_owned(),
                PLAYER_ID.to_owned()
            )
        ]
    );
    let private_fork_crypto_binding: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
          count(*) FILTER (
            WHERE event.visibility_label = 'private_to_player'
          ),
          count(*) FILTER (
            WHERE event.visibility_label = 'private_to_player'
              AND event.data_subject_id = event.visibility_subject
              AND subject_key.subject_id = event.data_subject_id
              AND subject_key.key_reference = event.payload_key_reference
              AND subject_key.wrapped_key IS NOT NULL
              AND subject_key.destroyed_at IS NULL
          )
          FROM public.event_store AS event
          LEFT JOIN public.privacy_subject_keys AS subject_key
            ON subject_key.subject_id = event.data_subject_id
         WHERE event.campaign_id = $1
           AND event.event_type = 'CampaignForkMaterialized'
        "#,
    )
    .bind(CHILD_CAMPAIGN_ID)
    .fetch_one(&primary)
    .await
    .expect("verify private fork payload crypto binding");
    assert!(private_fork_crypto_binding.0 > 0);
    assert_eq!(
        private_fork_crypto_binding.1, private_fork_crypto_binding.0,
        "every owner-private fork payload must use that player's live subject key"
    );

    let actual_event_types = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT event_type FROM public.event_store ORDER BY event_type",
    )
    .fetch_all(&primary)
    .await
    .expect("read the actual canonical event types")
    .into_iter()
    .collect::<BTreeSet<_>>();
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

#[test]
fn tutorial_rejects_early_ending_and_private_fork_scope() {
    use trpg_domain_core::ddd::AuthorityMode;
    use trpg_domain_core::fork_canon_lineage::{
        calculate_snapshot_hash, fork_campaign, CampaignForkRequest, CampaignForkSnapshot,
        CanonStatus, CopyScope,
    };
    use trpg_runtime::session_runtime::{CampaignConclusion, DurableSessionState};

    assert!(
        CampaignConclusion::begin(CAMPAIGN_ID, SESSION_ID, DurableSessionState::Active).is_err(),
        "an active Session cannot be declared concluded"
    );

    let parent = trpg_test_support::authority_contract_with_owner(
        CAMPAIGN_ID,
        AuthorityMode::HumanKp,
        KEEPER_ID,
        1,
    )
    .unwrap();
    let state = r#"{"public_events":[]}"#;
    let snapshot = CampaignForkSnapshot::verified(
        CAMPAIGN_ID,
        SESSION_ID,
        state,
        calculate_snapshot_hash(state),
    )
    .unwrap();
    let request = CampaignForkRequest::new(
        CAMPAIGN_ID,
        SESSION_ID,
        CHILD_CAMPAIGN_ID,
        AuthorityMode::HumanKp,
        KEEPER_ID,
        "attempt private copy",
        snapshot.snapshot_hash.clone(),
    )
    .unwrap()
    .with_scope(
        CanonStatus::WhatIf,
        vec![CopyScope::PublicEvents, CopyScope::KeeperNotes],
    )
    .unwrap();
    assert!(
        fork_campaign(&parent, &request, &snapshot, &[]).is_err(),
        "a fork cannot opt private Keeper notes back into the copy scope"
    );
}
