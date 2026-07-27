use trpg_ruleset_coc7::dice_roll_contract::{adjudicate_skill_growth, server_roll_skill_growth};

#[test]
fn coc7_growth_applies_only_after_a_qualifying_improvement_check() {
    let improved = adjudicate_skill_growth(70, 83, 7).unwrap();
    assert_eq!(improved.increase_roll, Some(7));
    assert_eq!(improved.skill_after, 77);

    let unchanged = adjudicate_skill_growth(70, 42, 10).unwrap();
    assert_eq!(unchanged.increase_roll, None);
    assert_eq!(unchanged.skill_after, 70);

    let capped = adjudicate_skill_growth(98, 100, 10).unwrap();
    assert_eq!(capped.skill_after, 99);
}

#[test]
fn production_growth_roll_is_server_generated_and_range_checked() {
    let roll = server_roll_skill_growth(55).unwrap();
    assert!(roll.roll_id().starts_with("server_percentile_"));
    assert!((1..=100).contains(&roll.outcome().improvement_check_roll));
    assert_eq!(
        roll.evidence().improvement_check().value(),
        roll.outcome().improvement_check_roll
    );
    let qualifies =
        roll.outcome().improvement_check_roll > 55 || roll.outcome().improvement_check_roll >= 96;
    assert_eq!(roll.evidence().increase().is_some(), qualifies);
    if let Some(increase) = roll.evidence().increase() {
        assert!(increase.roll_id().starts_with("server_d10_"));
        assert_ne!(increase.roll_id(), roll.roll_id());
        assert_eq!(Some(increase.value()), roll.outcome().increase_roll);
    }
    assert!(roll
        .outcome()
        .increase_roll
        .is_none_or(|value| (1..=10).contains(&value)));
    assert!((55..=65).contains(&roll.outcome().skill_after));
    assert!(adjudicate_skill_growth(100, 100, 10).is_err());
}
