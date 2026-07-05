mod common;

use trpg_ruleset_coc7::coc7::{
    coc7_governance_profile, record_coc7_governance_profile, validate_coc7_governance_profile,
    Coc7GovernanceProfile,
};

#[test]
fn coc7_profile_requires_server_dice_event_log_and_no_direct_llm() {
    let profile = coc7_governance_profile();

    validate_coc7_governance_profile(&profile).unwrap();
    assert_eq!(profile.ruleset_id, "coc7");
    assert!(profile.server_dice_required);
    assert!(profile.event_log_required);
    assert!(!profile.direct_llm_allowed);
}

#[test]
fn coc7_profile_rejects_missing_core_modules() {
    let mut profile = coc7_governance_profile();
    profile.modules = vec!["san"];

    assert!(validate_coc7_governance_profile(&profile).is_err());
}

#[test]
fn coc7_profile_is_recorded_through_event_store() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command(Coc7GovernanceProfile {
        ruleset_id: "coc7",
        modules: vec![],
        server_dice_required: true,
        event_log_required: true,
        direct_llm_allowed: false,
    });
    let profile = coc7_governance_profile();

    let event = record_coc7_governance_profile(&contract, &mut store, &command, &profile).unwrap();

    assert_eq!(event.event_type, "coc7.governance_profile_recorded");
    assert_eq!(event.payload.decision_type, "coc7");
    assert_eq!(store.events().len(), 1);
}
