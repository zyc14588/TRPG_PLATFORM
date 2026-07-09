use std::fmt::Debug;

use trpg_ops::{
    readme::replay_visible_ops_events, Actor, ActorRole, AuthorityContract, AuthorityMode,
    CommandEnvelope, EntityId, FactProvenance, FormalWritePath, KernelResult, OpsEvent,
    OpsEventEnvelope, OpsEventStore, OpsRunbookContract, PrincipalScope, ProvenanceKind, TrpgError,
    Visibility, VisibilityLabel, VISIBILITY_REDACTED,
};

pub fn assert_runbook_contract<T, F>(contract: OpsRunbookContract, payload: T, append: F)
where
    T: Clone + Debug,
    F: Fn(
        &mut OpsEventStore,
        &AuthorityContract,
        &CommandEnvelope<T>,
    ) -> KernelResult<OpsEventEnvelope>,
{
    assert!(contract.uses_current_safe_names());
    assert!(contract
        .required_command_fields
        .contains(&"idempotency_key"));
    assert!(contract
        .required_command_fields
        .contains(&"expected_version"));
    assert_eq!(
        contract.canon_boundary,
        "command_workflow_decision_event_store_projection"
    );

    let authority = authority_contract();
    let mut store = OpsEventStore::default();
    let command = governed_command(
        payload.clone(),
        0,
        "idem_success",
        Visibility::new(VisibilityLabel::SystemOnly),
    );

    let event = append(&mut store, &authority, &command).expect("runbook event is appended");
    assert_eq!(event.event_type, contract.event_type);
    assert_eq!(store.events().len(), 1);
    match &event.payload {
        OpsEvent::RunbookStepRecorded(record) => {
            assert_eq!(record.module_name, contract.module_name);
            assert_eq!(record.operation, contract.operation);
            assert!(
                record
                    .evidence_path
                    .starts_with("evidence/batches/BATCH-042/")
                    || record
                        .evidence_path
                        .starts_with("docs/codex/11-ops-migration/")
            );
        }
        other => panic!("unexpected event payload: {other:?}"),
    }

    let conflict = append(
        &mut store,
        &authority,
        &governed_command(
            payload.clone(),
            0,
            "idem_conflict",
            Visibility::new(VisibilityLabel::SystemOnly),
        ),
    )
    .expect_err("expected_version must match event stream");
    assert!(matches!(
        conflict,
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1
        }
    ));

    let duplicate = append(
        &mut store,
        &authority,
        &governed_command(
            payload.clone(),
            1,
            "idem_success",
            Visibility::new(VisibilityLabel::SystemOnly),
        ),
    )
    .expect_err("idempotency key replay must be rejected");
    assert_eq!(duplicate, TrpgError::DuplicateCommand);

    let mut wrong_authority = governed_command(
        payload.clone(),
        1,
        "idem_wrong_authority",
        Visibility::new(VisibilityLabel::SystemOnly),
    );
    wrong_authority.actor = Actor::new("human_keeper", ActorRole::HumanKeeper).unwrap();
    let authority_error = append(&mut store, &authority, &wrong_authority)
        .expect_err("AI_KP campaign cannot accept human keeper formal write");
    assert_eq!(authority_error, TrpgError::AuthorityViolation);

    let mut direct_agent = governed_command(
        payload,
        1,
        "idem_direct_agent",
        Visibility::new(VisibilityLabel::SystemOnly),
    );
    direct_agent.write_path = FormalWritePath::DirectAgent;
    let direct_error = append(&mut store, &authority, &direct_agent)
        .expect_err("direct agent state write must be blocked");
    assert_eq!(direct_error, TrpgError::DirectAgentStateWrite);

    assert_eq!(store.events().len(), 1);
}

#[allow(dead_code)]
pub fn assert_visibility_redaction<T, F>(payload: T, append: F)
where
    T: Clone + Debug,
    F: Fn(
        &mut OpsEventStore,
        &AuthorityContract,
        &CommandEnvelope<T>,
    ) -> KernelResult<OpsEventEnvelope>,
{
    let authority = authority_contract();
    let mut store = OpsEventStore::default();
    let player_a = EntityId::new("player_a").unwrap();

    append(
        &mut store,
        &authority,
        &governed_command(
            payload.clone(),
            0,
            "idem_private_player_a",
            Visibility::private_to_player(player_a.clone()),
        ),
    )
    .unwrap();
    append(
        &mut store,
        &authority,
        &governed_command(
            payload,
            1,
            "idem_ai_internal",
            Visibility::new(VisibilityLabel::AiInternal),
        ),
    )
    .unwrap();

    assert_eq!(
        replay_visible_ops_events(&store, &PrincipalScope::Player(player_a)).len(),
        1
    );
    assert!(replay_visible_ops_events(&store, &PrincipalScope::Public).is_empty());
    assert_eq!(
        replay_visible_ops_events(&store, &PrincipalScope::System).len(),
        2
    );

    assert_eq!(
        trpg_ops::redact_ops_output(
            &Visibility::new(VisibilityLabel::AiInternal),
            "keeper secret"
        ),
        VISIBILITY_REDACTED
    );
    assert_eq!(
        trpg_ops::redact_ops_output(&Visibility::new(VisibilityLabel::Public), "public note"),
        "public note"
    );
}

fn authority_contract() -> AuthorityContract {
    AuthorityContract::new("campaign_ops_001", AuthorityMode::AiKp, 1).unwrap()
}

fn governed_command<T>(
    payload: T,
    expected_version: u64,
    idempotency_key: &'static str,
    visibility: Visibility,
) -> CommandEnvelope<T> {
    let mut command = CommandEnvelope::governed(payload, ActorRole::Workflow, AuthorityMode::AiKp);
    command.command_id = EntityId::new(format!("command_{idempotency_key}")).unwrap();
    command.idempotency_key = idempotency_key.to_owned();
    command.expected_version = expected_version;
    command.visibility = visibility;
    command.fact_provenance =
        FactProvenance::new(ProvenanceKind::SystemFixture, "fact_ops", "fixture_s10").unwrap();
    command.correlation_id = EntityId::new(format!("corr_{idempotency_key}")).unwrap();
    command.causation_id = EntityId::new(format!("cause_{idempotency_key}")).unwrap();
    command
}
