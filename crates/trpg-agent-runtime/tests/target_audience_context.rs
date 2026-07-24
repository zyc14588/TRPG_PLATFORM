pub mod common;

use trpg_agent_runtime::{agent_context_assembler, assemble_context_for_audience};
use trpg_shared_kernel::{EntityId, PrincipalScope, Visibility, VisibilityLabel};

#[test]
fn system_processor_cannot_put_keeper_or_system_facts_in_player_context() {
    let facts = vec![
        common::context_fact(
            "public_fact",
            "the door is open",
            Visibility::new(VisibilityLabel::Public),
        )
        .unwrap(),
        common::context_fact(
            "keeper_fact",
            "keeper truth",
            Visibility::new(VisibilityLabel::KeeperOnly),
        )
        .unwrap(),
        common::context_fact(
            "system_fact",
            "system routing data",
            Visibility::new(VisibilityLabel::SystemOnly),
        )
        .unwrap(),
        common::context_fact(
            "player_fact",
            "private investigator note",
            Visibility::private_to_player(EntityId::new("player_a").unwrap()),
        )
        .unwrap(),
    ];

    let player = PrincipalScope::Player(EntityId::new("player_a").unwrap());
    let context = assemble_context_for_audience(&facts, &PrincipalScope::System, &player);

    assert_eq!(context.facts.len(), 2);
    assert!(context
        .facts
        .iter()
        .all(|fact| fact.visibility.can_view(&player)));
    assert!(!context
        .facts
        .iter()
        .any(|fact| fact.text.contains("keeper")));
    assert!(!context
        .facts
        .iter()
        .any(|fact| fact.text.contains("routing")));
    assert_eq!(
        context.derived_visibility.subject_id().unwrap().as_str(),
        "player_a"
    );
}

#[test]
fn public_context_wrapper_does_not_treat_system_processor_as_the_audience() {
    let public = common::context_fact(
        "wrapper_public_fact",
        "the public notice",
        Visibility::new(VisibilityLabel::Public),
    )
    .unwrap();
    let keeper = common::context_fact(
        "wrapper_keeper_fact",
        "the keeper solution",
        Visibility::new(VisibilityLabel::KeeperOnly),
    )
    .unwrap();

    let context = agent_context_assembler::assemble_agent_context(
        &[public.clone(), keeper],
        &PrincipalScope::System,
        &PrincipalScope::Public,
    );

    assert_eq!(context.facts, vec![public]);
}
