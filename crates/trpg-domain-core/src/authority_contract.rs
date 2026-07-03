use crate::ddd::{
    AuthorityMode, CommandEnvelope, DomainError, DomainResult, EntityId, KernelAuthorityContract,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangePolicy {
    ForkOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainAuthorityContract {
    pub contract_id: EntityId,
    pub campaign_id: EntityId,
    pub authority_mode: AuthorityMode,
    pub authority_owner: EntityId,
    pub version: u64,
    pub locked: bool,
    pub change_policy: ChangePolicy,
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
