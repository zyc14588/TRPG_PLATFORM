mod common;

use trpg_ruleset_coc7::rule_runtime_coc7::{
    record_rule_runtime_coc7_decision, rule_runtime_coc7_subject, RuleRuntimeCoc7Call,
};

#[test]
fn runtime_subject_uses_current_safe_coc7_subjects() {
    assert_eq!(
        rule_runtime_coc7_subject(RuleRuntimeCoc7Call::ChaseRound),
        "ruleset.coc7.chase_round.decide"
    );
}

#[test]
fn runtime_decision_records_subject() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("runtime");

    let event = record_rule_runtime_coc7_decision(
        &contract,
        &mut store,
        &command,
        RuleRuntimeCoc7Call::SkillCheck,
    )
    .unwrap();

    assert!(event
        .payload
        .summary
        .contains("ruleset.coc7.skill_check.decide"));
}
