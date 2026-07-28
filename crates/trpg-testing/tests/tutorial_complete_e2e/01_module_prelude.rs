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
            "Psychology": 55,
            "Fighting (Brawl)": 45,
            "Firearms (Handgun)": 35,
            "Dodge": 40,
            "First Aid": 30,
            "Medicine": 10
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
            "skill_target_sources": {
                "melee": "Fighting (Brawl)",
                "firearm": "Firearms (Handgun)",
                "dodge": "Dodge",
                "first_aid": "First Aid",
                "medicine": "Medicine"
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
