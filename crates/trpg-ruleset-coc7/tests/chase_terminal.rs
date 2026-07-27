use trpg_ruleset_coc7::chase_state_machine::{
    advance_chase, ChaseObstacle, ChaseParticipant, ChaseRole, ChaseState, ChaseStatus,
};
use trpg_shared_kernel::TrpgError;

#[test]
fn escaped_and_caught_chases_reject_normal_advancement() {
    let escaped = advance_chase(4, ChaseStatus::Ongoing, true, false, 0).unwrap();
    assert_eq!(escaped.status, ChaseStatus::Escaped);
    assert_eq!(
        advance_chase(escaped.after_range, escaped.status, false, true, 0).unwrap_err(),
        TrpgError::InvalidConfiguration("chase_terminal")
    );

    let caught = advance_chase(1, ChaseStatus::Ongoing, false, true, 0).unwrap();
    assert_eq!(caught.status, ChaseStatus::Caught);
    assert_eq!(
        advance_chase(caught.after_range, caught.status, true, false, 0).unwrap_err(),
        TrpgError::InvalidConfiguration("chase_terminal")
    );
}

#[test]
fn a_new_chase_requires_a_new_identity_and_owns_participants_and_obstacles() {
    let quarry = ChaseParticipant::new("character_ada", ChaseRole::Quarry, 8).unwrap();
    let pursuer = ChaseParticipant::new("npc_salt_wight", ChaseRole::Pursuer, 7).unwrap();
    let mut first = ChaseState::start(
        "chase_cellar_escape",
        vec![quarry.clone(), pursuer.clone()],
        4,
    )
    .unwrap();
    let obstacle = ChaseObstacle::new("obstacle_archive_stairs", 1).unwrap();

    let terminal = first.advance(true, false, Some(&obstacle)).unwrap();
    assert_eq!(terminal.status, ChaseStatus::Escaped);
    assert_eq!(
        first.advance(false, true, None).unwrap_err(),
        TrpgError::InvalidConfiguration("chase_terminal")
    );

    let second = ChaseState::start("chase_harbor_road", vec![quarry, pursuer], 2).unwrap();
    assert_ne!(first.chase_id(), second.chase_id());
    assert_eq!(second.status(), ChaseStatus::Ongoing);
    assert_eq!(second.segment(), 1);
}
