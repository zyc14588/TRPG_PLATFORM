#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainCoreReadmeContract {
    pub crate_name: &'static str,
    pub invariants: Vec<&'static str>,
    pub non_goals: Vec<&'static str>,
}

pub fn domain_core_readme_contract() -> DomainCoreReadmeContract {
    DomainCoreReadmeContract {
        crate_name: "trpg-domain-core",
        invariants: vec![
            "authority_contract_locked",
            "human_kp_ai_kp_mutually_exclusive",
            "formal_state_via_decision_event_store",
            "visibility_label_propagates",
            "fact_provenance_required",
        ],
        non_goals: vec![
            "no_api_handler",
            "no_sqlx_migration",
            "no_nats_subject",
            "no_direct_llm_call",
        ],
    }
}
