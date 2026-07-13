use trpg_shared_kernel::shared_kernel::{
    kernel_contract_snapshot, validate_command_envelope, ActorRole, AuthorityContract,
    AuthorityMode, CommandEnvelope, EntityId, EventStore, FormalWritePath, PrincipalScope,
    ProvenanceKind, TrpgError, Visibility, VisibilityLabel,
};

#[test]
fn shared_kernel_enforces_typed_ids_and_visibility_fixture_contract() {
    let fixture = include_str!(
        "../../../fixtures/stages/detailed/S01_foundation_shared_kernel.current.json.md"
    );
    assert!(fixture.contains("\"stage\": \"S01\""));
    assert!(fixture.contains("\"UNKNOWN_VISIBILITY_LABEL\""));
    assert!(fixture.contains("\"INVALID_ENTITY_ID\""));

    assert_eq!(EntityId::new("").unwrap_err(), TrpgError::InvalidEntityId);
    assert_eq!(
        VisibilityLabel::try_from("unknown").unwrap_err(),
        TrpgError::UnknownVisibilityLabel
    );

    let snapshot = kernel_contract_snapshot();
    assert_eq!(snapshot.id_format, "non_empty_ascii_alnum_underscore_dash");
    assert_eq!(
        VisibilityLabel::try_from("party_visible").unwrap(),
        VisibilityLabel::PartyVisible
    );
    assert!(snapshot.visibility_enum.contains(&"system_only"));
    assert!(snapshot.visibility_enum.contains(&"party_visible"));
    assert!(snapshot.visibility_enum.contains(&"ai_internal"));
    assert!(snapshot.error_codes.contains(&"INVALID_ENTITY_ID"));
}

#[test]
fn test_support_governed_command_preserves_the_legacy_fixture_metadata() {
    let command: CommandEnvelope<&str> =
        trpg_test_support::governed_command!("payload", ActorRole::Workflow, AuthorityMode::AiKp);

    assert_eq!(command.command_id.as_str(), "command_001");
    assert_eq!(command.idempotency_key, "idem_001");
    assert_eq!(command.expected_version, 0);
    assert_eq!(command.actor.id.as_str(), "actor_001");
    assert_eq!(command.actor.role, ActorRole::Workflow);
    assert_eq!(command.authority_mode, AuthorityMode::AiKp);
    assert_eq!(command.authority_contract_version, 1);
    assert_eq!(command.visibility.label(), &VisibilityLabel::SystemOnly);
    assert_eq!(
        command.fact_provenance.kind,
        ProvenanceKind::RulesEngineDecision
    );
    assert_eq!(command.fact_provenance.reference.as_str(), "fact_001");
    assert_eq!(command.fact_provenance.recorded_by.as_str(), "rules_001");
    assert_eq!(command.correlation_id.as_str(), "corr_001");
    assert_eq!(command.causation_id.as_str(), "cause_001");
    assert_eq!(command.write_path, FormalWritePath::WorkflowDecision);
    assert_eq!(command.payload, "payload");
}

#[test]
fn shared_kernel_blocks_direct_agent_state_writes() {
    let mut command =
        trpg_test_support::governed_command!("payload", ActorRole::AiKeeper, AuthorityMode::AiKp);
    command.write_path = FormalWritePath::DirectAgent;

    assert_eq!(
        validate_command_envelope(&command).unwrap_err(),
        TrpgError::DirectAgentStateWrite
    );
}

#[test]
fn shared_kernel_keeps_authority_contract_immutable() {
    let contract = AuthorityContract::new("campaign_001", AuthorityMode::HumanKp, 1).unwrap();
    let command = trpg_test_support::governed_command!(
        "payload",
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp
    );

    contract.validate_command(&command).unwrap();

    let forked = contract.fork(AuthorityMode::AiKp, 2).unwrap();
    assert_eq!(forked.version(), 2);
    assert_eq!(forked.mode(), &AuthorityMode::AiKp);
    assert_eq!(forked.campaign_id().as_str(), "campaign_001");
    assert_eq!(
        contract.fork(AuthorityMode::AiKp, 1).unwrap_err(),
        TrpgError::AuthorityContractMutation
    );
}

#[test]
fn shared_kernel_replay_redacts_visibility_restricted_events() {
    let player = EntityId::new("character_001").unwrap();
    let mut command = trpg_test_support::governed_command!(
        "secret",
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp
    );
    command.visibility = Visibility::private_to_player(player.clone());

    let mut store = EventStore::default();
    store
        .append(&command, "SharedKernelTypesValidated", "secret")
        .unwrap();

    assert_eq!(
        store.replay_visible(&PrincipalScope::Player(player)).len(),
        1
    );
    assert!(store
        .replay_visible(&PrincipalScope::Player(
            EntityId::new("character_002").unwrap()
        ))
        .is_empty());
    assert!(store.replay_visible(&PrincipalScope::Public).is_empty());
}

#[test]
fn shared_kernel_replay_never_exposes_ai_internal_to_players() {
    let mut command =
        trpg_test_support::governed_command!("internal", ActorRole::System, AuthorityMode::AiKp);
    command.visibility = Visibility::new(VisibilityLabel::AiInternal);

    let mut store = EventStore::default();
    store
        .append(&command, "SharedKernelTypesValidated", "internal")
        .unwrap();

    assert_eq!(store.replay_visible(&PrincipalScope::System).len(), 1);
    assert!(store.replay_visible(&PrincipalScope::Public).is_empty());
    assert!(store
        .replay_visible(&PrincipalScope::PartyMember)
        .is_empty());
    assert!(store.replay_visible(&PrincipalScope::Keeper).is_empty());
    assert!(store
        .replay_visible(&PrincipalScope::Player(
            EntityId::new("character_001").unwrap()
        ))
        .is_empty());
}
