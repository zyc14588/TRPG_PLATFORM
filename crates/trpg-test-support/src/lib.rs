#![forbid(unsafe_code)]

#[macro_export]
macro_rules! governed_command {
    ($payload:expr, $actor_role:expr, $mode:expr $(,)?) => {{
        trpg_shared_kernel::CommandEnvelope {
            command_id: trpg_shared_kernel::EntityId::new("command_001")
                .expect("valid fixture command id"),
            idempotency_key: "idem_001".to_owned(),
            expected_version: 0,
            actor: trpg_shared_kernel::Actor::new("actor_001", $actor_role)
                .expect("valid fixture actor"),
            authority_mode: $mode,
            authority_contract_version: 1,
            visibility: trpg_shared_kernel::Visibility::new(
                trpg_shared_kernel::VisibilityLabel::SystemOnly,
            ),
            fact_provenance: trpg_shared_kernel::FactProvenance::new(
                trpg_shared_kernel::ProvenanceKind::RulesEngineDecision,
                "fact_001",
                "rules_001",
            )
            .expect("valid fixture provenance"),
            correlation_id: trpg_shared_kernel::EntityId::new("corr_001")
                .expect("valid fixture correlation id"),
            causation_id: trpg_shared_kernel::EntityId::new("cause_001")
                .expect("valid fixture causation id"),
            write_path: trpg_shared_kernel::FormalWritePath::WorkflowDecision,
            payload: $payload,
        }
    }};
}
