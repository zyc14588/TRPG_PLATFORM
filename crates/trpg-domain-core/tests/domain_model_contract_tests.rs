use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::command_cqrs::{CommandAcceptedPayload, DomainCommandKind};
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, DomainError, EventStore, FactSource, FormalWritePath,
};
use trpg_domain_core::domain_model::{
    reject_direct_state_write, DomainModelCommand, DomainModelService,
};

#[test]
fn domain_model_accepts_only_governed_command_path() {
    let contract = DomainAuthorityContract::new_locked(
        "campaign_001",
        AuthorityMode::HumanKp,
        "keeper_001",
        1,
    )
    .unwrap();
    let command = trpg_test_support::governed_command!(
        "look under desk",
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let model_command = DomainModelCommand {
        kind: DomainCommandKind::SubmitPlayerAction,
        fact_source: FactSource::DecisionRecord,
        envelope: command,
    };
    let mut store: EventStore<CommandAcceptedPayload> = EventStore::default();

    let event = DomainModelService::accept(&contract, &mut store, &model_command).unwrap();

    assert_eq!(event.sequence, 1);
    assert_eq!(event.payload.kind, DomainCommandKind::SubmitPlayerAction);
    assert_eq!(store.events().len(), 1);
}

#[test]
fn domain_model_rejects_direct_agent_state_write() {
    let mut command =
        trpg_test_support::governed_command!("draft", ActorRole::Workflow, AuthorityMode::AiKp);
    command.write_path = FormalWritePath::DirectAgent;

    assert_eq!(
        reject_direct_state_write(&command).unwrap_err(),
        DomainError::PolicyDenied
    );
}
