use trpg_domain_core::authority_contract::DomainAuthorityContract;
use trpg_domain_core::authority_contract_guard::guard_authority_contract;
use trpg_domain_core::ddd::{ActorRole, AuthorityMode, CommandEnvelope, DomainError};

#[test]
fn authority_contract_guard_allows_matching_workflow_command() {
    let contract = DomainAuthorityContract::new_locked(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();
    let command = CommandEnvelope::governed("payload", ActorRole::Workflow, AuthorityMode::AiKp);

    let decision = guard_authority_contract(&contract, &command).unwrap();

    assert!(decision.accepted);
}

#[test]
fn authority_contract_guard_rejects_human_override_in_ai_kp_mode() {
    let contract = DomainAuthorityContract::new_locked(
        "camp_ai_harbor",
        AuthorityMode::AiKp,
        "ai_kp_local_level4",
        1,
    )
    .unwrap();
    let command = CommandEnvelope::governed("payload", ActorRole::HumanKeeper, AuthorityMode::AiKp);

    let error = guard_authority_contract(&contract, &command).unwrap_err();

    assert_eq!(error, DomainError::AuthorityViolation);
}
