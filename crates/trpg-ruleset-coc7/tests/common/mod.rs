use trpg_ruleset_coc7::Coc7EventPayload;
use trpg_shared_kernel::{
    ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EventStore,
};

pub fn human_contract() -> AuthorityContract {
    trpg_test_support::authority_contract("camp_b021", AuthorityMode::HumanKp, 1).unwrap()
}

pub fn rules_command<T>(payload: T) -> CommandEnvelope<T> {
    trpg_test_support::governed_command_for_contract(
        &human_contract(),
        payload,
        ActorRole::RulesEngine,
    )
}

pub fn event_store() -> EventStore<Coc7EventPayload> {
    EventStore::default()
}
