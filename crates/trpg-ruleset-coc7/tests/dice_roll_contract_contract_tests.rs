mod common;

use trpg_ruleset_coc7::dice_roll_contract::{
    adjudicate_skill_check, record_dice_roll_contract, DiceAdjustment, SuccessLevel,
};
use trpg_shared_kernel::{FormalWritePath, TrpgError};

#[test]
fn dice_roll_uses_server_digits_and_bonus_die() {
    let outcome = adjudicate_skill_check(60, 7, 4, &[2], DiceAdjustment::Bonus).unwrap();

    assert_eq!(outcome.roll, 24);
    assert_eq!(outcome.success_level, SuccessLevel::Hard);
}

#[test]
fn dice_roll_event_rejects_direct_agent_write() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let outcome = adjudicate_skill_check(60, 3, 2, &[], DiceAdjustment::None).unwrap();
    let mut command = common::rules_command("dice");
    command.write_path = FormalWritePath::DirectAgent;

    let error = record_dice_roll_contract(&contract, &mut store, &command, &outcome).unwrap_err();

    assert_eq!(error, TrpgError::DirectAgentStateWrite);
}
