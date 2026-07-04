use crate::ddd::{
    AuthorityMode, CommandEnvelope, DomainError, DomainResult, EntityId, KernelAuthorityContract,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangePolicy {
    ForkOnly,
}

/// Immutable campaign authority contract.
///
/// ```compile_fail
/// use trpg_domain_core::authority_contract::DomainAuthorityContract;
/// use trpg_domain_core::ddd::AuthorityMode;
///
/// let mut contract =
///     DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
///         .unwrap();
/// contract.authority_mode = AuthorityMode::HumanKp;
/// ```
///
/// ```compile_fail
/// use trpg_domain_core::authority_contract::DomainAuthorityContract;
/// use trpg_domain_core::ddd::AuthorityMode;
///
/// let mut contract =
///     DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
///         .unwrap();
/// contract.version = 2;
/// ```
///
/// ```compile_fail
/// use trpg_domain_core::authority_contract::DomainAuthorityContract;
/// use trpg_domain_core::ddd::AuthorityMode;
///
/// let mut contract =
///     DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
///         .unwrap();
/// contract.locked = false;
/// ```
///
/// ```compile_fail
/// use trpg_domain_core::authority_contract::DomainAuthorityContract;
/// use trpg_domain_core::ddd::{AuthorityMode, EntityId};
///
/// let mut contract =
///     DomainAuthorityContract::new_locked("campaign_001", AuthorityMode::AiKp, "ai_kp", 1)
///         .unwrap();
/// contract.authority_owner = EntityId::new("keeper_2").unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainAuthorityContract {
    contract_id: EntityId,
    campaign_id: EntityId,
    authority_mode: AuthorityMode,
    authority_owner: EntityId,
    version: u64,
    locked: bool,
    change_policy: ChangePolicy,
}

impl DomainAuthorityContract {
    pub fn new_locked(
        campaign_id: impl Into<String>,
        authority_mode: AuthorityMode,
        authority_owner: impl Into<String>,
        version: u64,
    ) -> DomainResult<Self> {
        if version == 0 {
            return Err(DomainError::AuthorityContractImmutable);
        }

        let campaign_id = EntityId::new(campaign_id)?;
        let contract_id = EntityId::new(format!(
            "authority_contract_{}_{}",
            campaign_id.as_str(),
            version
        ))?;

        Ok(Self {
            contract_id,
            campaign_id,
            authority_mode,
            authority_owner: EntityId::new(authority_owner)?,
            version,
            locked: true,
            change_policy: ChangePolicy::ForkOnly,
        })
    }

    pub fn contract_id(&self) -> &EntityId {
        &self.contract_id
    }

    pub fn campaign_id(&self) -> &EntityId {
        &self.campaign_id
    }

    pub fn authority_mode(&self) -> &AuthorityMode {
        &self.authority_mode
    }

    pub fn authority_owner(&self) -> &EntityId {
        &self.authority_owner
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn change_policy(&self) -> ChangePolicy {
        self.change_policy
    }

    pub fn validate_command<T>(&self, command: &CommandEnvelope<T>) -> DomainResult<()> {
        if !self.locked || self.change_policy != ChangePolicy::ForkOnly {
            return Err(DomainError::AuthorityContractImmutable);
        }

        KernelAuthorityContract::new(
            self.campaign_id.as_str(),
            self.authority_mode.clone(),
            self.version,
        )?
        .validate_command(command)?;

        Ok(())
    }

    pub fn reject_in_place_authority_change(
        &self,
        attempted_mode: &AuthorityMode,
        attempted_owner: &EntityId,
    ) -> DomainResult<()> {
        if self.locked
            && (&self.authority_mode != attempted_mode || &self.authority_owner != attempted_owner)
        {
            return Err(DomainError::AuthorityContractImmutable);
        }

        Ok(())
    }

    pub fn fork_for_child(
        &self,
        child_campaign_id: impl Into<String>,
        child_mode: AuthorityMode,
        child_owner: impl Into<String>,
    ) -> DomainResult<Self> {
        Self::new_locked(child_campaign_id, child_mode, child_owner, 1)
    }
}

pub fn patch_locked_authority_contract(
    contract: &DomainAuthorityContract,
    attempted_mode: AuthorityMode,
    attempted_owner: impl Into<String>,
) -> DomainResult<()> {
    let attempted_owner = EntityId::new(attempted_owner)?;
    contract.reject_in_place_authority_change(&attempted_mode, &attempted_owner)
}
