use trpg_ruleset_coc7::dice_roll_contract::server_roll_skill_growth;
use trpg_runtime::session_runtime::{
    CampaignConclusion, ConclusionError, ConclusionState, DurableSessionState, SkillGrowthRecord,
};

#[test]
fn ending_requires_an_ended_session_and_growth_completes_once() {
    assert_eq!(
        CampaignConclusion::begin(
            "campaign_conclusion",
            "session_conclusion",
            DurableSessionState::Active
        )
        .unwrap_err(),
        ConclusionError::SessionNotEnded
    );
    let mut conclusion = CampaignConclusion::begin(
        "campaign_conclusion",
        "session_conclusion",
        DurableSessionState::Ended,
    )
    .unwrap();
    conclusion
        .record_ending(
            "ending_event_conclusion",
            "ending_expose_marta",
            "The investigators expose Marta.",
            2_000_000_000_000,
        )
        .unwrap();
    assert_eq!(conclusion.state, ConclusionState::AwaitingGrowth);

    let growth_roll = server_roll_skill_growth(0).unwrap();
    let expected_after = growth_roll.outcome().skill_after;
    let growth = SkillGrowthRecord::from_server_roll(
        "growth_event_conclusion",
        "character_conclusion",
        "sheet_character_conclusion_1",
        "sheet_character_conclusion_2",
        "Library Use",
        0,
        growth_roll.evidence(),
    )
    .unwrap();
    conclusion.settle_growth(vec![growth]).unwrap();
    assert_eq!(conclusion.state, ConclusionState::Completed);
    assert_eq!(conclusion.version, 2);
    assert_eq!(conclusion.growth[0].skill_after(), expected_after);
    assert_eq!(
        conclusion.growth[0].server_roll_id().as_str(),
        growth_roll.roll_id()
    );
    assert!(matches!(
        conclusion.settle_growth(Vec::new()),
        Err(ConclusionError::InvalidTransition {
            operation: "SETTLE_GROWTH",
            ..
        })
    ));
}

#[test]
fn growth_requires_an_opaque_server_roll_and_rejects_duplicate_results() {
    let server_roll = server_roll_skill_growth(0).unwrap();
    assert!(SkillGrowthRecord::from_server_roll(
        "growth_event_bad",
        "character_conclusion",
        "sheet_character_conclusion_1",
        "sheet_character_conclusion_2",
        " ",
        0,
        server_roll.evidence(),
    )
    .is_err());

    let mut conclusion = CampaignConclusion::begin(
        "campaign_conclusion",
        "session_conclusion",
        DurableSessionState::Ended,
    )
    .unwrap();
    conclusion
        .record_ending(
            "ending_event_conclusion",
            "ending_expose_marta",
            "The investigators expose Marta.",
            2_000_000_000_000,
        )
        .unwrap();
    let first = SkillGrowthRecord::from_server_roll(
        "growth_event_duplicate",
        "character_conclusion",
        "sheet_character_conclusion_1",
        "sheet_character_conclusion_2",
        "Library Use",
        0,
        server_roll.evidence(),
    )
    .unwrap();
    assert_eq!(
        conclusion
            .settle_growth(vec![first.clone(), first])
            .unwrap_err(),
        ConclusionError::DuplicateGrowth
    );
}

#[test]
fn an_ending_without_growth_awards_can_complete_with_an_empty_settlement() {
    let mut conclusion = CampaignConclusion::begin(
        "campaign_conclusion_without_growth",
        "session_conclusion_without_growth",
        DurableSessionState::Ended,
    )
    .unwrap();
    conclusion
        .record_ending(
            "ending_event_without_growth",
            "ending_without_growth",
            "The investigators leave without a growth award.",
            2_000_000_000_000,
        )
        .unwrap();

    conclusion.settle_growth(Vec::new()).unwrap();

    assert_eq!(conclusion.state, ConclusionState::Completed);
    assert_eq!(conclusion.version, 2);
    assert!(conclusion.growth.is_empty());
}
