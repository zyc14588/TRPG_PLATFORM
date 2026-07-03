use crate::ddd::{CommandEnvelope, DomainError, DomainResult, PrincipalScope, Visibility};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenFgaRelationDecision {
    Allow,
    Deny,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpaContextDecision {
    Allow,
    Deny,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenFgaOpaVisibilityContext {
    pub principal: PrincipalScope,
    pub relation: OpenFgaRelationDecision,
    pub policy: OpaContextDecision,
    pub target_visibility: Visibility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenFgaOpaVisibilityDecision {
    pub allowed: bool,
}

pub fn evaluate_openfga_opa_visibility<T>(
    command: &CommandEnvelope<T>,
    context: &OpenFgaOpaVisibilityContext,
) -> DomainResult<OpenFgaOpaVisibilityDecision> {
    trpg_shared_kernel::shared_kernel::validate_command_envelope(command)?;

    if context.relation != OpenFgaRelationDecision::Allow
        || context.policy != OpaContextDecision::Allow
    {
        return Err(DomainError::PolicyDenied);
    }

    if !context.target_visibility.can_view(&context.principal) {
        return Err(DomainError::VisibilityDenied);
    }

    Ok(OpenFgaOpaVisibilityDecision { allowed: true })
}
