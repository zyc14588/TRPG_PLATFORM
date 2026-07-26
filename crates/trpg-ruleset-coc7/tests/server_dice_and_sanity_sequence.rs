use trpg_ruleset_coc7::dice_roll_contract::{
    server_roll_skill_check, DiceAdjustment, SuccessLevel,
};
use trpg_ruleset_coc7::investigation_clue_npc_time::{
    resolve_clue_check, ClueImportance, ClueOutcome,
};
use trpg_ruleset_coc7::sanity_madness_state_machine::{apply_sanity_loss, MadnessState};

#[test]
fn server_rng_produces_an_opaque_valid_coc7_percentile_result() {
    for adjustment in [
        DiceAdjustment::None,
        DiceAdjustment::Bonus,
        DiceAdjustment::Penalty,
    ] {
        let generated = server_roll_skill_check(55, adjustment)
            .expect("the rules service should have operating-system entropy");
        let outcome = generated.outcome();
        assert!((1..=100).contains(&outcome.roll));
        assert_eq!(outcome.target, 55);
        assert_eq!(outcome.adjustment, adjustment);
        assert!(matches!(
            outcome.success_level,
            SuccessLevel::Critical
                | SuccessLevel::Extreme
                | SuccessLevel::Hard
                | SuccessLevel::Regular
                | SuccessLevel::Failure
                | SuccessLevel::Fumble
        ));
        assert!(generated.roll_id().starts_with("dice_"));
    }
}

#[test]
fn a_core_clue_is_revealed_with_a_cost_when_the_check_fails() {
    let resolution = resolve_clue_check(ClueImportance::Core, false);
    assert_eq!(resolution.outcome, ClueOutcome::RevealedWithCost);
    assert_eq!(resolution.cost, Some("time_or_complication"));
}

#[test]
fn indefinite_insanity_uses_the_fixed_day_start_baseline() {
    let first = apply_sanity_loss(60, 5, 0, 60).expect("first SAN loss");
    assert_eq!(first.indefinite_threshold, 12);
    assert_eq!(first.state, MadnessState::TemporaryInsanity);

    let second = apply_sanity_loss(first.after, 6, first.day_loss, 60)
        .expect("second SAN loss against the same day baseline");
    assert_eq!(second.day_loss, 11);
    assert_ne!(second.state, MadnessState::IndefiniteInsanity);

    let third = apply_sanity_loss(second.after, 1, second.day_loss, 60)
        .expect("third SAN loss crosses one fifth of day-start SAN");
    assert_eq!(third.day_loss, 12);
    assert_eq!(third.state, MadnessState::IndefiniteInsanity);
}

#[test]
fn sanity_grouping_does_not_change_the_indefinite_threshold() {
    let grouped = apply_sanity_loss(60, 12, 0, 60).expect("grouped loss");
    let split_first = apply_sanity_loss(60, 7, 0, 60).expect("split loss one");
    let split_second =
        apply_sanity_loss(split_first.after, 5, split_first.day_loss, 60).expect("split loss two");

    assert_eq!(grouped.after, split_second.after);
    assert_eq!(grouped.day_loss, split_second.day_loss);
    assert_eq!(
        grouped.indefinite_threshold,
        split_second.indefinite_threshold
    );
    assert_eq!(grouped.state, split_second.state);
    assert_eq!(grouped.state, MadnessState::IndefiniteInsanity);
}
