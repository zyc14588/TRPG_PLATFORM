use trpg_shared_kernel::shared_kernel::{
    kernel_contract_snapshot, validate_command_envelope, ActorRole, AuthorityMode, EntityId,
    EventStore, FormalWritePath, PrincipalCapability, PrincipalClaims, PrincipalScope, TrpgError,
    Visibility, VisibilityLabel,
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
    assert!(snapshot.visibility_enum.contains(&"private_to_group"));
    assert!(snapshot.visibility_enum.contains(&"ai_internal"));
    assert!(snapshot.visibility_enum.contains(&"spectator_visible"));
    assert!(snapshot.visibility_enum.contains(&"spectator_hidden"));
    assert!(snapshot.error_codes.contains(&"INVALID_ENTITY_ID"));
}

#[test]
fn shared_kernel_enforces_the_authoritative_audience_matrix() {
    let player = EntityId::new("player_a").unwrap();
    let group_a = EntityId::new("group_a").unwrap();
    let group_b = EntityId::new("group_b").unwrap();

    let party = Visibility::new(VisibilityLabel::PartyVisible);
    assert!(party.can_view(&PrincipalScope::Player(player.clone())));
    assert!(party.can_view(&PrincipalScope::GroupMember(group_a.clone())));
    assert!(!party.can_view(&PrincipalScope::Spectator));
    assert!(!party.can_view(&PrincipalScope::Public));

    let private_group = Visibility::private_to_group(group_a.clone());
    assert!(private_group.is_well_formed());
    assert_eq!(private_group.group_id(), Some(&group_a));
    assert!(private_group.can_view(&PrincipalScope::GroupMember(group_a)));
    assert!(!private_group.can_view(&PrincipalScope::GroupMember(group_b)));
    assert!(!private_group.can_view(&PrincipalScope::Player(player.clone())));
    assert!(private_group.can_view(&PrincipalScope::Keeper));

    let spectator_visible = Visibility::new(VisibilityLabel::SpectatorVisible);
    assert!(spectator_visible.can_view(&PrincipalScope::Spectator));
    assert!(spectator_visible.can_view(&PrincipalScope::Player(player.clone())));
    assert!(!spectator_visible.can_view(&PrincipalScope::Public));

    let spectator_hidden = Visibility::new(VisibilityLabel::SpectatorHidden);
    assert!(!spectator_hidden.can_view(&PrincipalScope::Spectator));
    assert!(spectator_hidden.can_view(&PrincipalScope::Player(player)));
    assert!(spectator_hidden.can_view(&PrincipalScope::Keeper));

    let private_player = Visibility::private_to_player(EntityId::new("player_a").unwrap());
    let private_group = Visibility::private_to_group(EntityId::new("group_a").unwrap());
    assert_eq!(
        private_player
            .label()
            .conservative_merge(private_group.label()),
        VisibilityLabel::KeeperOnly
    );
    assert_eq!(
        private_group
            .label()
            .conservative_merge(private_player.label()),
        VisibilityLabel::KeeperOnly
    );
}

