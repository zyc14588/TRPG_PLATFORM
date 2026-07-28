use std::env;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::persistence_postgresql::{
    AcceptInviteRequest, AuthorityContractSnapshot, CoreCommandMetadata, CoreDomainClock,
    CoreDomainRepository, CoreDomainRepositoryError, CreateCampaignRequest, CreateCharacterRequest,
    ImportScenarioRequest, IssueInviteRequest, PlayerActionDiceRecord, PlayerActionIntentRecord,
    RecordCampaignForkRequest, RecordChaseStateRequest, RecordCombatStateRequest,
    RecordEndingRequest, RecordGrowthRequest, RequestReconsiderationRequest,
    ResolveReconsiderationRequest, ReviewReconsiderationRequest, SanityExecutionRecord,
    StartSessionRequest, SubmitPlayerActionRequest, SwitchSceneRequest,
};
use trpg_domain_core::canonical_gameplay_state::validate_combat_server_roll_evidence;
use trpg_domain_core::domain_entities_value_objects::{
    MembershipRole, ReconsiderationOutcome, SessionState,
};
use trpg_domain_core::fork_canon_lineage::CopyScope;
use trpg_ruleset_coc7::character_combat_san_chase::parse_scenario_yaml;
use trpg_ruleset_coc7::chase_state_machine::{
    ChaseObstacle, ChaseParticipant, ChaseRole, ChaseState, ChaseStatus,
};
use trpg_ruleset_coc7::combat_state_machine::{
    CombatActionKind, CombatCondition, CombatDamageFormula, CombatDefense, CombatHealth,
    CombatMedicalSkill, CombatSkillTargets, CombatState, CombatStatus, CombatWeapon,
    CombatWeaponLoadout, CombatantState,
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
const RACE_CHILD_CAMPAIGN_ID: &str = "campaign_p08_fork_race_child";
const STATE_RACE_CHILD_CAMPAIGN_ID: &str = "campaign_p08_fork_state_race_child";
const EMPTY_P08_CAMPAIGN_ID: &str = "campaign_p08_empty_rebuild";
const KEEPER_ID: &str = "keeper_p06_schema";
const PLAYER_ID: &str = "player_p06_schema";
const OTHER_ID: &str = "other_p06_schema";
const CAMPAIGN_OWNER_ID: &str = "owner_p06_schema";
const AUTHORITY_ID: &str = "authority_campaign_p06_schema_1";
const CHILD_AUTHORITY_ID: &str = "authority_contract_campaign_p06_fork_child_1";
const RACE_CHILD_AUTHORITY_ID: &str = "authority_contract_campaign_p08_fork_race_child_1";
const STATE_RACE_CHILD_AUTHORITY_ID: &str =
    "authority_contract_campaign_p08_fork_state_race_child_1";
const EMPTY_P08_AUTHORITY_ID: &str = "authority_contract_campaign_p08_empty_rebuild_1";
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
                created_at_unix_ms: if campaign_id == CAMPAIGN_ID {
                    NOW_MS
                } else {
                    NOW_MS + 1
                },
                authority: authority(authority_id),
            },
        )
        .await
        .expect("create campaign and lock authority")
        .last_event_sequence
}

async fn persist_combat_turn_advance(
    repository: &CoreDomainRepository,
    combat: &mut CombatState,
    expected_version: i64,
    suffix: &str,
) -> String {
    let next_actor = combat
        .advance_turn()
        .expect("advance to the next capable combat actor")
        .to_owned();
    repository
        .record_combat_state(
            &metadata(
                CAMPAIGN_ID,
                AUTHORITY_ID,
                KEEPER_ID,
                "human_keeper",
                "combat_p08_schema",
                "combat_state",
                "combat.state.turn",
                expected_version,
                suffix,
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
        .expect("persist a turn-advance transition");
    next_actor
}
