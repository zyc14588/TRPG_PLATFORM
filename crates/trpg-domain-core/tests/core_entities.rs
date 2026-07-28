use trpg_domain_core::ddd::AuthorityMode;
use trpg_domain_core::domain_entities_value_objects::{
    CampaignAggregate, CampaignInvite, Character, CharacterState, CoreDomainEvent, CoreEntityError,
    MembershipRole, Reconsideration, ReconsiderationOutcome, ReconsiderationState, Session,
    SessionState, UserId,
};
use trpg_test_support::authority_contract;

#[test]
fn stable_core_entities_reject_invalid_ids_and_preserve_authority_reference() {
    let campaign = CampaignAggregate::new(
        "campaign_core_entities",
        "user_campaign_owner",
        "authority_campaign_core_entities_1",
        "The Mist Archive",
        1_000,
    )
    .expect("valid campaign aggregate");
    assert_eq!(campaign.campaign_id.as_str(), "campaign_core_entities");
    assert_eq!(campaign.owner_user_id.as_str(), "user_campaign_owner");
    assert_eq!(campaign.version, 1);

    assert_eq!(
        CampaignAggregate::new(
            "contains spaces",
            "user_campaign_owner",
            "authority_campaign_core_entities_1",
            "The Mist Archive",
            1_000,
        )
        .unwrap_err(),
        CoreEntityError::InvalidIdentifier
    );

    let authority =
        authority_contract("campaign_core_entities", AuthorityMode::HumanKp, 1).unwrap();
    assert!(authority.is_locked());
    assert_eq!(
        authority.campaign_id().as_str(),
        campaign.campaign_id.as_str()
    );
}

#[test]
fn session_and_character_lifecycles_reject_illegal_transitions() {
    let mut session = Session::scheduled(
        "session_core_entities",
        "campaign_core_entities",
        "room_core_entities",
        "scenario_core_entities",
    )
    .unwrap();
    assert_eq!(
        session.transition(SessionState::Paused).unwrap_err(),
        CoreEntityError::InvalidTransition {
            aggregate: "session",
            from: "SCHEDULED",
            to: "PAUSED",
        }
    );
    session.transition(SessionState::Active).unwrap();
    session.transition(SessionState::Paused).unwrap();
    session.transition(SessionState::Active).unwrap();
    session.transition(SessionState::Ended).unwrap();
    assert_eq!(session.version, 4);
    assert!(session.transition(SessionState::Active).is_err());

    let mut character = Character::draft(
        "character_core_entities",
        "campaign_core_entities",
        "user_investigator",
        "Evelyn Marsh",
    )
    .unwrap();
    assert_eq!(
        character.approve_initial_version().unwrap_err(),
        CoreEntityError::CharacterSheetNotSubmitted
    );
    character.submit().unwrap();
    character.approve_initial_version().unwrap();
    assert_eq!(character.state, CharacterState::Approved);
    assert!(character.initial_version_locked);
    assert_eq!(
        character.submit().unwrap_err(),
        CoreEntityError::CharacterSheetAlreadyLocked
    );
}

#[test]
fn invite_expiry_and_subject_are_enforced_without_server_state() {
    let invite = CampaignInvite::new(
        "invite_core_entities",
        "campaign_core_entities",
        "user_invited",
        "user_campaign_owner",
        MembershipRole::Player,
        format!("sha256:{}", "a".repeat(64)),
        2_000,
        1_000,
    )
    .unwrap();
    invite
        .validate_acceptance(&UserId::new("user_invited").unwrap(), 1_999)
        .unwrap();
    assert_eq!(
        invite
            .validate_acceptance(&UserId::new("user_other").unwrap(), 1_999)
            .unwrap_err(),
        CoreEntityError::InviteSubjectMismatch
    );
    assert_eq!(
        invite
            .validate_acceptance(&UserId::new("user_invited").unwrap(), 2_000)
            .unwrap_err(),
        CoreEntityError::InviteExpired
    );
}

#[test]
fn reconsideration_is_append_only_and_core_events_are_versioned() {
    let mut reconsideration = Reconsideration::requested(
        "reconsideration_core_entities",
        "campaign_core_entities",
        41,
        "user_investigator",
        "event_reconsideration_requested",
    )
    .unwrap();
    reconsideration
        .append_review_event("event_reconsideration_reviewed")
        .unwrap();
    reconsideration
        .resolve(
            "event_reconsideration_corrected",
            ReconsiderationOutcome::Corrected,
        )
        .unwrap();
    assert_eq!(reconsideration.state, ReconsiderationState::Resolved);
    assert_eq!(
        reconsideration.outcome,
        Some(ReconsiderationOutcome::Corrected)
    );
    assert_eq!(reconsideration.event_chain.len(), 3);
    assert_eq!(
        reconsideration
            .resolve("event_after_resolution", ReconsiderationOutcome::Upheld)
            .unwrap_err(),
        CoreEntityError::EventChainInvalid
    );

    let event = CoreDomainEvent::ReconsiderationRequested {
        schema_version: CoreDomainEvent::SCHEMA_VERSION,
        reconsideration_id: "reconsideration_core_entities".to_owned(),
        campaign_id: "campaign_core_entities".to_owned(),
        original_event_sequence: 41,
        requested_by: "user_investigator".to_owned(),
        reason: "rule citation was incomplete".to_owned(),
    };
    event.validate_schema_version().unwrap();
    assert_eq!(event.event_type(), "ReconsiderationRequested");
}
