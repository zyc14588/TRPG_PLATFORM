use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, DomainError, PrincipalScope, Visibility, VisibilityLabel,
};
use trpg_domain_core::openfga_opa_visibility::{
    evaluate_openfga_opa_visibility, OpaContextDecision, OpenFgaOpaVisibilityContext,
    OpenFgaRelationDecision,
};

#[test]
fn openfga_opa_visibility_denies_by_default() {
    let command =
        trpg_test_support::governed_command!("payload", ActorRole::Workflow, AuthorityMode::AiKp);
    let context = OpenFgaOpaVisibilityContext {
        principal: PrincipalScope::Keeper,
        relation: OpenFgaRelationDecision::Deny,
        policy: OpaContextDecision::Allow,
        target_visibility: Visibility::new(VisibilityLabel::KeeperOnly),
    };

    assert_eq!(
        evaluate_openfga_opa_visibility(&command, &context).unwrap_err(),
        DomainError::PolicyDenied
    );
}

#[test]
fn openfga_opa_visibility_requires_relation_policy_and_visibility() {
    let command =
        trpg_test_support::governed_command!("payload", ActorRole::Workflow, AuthorityMode::AiKp);
    let context = OpenFgaOpaVisibilityContext {
        principal: PrincipalScope::Keeper,
        relation: OpenFgaRelationDecision::Allow,
        policy: OpaContextDecision::Allow,
        target_visibility: Visibility::new(VisibilityLabel::KeeperOnly),
    };

    assert!(
        evaluate_openfga_opa_visibility(&command, &context)
            .unwrap()
            .allowed
    );
}
