mod common;

use trpg_ruleset_coc7::dice_roll_contract::{
    adjudicate_skill_check, record_dice_roll_contract, server_roll_skill_check, DiceAdjustment,
    SuccessLevel,
};
use trpg_shared_kernel::{FormalWritePath, TrpgError};

#[test]
fn adjudication_selects_bonus_die_from_supplied_server_digits() {
    let outcome = adjudicate_skill_check(60, 7, 4, &[2], DiceAdjustment::Bonus).unwrap();

    assert_eq!(outcome.roll, 24);
    assert_eq!(outcome.success_level, SuccessLevel::Hard);
}

#[test]
fn recordable_dice_roll_is_generated_by_the_server_rng() {
    let server_roll = server_roll_skill_check(60, DiceAdjustment::Bonus).unwrap();

    assert!(server_roll.roll_id().starts_with("dice_"));
    assert_eq!(server_roll.roll_id().len(), 37);
    assert!((1..=100).contains(&server_roll.outcome().roll));
    assert!(server_roll.outcome().selected_tens_digit <= 9);
    assert!(server_roll.outcome().ones_digit <= 9);
}

#[test]
fn dice_roll_event_rejects_direct_agent_write() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let server_roll = server_roll_skill_check(60, DiceAdjustment::None).unwrap();
    let mut command = common::rules_command("dice");
    command.write_path = FormalWritePath::DirectAgent;

    let error =
        record_dice_roll_contract(&contract, &mut store, &command, &server_roll).unwrap_err();

    assert_eq!(error, TrpgError::DirectAgentStateWrite);
}
