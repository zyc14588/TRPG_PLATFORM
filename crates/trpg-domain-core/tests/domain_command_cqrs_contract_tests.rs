use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::command_cqrs::{CommandAcceptedPayload, DomainCommandKind};
use trpg_domain_core::ddd::{ActorRole, AuthorityMode, DomainError, EventStore, FactSource};
use trpg_domain_core::domain_command_cqrs::{decide_and_append, DomainCommandDecision};

#[test]
fn domain_command_cqrs_preserves_idempotency_and_expected_version() {
    let contract = DomainAuthorityContract::new_locked(
        "campaign_001",
        AuthorityMode::AiKp,
        "ai_keeper_001",
        1,
    )
    .unwrap();
    let command =
        trpg_test_support::governed_command!("rule", ActorRole::Workflow, AuthorityMode::AiKp);
    let decision = DomainCommandDecision::command_accepted(
        DomainCommandKind::RecordDecision,
        FactSource::GameEvent,
    );
    let mut store: EventStore<CommandAcceptedPayload> = EventStore::default();

    decide_and_append(&contract, &mut store, &command, decision.clone()).unwrap();
    let duplicate = decide_and_append(&contract, &mut store, &command, decision).unwrap_err();

    assert_eq!(
        duplicate,
        DomainError::ExpectedVersionConflict {
            expected: 0,
            actual: 1
        }
    );
}

#[test]
fn domain_command_cqrs_blocks_ai_keeper_actor_from_formal_ai_kp_write() {
    let contract = DomainAuthorityContract::new_locked(
        "campaign_001",
        AuthorityMode::AiKp,
        "ai_keeper_001",
        1,
    )
    .unwrap();
    let command =
        trpg_test_support::governed_command!("ruling", ActorRole::AiKeeper, AuthorityMode::AiKp);
    let decision = DomainCommandDecision::command_accepted(
        DomainCommandKind::RecordDecision,
        FactSource::GameEvent,
    );
    let mut store: EventStore<CommandAcceptedPayload> = EventStore::default();

    assert_eq!(
        decide_and_append(&contract, &mut store, &command, decision).unwrap_err(),
        DomainError::AuthorityViolation
    );
}
