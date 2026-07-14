mod common;

use trpg_ruleset_coc7::character_combat_san_chase::{
    derive_character_stats, Coc7Characteristics, DamageBonus,
};
use trpg_ruleset_coc7::chase_state_machine::{advance_chase, record_chase_transition, ChaseStatus};
use trpg_ruleset_coc7::combat_state_machine::{apply_damage, record_combat_transition};
use trpg_ruleset_coc7::dice_roll_contract::{
    adjusted_percentile_roll, record_dice_roll_contract, success_level, DiceAdjustment,
    SuccessLevel,
};
use trpg_ruleset_coc7::investigation_clue_npc_time::{
    record_investigation_clue_npc_time_decision, resolve_clue_check, ClueImportance, ClueOutcome,
};
use trpg_ruleset_coc7::npc::{npc_secret_for_principal, NpcSecret};
use trpg_ruleset_coc7::san::{record_san_decision, resolve_san_check};
use trpg_shared_kernel::{FormalWritePath, PrincipalScope, ProvenanceKind, TrpgError};

const S05_STAGE_FIXTURE: &str =
    include_str!("../../../fixtures/stages/S05_stage_acceptance_fixture.v1.json.md");
const S05_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S05_coc7_roll_san_combat_chase_expected.current.json.md"
);
const CHARACTER_FIXTURE: &str =
    include_str!("../../../fixtures/rules/coc7_character_creation_review.v1.json.md");
const DICE_FIXTURE: &str = include_str!("../../../fixtures/rules/coc7_dice_matrix.v1.json.md");
const SAN_COMBAT_CHASE_FIXTURE: &str =
    include_str!("../../../fixtures/rules/coc7_san_combat_chase_flow.v1.json.md");
const DICE_SAN_COMBAT_CHASE_TEST_DATA: &str =
    include_str!("../../../test-data/dice_san_combat_chase_cases.md");

#[test]
fn s05_fixture_files_are_bound_to_current_acceptance_gate() {
    for token in [
        "\"stage\": \"S05\"",
        "docs/reports/stages/S05_ACCEPTANCE_EVIDENCE.md",
        "docs/reports/stages/S05_TEST_RESULTS.md",
        "docs/reports/stages/S05_TRACEABILITY.md",
    ] {
        assert!(
            S05_STAGE_FIXTURE.contains(token),
            "missing S05 stage token {token}"
        );
    }

    for token in [
        "\"expected_events\"",
        "\"DiceRolled\"",
        "\"SkillCheckResolved\"",
        "\"SanityLossApplied\"",
        "\"CombatStateUpdated\"",
        "\"ChaseSegmentResolved\"",
        "\"CLIENT_FORMAL_DICE_FORBIDDEN\"",
        "\"AI_DICE_FABRICATION_FORBIDDEN\"",
        "\"STATE_CHANGE_WITHOUT_EVENT\"",
        "\"server_dice_only\"",
        "\"combat_and_chase_state_evented\"",
    ] {
        assert!(
            S05_DETAILED_FIXTURE.contains(token),
            "missing detailed fixture token {token}"
        );
    }

    for token in [
        "\"expected_derived\"",
        "\"expected_level\"",
        "\"expected_loss\"",
        "\"expected_events\"",
        "\"expected_lead\"",
    ] {
        let fixture_text = [
            CHARACTER_FIXTURE,
            DICE_FIXTURE,
            SAN_COMBAT_CHASE_FIXTURE,
            DICE_SAN_COMBAT_CHASE_TEST_DATA,
        ]
        .join("\n");
        assert!(
            fixture_text.contains(token),
            "missing fixture token {token}"
        );
    }
}