#[test]
fn visibility_wire_values_reject_targetless_or_spuriously_targeted_labels() {
    assert_eq!(
        serde_json::from_str::<Visibility>(r#"{"label":"private_to_player"}"#)
            .unwrap_err()
            .to_string(),
        "VISIBILITY_DENIED"
    );
    assert_eq!(
        serde_json::from_str::<Visibility>(r#"{"label":"public","subject_id":"player_a"}"#)
            .unwrap_err()
            .to_string(),
        "VISIBILITY_DENIED"
    );

    let targeted = Visibility::private_to_player(EntityId::new("player_a").unwrap());
    assert_eq!(
        serde_json::to_value(&targeted).unwrap(),
        serde_json::json!({"label": "private_to_player", "subject_id": "player_a"})
    );
    assert_eq!(
        serde_json::from_value::<Visibility>(serde_json::to_value(targeted).unwrap()).unwrap(),
        Visibility::private_to_player(EntityId::new("player_a").unwrap())
    );
}

#[test]
fn authenticated_principal_claims_preserve_composite_audiences() {
    let claims = PrincipalClaims::new("user_a")
        .unwrap()
        .with_player("player_a")
        .unwrap()
        .with_group("group_a")
        .unwrap()
        .with_group("group_b")
        .unwrap()
        .with_character("investigator_a")
        .unwrap()
        .with_capability(PrincipalCapability::PartyMember)
        .with_capability(PrincipalCapability::Spectator);
    let principal = PrincipalScope::Claims(claims);

    assert!(Visibility::private_to_player(EntityId::new("player_a").unwrap()).can_view(&principal));
    assert!(Visibility::private_to_group(EntityId::new("group_b").unwrap()).can_view(&principal));
    assert!(
        !Visibility::private_to_player(EntityId::new("player_b").unwrap()).can_view(&principal)
    );
    assert!(Visibility::new(VisibilityLabel::SpectatorVisible).can_view(&principal));
    assert!(Visibility::new(VisibilityLabel::SpectatorHidden).can_view(&principal));
}

#[test]
fn user_identity_alone_does_not_impersonate_a_private_player_target() {
    let principal = PrincipalScope::Claims(PrincipalClaims::new("player_a").unwrap());

    assert!(
        !Visibility::private_to_player(EntityId::new("player_a").unwrap()).can_view(&principal)
    );
}

#[test]
fn shared_kernel_blocks_direct_agent_state_writes() {
    let mut command =
        trpg_test_support::governed_command("payload", ActorRole::AiKeeper, AuthorityMode::AiKp);
    command.write_path = FormalWritePath::DirectAgent;

    assert_eq!(
        validate_command_envelope(&command).unwrap_err(),
        TrpgError::DirectAgentStateWrite
    );
}

#[test]
fn shared_kernel_keeps_authority_contract_immutable() {
    let contract =
        trpg_test_support::authority_contract("campaign_001", AuthorityMode::HumanKp, 1).unwrap();
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        "payload",
        ActorRole::HumanKeeper,
    );

    contract.validate_command(&command).unwrap();

    let forked = contract
        .fork_for_child(
            "campaign_001_fork",
            AuthorityMode::AiKp,
            "ai_kp_profile_001",
        )
        .unwrap();
    assert_eq!(forked.version(), 1);
    assert_eq!(forked.mode(), &AuthorityMode::AiKp);
    assert_eq!(forked.campaign_id().as_str(), "campaign_001_fork");
    assert_eq!(
        contract.fork(AuthorityMode::AiKp, 2).unwrap_err(),
        TrpgError::AuthorityContractMutation
    );
    assert_eq!(contract.version(), 1);
    assert_eq!(contract.mode(), &AuthorityMode::HumanKp);
}

#[test]
fn shared_kernel_replay_redacts_visibility_restricted_events() {
    let player = EntityId::new("character_001").unwrap();
    let mut command = trpg_test_support::governed_command(
        "secret",
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
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
fn unscoped_replay_fails_closed_for_multi_campaign_stores() {
    let contract_a =
        trpg_test_support::authority_contract("campaign_replay_a", AuthorityMode::HumanKp, 1)
            .unwrap();
    let contract_b =
        trpg_test_support::authority_contract("campaign_replay_b", AuthorityMode::HumanKp, 1)
            .unwrap();
    let mut command_a = trpg_test_support::governed_command_for_contract(
        &contract_a,
        "secret-a",
        ActorRole::HumanKeeper,
    );
    command_a.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let mut command_b = trpg_test_support::governed_command_for_contract(
        &contract_b,
        "secret-b",
        ActorRole::HumanKeeper,
    );
    command_b.visibility = Visibility::new(VisibilityLabel::KeeperOnly);

    let mut store = EventStore::default();
    store
        .append(&command_a, "CampaignASecret", "secret-a")
        .unwrap();
    store
        .append(&command_b, "CampaignBSecret", "secret-b")
        .unwrap();

    assert!(store.replay_visible(&PrincipalScope::Keeper).is_empty());
    assert_eq!(
        store
            .replay_visible_in_campaign(contract_a.campaign_id(), &PrincipalScope::Keeper)
            .len(),
        1
    );
}

#[test]
fn shared_kernel_replay_never_exposes_ai_internal_to_players() {
    let mut command =
        trpg_test_support::governed_command("internal", ActorRole::System, AuthorityMode::AiKp);
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

#[test]
fn event_integrity_binds_the_recorded_payload() {
    let command = trpg_test_support::governed_command(
        "recorded payload".to_owned(),
        ActorRole::System,
        AuthorityMode::AiKp,
    );
    let mut store = EventStore::default();
    let mut event = store
        .append(
            &command,
            "SharedKernelTypesValidated",
            "recorded payload".to_owned(),
        )
        .unwrap();

    event.verify_recorded_integrity().unwrap();
    event.payload = "substituted payload".to_owned();

    assert_eq!(
        event.verify_recorded_integrity().unwrap_err(),
        TrpgError::PolicyEvidenceUntrusted
    );
}
