use trpg_domain_core::ddd::{require_confirmable_fact_source, DomainError, FactSource};

#[test]
fn ddd_defines_confirmed_fact_source_boundary() {
    assert!(require_confirmable_fact_source(FactSource::GameEvent).is_ok());
    assert!(require_confirmable_fact_source(FactSource::DecisionRecord).is_ok());
    assert_eq!(
        require_confirmable_fact_source(FactSource::AgentDraft).unwrap_err(),
        DomainError::InvalidConfirmedFactSource
    );
    assert_eq!(
        DomainError::AuthorityContractImmutable.code(),
        "AUTHORITY_CONTRACT_IMMUTABLE"
    );
}
