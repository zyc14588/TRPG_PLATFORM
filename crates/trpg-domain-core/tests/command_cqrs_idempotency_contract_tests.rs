use trpg_domain_core::command_cqrs_idempotency::append_idempotent_event;
use trpg_domain_core::ddd::{ActorRole, AuthorityMode, DomainError, EventStore};

#[test]
fn command_cqrs_idempotency_rejects_duplicate_key_and_bad_version() {
    let mut store = EventStore::default();
    let command =
        trpg_test_support::governed_command("payload", ActorRole::Workflow, AuthorityMode::AiKp);

    append_idempotent_event(&mut store, &command, "CommandAccepted", "payload").unwrap();

    let mut duplicate = command.clone();
    duplicate.expected_version = 1;
    assert_eq!(
        append_idempotent_event(&mut store, &duplicate, "CommandAccepted", "payload").unwrap_err(),
        DomainError::DuplicateCommand
    );

    let mut stale = command;
    stale.idempotency_key = "idem_002".to_owned();
    stale.expected_version = 99;
    assert_eq!(
        append_idempotent_event(&mut store, &stale, "CommandAccepted", "payload").unwrap_err(),
        DomainError::ExpectedVersionConflict {
            expected: 99,
            actual: 1
        }
    );
}