#[test]
fn s05_character_and_dice_fixtures_map_to_ruleset_assertions() {
    assert!(CHARACTER_FIXTURE.contains("\"occupation\": \"Antiquarian\""));
    let stats = derive_character_stats(Coc7Characteristics {
        strength: 45,
        constitution: 55,
        size: 60,
        dexterity: 50,
        appearance: 40,
        intelligence: 70,
        power: 65,
        education: 75,
        luck: 55,
    })
    .unwrap();

    assert_eq!(stats.hit_points, 11);
    assert_eq!(stats.magic_points, 13);
    assert_eq!(stats.sanity, 65);
    assert_eq!(stats.luck, 55);
    assert_eq!(stats.damage_bonus, DamageBonus::None);
    assert_eq!(stats.build, 0);
    assert!(CHARACTER_FIXTURE.contains("\"Move\": 7"));
    assert_eq!(stats.movement_rate, 7);

    assert_eq!(success_level(37, 60).unwrap(), SuccessLevel::Regular);
    assert_eq!(success_level(30, 60).unwrap(), SuccessLevel::Hard);
    assert_eq!(success_level(12, 60).unwrap(), SuccessLevel::Extreme);
    assert_eq!(success_level(100, 60).unwrap(), SuccessLevel::Fumble);

    assert_eq!(
        adjusted_percentile_roll(8, 7, &[3], DiceAdjustment::Bonus).unwrap(),
        (3, 37)
    );
    assert_eq!(
        adjusted_percentile_roll(3, 7, &[8], DiceAdjustment::Penalty).unwrap(),
        (8, 87)
    );

    let contract = common::human_contract();
    let mut store = common::event_store();
    let outcome = trpg_ruleset_coc7::dice_roll_contract::adjudicate_skill_check(
        60,
        3,
        7,
        &[],
        DiceAdjustment::None,
    )
    .unwrap();
    let command = common::rules_command("dice");
    let event = record_dice_roll_contract(&contract, &mut store, &command, &outcome).unwrap();

    assert_eq!(event.event_type, "DiceRolled");
    assert_eq!(event.payload.schema_version, 1);
    assert_eq!(event.payload.visibility_label, "system_only");
    assert_eq!(
        event.payload.provenance_kind,
        ProvenanceKind::RulesEngineDecision
    );

    let mut denied = common::rules_command("dice");
    denied.write_path = FormalWritePath::DirectAgent;
    assert_eq!(
        record_dice_roll_contract(&contract, &mut store, &denied, &outcome).unwrap_err(),
        TrpgError::DirectAgentStateWrite
    );
}

#[test]
fn s05_san_combat_chase_and_clue_fixtures_map_to_evented_assertions() {
    let san_success = resolve_san_check(41, 55, 0, 2, 0).unwrap();
    assert_eq!(san_success.loss, 0);

    let san_failure = resolve_san_check(81, 55, 1, 3, 0).unwrap();
    assert_eq!(san_failure.loss, 3);

    let contract = common::human_contract();
    let mut store = common::event_store();
    let san_command = common::rules_command("san");
    let san_event = record_san_decision(&contract, &mut store, &san_command, &san_failure).unwrap();
    assert_eq!(san_event.event_type, "SanityLossApplied");

    let combat = apply_damage(12, 12, 5).unwrap();
    assert_eq!(combat.damage, 5);
    assert_eq!(combat.after_hp, 7);
    let mut combat_command = common::rules_command("combat");
    combat_command.expected_version = 1;
    combat_command.idempotency_key = "idem_combat".to_owned();
    let combat_event =
        record_combat_transition(&contract, &mut store, &combat_command, &combat).unwrap();
    assert_eq!(combat_event.event_type, "CombatStateUpdated");

    let lead_increases = advance_chase(2, true, false, 0).unwrap();
    assert_eq!(lead_increases.after_range, 3);
    let caught = advance_chase(1, false, true, 0).unwrap();
    assert_eq!(caught.after_range, 0);
    assert_eq!(caught.status, ChaseStatus::Caught);
    let mut chase_command = common::rules_command("chase");
    chase_command.expected_version = 2;
    chase_command.idempotency_key = "idem_chase".to_owned();
    let chase_event =
        record_chase_transition(&contract, &mut store, &chase_command, &caught).unwrap();
    assert_eq!(chase_event.event_type, "ChaseSegmentResolved");

    let clue = resolve_clue_check(ClueImportance::Core, false);
    assert_eq!(clue.outcome, ClueOutcome::RevealedWithCost);
    assert_eq!(clue.cost, Some("time_or_complication"));
    let mut clue_command = common::rules_command("clue");
    clue_command.expected_version = 3;
    clue_command.idempotency_key = "idem_clue".to_owned();
    let clue_event =
        record_investigation_clue_npc_time_decision(&contract, &mut store, &clue_command, &clue)
            .unwrap();
    assert_eq!(clue_event.event_type, "ClueRevealed");

    assert_eq!(store.events().len(), 4);
}

#[test]
fn s05_visibility_provenance_and_private_leakage_assertions_are_event_bound() {
    let secret = NpcSecret::keeper_only("npc_001", "Dr. Allen", "cult leader");
    assert_eq!(
        npc_secret_for_principal(&secret, &PrincipalScope::Public),
        None
    );
    assert_eq!(
        npc_secret_for_principal(&secret, &PrincipalScope::Keeper),
        Some("cult leader")
    );

    let contract = common::human_contract();
    let mut store = common::event_store();
    let transition = resolve_san_check(81, 55, 1, 3, 0).unwrap();
    let command = common::rules_command("san");
    let event = record_san_decision(&contract, &mut store, &command, &transition).unwrap();

    assert_eq!(event.payload.ruleset_id, "coc7");
    assert_eq!(event.payload.visibility_label, "system_only");
    assert_eq!(
        event.payload.provenance_kind,
        ProvenanceKind::RulesEngineDecision
    );
    assert!(store.replay_visible(&PrincipalScope::Public).is_empty());
    assert_eq!(store.replay_visible(&PrincipalScope::System).len(), 1);
}
