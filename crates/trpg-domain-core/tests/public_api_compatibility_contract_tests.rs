#[test]
fn compatibility_modules_reexport_the_canonical_implementations() {
    assert_eq!(
        trpg_domain_core::PUBLIC_COMPATIBILITY_MODULES,
        [
            "character_combat_san_chase_impl",
            "investigation_clue_npc_time_impl",
            "rule_runtime_coc7_impl",
        ]
    );

    let canonical =
        trpg_domain_core::rule_runtime_coc7::Coc7RuleRuntimeDecision::SkillCheck.fact_source();
    let compatibility =
        trpg_domain_core::rule_runtime_coc7_impl::Coc7RuleRuntimeDecision::SkillCheck.fact_source();
    assert_eq!(canonical, compatibility);
}
