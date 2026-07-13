use trpg_ruleset_coc7::Coc7EventPayload;
use trpg_shared_kernel::{
    ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EventStore,
};

pub fn human_contract() -> AuthorityContract {
    AuthorityContract::new("camp_b021", AuthorityMode::HumanKp, 1).unwrap()
}

pub fn rules_command<T>(payload: T) -> CommandEnvelope<T> {
    trpg_test_support::governed_command!(payload, ActorRole::RulesEngine, AuthorityMode::HumanKp)
}

pub fn event_store() -> EventStore<Coc7EventPayload> {
    EventStore::default()
}
