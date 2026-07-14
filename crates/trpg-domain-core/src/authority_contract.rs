use crate::ddd::{AuthorityMode, DomainError, DomainResult, EntityId};

pub use trpg_shared_kernel::{
    AuthorityContract as DomainAuthorityContract, AuthorityContractDraft, AuthorityVersionSnapshot,
    AuthorityVersionSnapshotDraft, ChangePolicy,
};

pub fn patch_locked_authority_contract(
    contract: &DomainAuthorityContract,
    attempted_mode: AuthorityMode,
    attempted_owner: impl Into<String>,
) -> DomainResult<()> {
    let attempted_owner = EntityId::new(attempted_owner)?;
    contract
        .reject_in_place_authority_change(&attempted_mode, &attempted_owner)
        .map_err(DomainError::from)
}
