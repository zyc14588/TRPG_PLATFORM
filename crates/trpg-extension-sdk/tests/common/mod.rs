use std::fmt::Debug;

use trpg_extension_sdk::{
    readme::replay_visible_extension_events, Actor, ActorRole, AuthorityContract, AuthorityMode,
    CommandEnvelope, EntityId, ExtensionContract, ExtensionEvent, ExtensionEventEnvelope,
    ExtensionEventStore, FactProvenance, FormalWritePath, KernelResult, PrincipalScope,
    ProvenanceKind, TrpgError, Visibility, VisibilityLabel, EXTENSION_CANON_BOUNDARY,
    EXTENSION_REDACTED,
};

pub fn assert_extension_contract<T, F>(contract: ExtensionContract, payload: T, append: F)
where
    T: Clone + Debug,
    F: Fn(
        &mut ExtensionEventStore,
        &AuthorityContract,
        &CommandEnvelope<T>,
    ) -> KernelResult<ExtensionEventEnvelope>,
{
    assert!(contract.uses_current_safe_names());
    assert_eq!(contract.canon_boundary, EXTENSION_CANON_BOUNDARY);
    assert!(contract
        .required_command_fields
        .contains(&"idempotency_key"));
    assert!(contract
        .required_command_fields
        .contains(&"expected_version"));
    assert!(contract
        .forbidden_capabilities
        .iter()
        .any(|capability| capability.denial_code() == "EXTENSION_DIRECT_LLM_FORBIDDEN"));

    let authority = authority_contract();
    let mut store = ExtensionEventStore::default();
    let command = governed_command(
        payload.clone(),
        0,
        "idem_success",
        Visibility::new(VisibilityLabel::SystemOnly),
    );

    let event = append(&mut store, &authority, &command).expect("extension event is appended");
    assert_eq!(event.event_type, contract.event_type);
    assert_eq!(store.events().len(), 1);
    match &event.payload {
        ExtensionEvent::ContractRecorded(record) => {
            assert_eq!(record.module_name, contract.module_name);
            assert_eq!(record.operation, contract.operation);
            assert!(record
                .evidence_path
                .starts_with("evidence/batches/BATCH-044/"));
            assert!(record
                .capabilities
                .iter()
                .all(|capability| !capability.is_forbidden()));
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
    .expect_err("expected_version must match stream length");
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
        .expect_err("AI_KP command path cannot accept human KP formal writes");
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
        &mut ExtensionEventStore,
        &AuthorityContract,
        &CommandEnvelope<T>,
    ) -> KernelResult<ExtensionEventEnvelope>,
{
    let authority = authority_contract();
    let mut store = ExtensionEventStore::default();
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
            payload.clone(),
            1,
            "idem_ai_internal",
            Visibility::new(VisibilityLabel::AiInternal),
        ),
    )
    .unwrap();
    append(
        &mut store,
        &authority,
        &governed_command(
            payload,
            2,
            "idem_keeper_only",
            Visibility::new(VisibilityLabel::KeeperOnly),
        ),
    )
    .unwrap();

    assert_eq!(
        replay_visible_extension_events(&store, &PrincipalScope::Player(player_a)).len(),
        1
    );
    assert!(replay_visible_extension_events(&store, &PrincipalScope::Public).is_empty());
    assert_eq!(
        replay_visible_extension_events(&store, &PrincipalScope::System).len(),
        3
    );
    assert_eq!(
        replay_visible_extension_events(&store, &PrincipalScope::Keeper).len(),
        2
    );

    assert_eq!(
        trpg_extension_sdk::redact_extension_output(
            &Visibility::new(VisibilityLabel::AiInternal),
            "internal tool trace"
        ),
        EXTENSION_REDACTED
    );
    assert_eq!(
        trpg_extension_sdk::redact_extension_output(
            &Visibility::new(VisibilityLabel::KeeperOnly),
            "keeper secret"
        ),
        EXTENSION_REDACTED
    );
    assert_eq!(
        trpg_extension_sdk::redact_extension_output(
            &Visibility::new(VisibilityLabel::Public),
            "public note"
        ),
        "public note"
    );
}

pub fn authority_contract() -> AuthorityContract {
    AuthorityContract::new("campaign_extension_001", AuthorityMode::AiKp, 1).unwrap()
}

pub fn governed_command<T>(
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
        FactProvenance::new(ProvenanceKind::SystemFixture, "fact_s12", "fixture_s12").unwrap();
    command.correlation_id = EntityId::new(format!("corr_{idempotency_key}")).unwrap();
    command.causation_id = EntityId::new(format!("cause_{idempotency_key}")).unwrap();
    command
}
