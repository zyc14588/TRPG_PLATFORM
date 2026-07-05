mod common;

use trpg_ruleset_coc7::npc::{npc_secret_for_principal, record_npc_decision, NpcSecret};
use trpg_shared_kernel::PrincipalScope;

#[test]
fn npc_secret_is_hidden_from_public_and_visible_to_keeper() {
    let secret = NpcSecret::keeper_only("npc_001", "Dr. Allen", "cult leader");

    assert_eq!(
        npc_secret_for_principal(&secret, &PrincipalScope::Public),
        None
    );
    assert_eq!(
        npc_secret_for_principal(&secret, &PrincipalScope::Keeper),
        Some("cult leader")
    );
}

#[test]
fn npc_decision_records_visibility_label() {
    let contract = common::human_contract();
    let mut store = common::event_store();
    let command = common::rules_command("npc");
    let secret = NpcSecret::keeper_only("npc_001", "Dr. Allen", "cult leader");

    let event = record_npc_decision(&contract, &mut store, &command, &secret).unwrap();

    assert_eq!(event.payload.summary, "npc=npc_001 visibility=keeper_only");
}
