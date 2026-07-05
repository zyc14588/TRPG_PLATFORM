mod common;

use trpg_ruleset_coc7::rules_coc7::{
    assert_rules_coc7_request, record_rules_coc7_dispatch, rules_coc7_metadata,
};
use trpg_shared_kernel::TrpgError;

#[test]
fn rules_metadata_requires_server_dice_and_no_direct_llm() {
    let metadata = rules_coc7_metadata();

    assert_eq!(metadata.ruleset_id, "coc7");
    assert!(metadata.server_dice_required);
    assert!(!metadata.direct_llm_allowed);
    assert_eq!(
        assert_rules_coc7_request("legacy_coc").unwrap_err(),
        TrpgError::InvalidConfiguration("coc7_ruleset_id")
    );
}

#[test]
fn rules_dispatch_records_governed_event() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("rules");

    let event = record_rules_coc7_dispatch(&contract, &mut store, &command, "skill_check").unwrap();

    assert_eq!(event.payload.decision_type, "rules_coc7");
}
