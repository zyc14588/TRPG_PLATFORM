use trpg_shared_kernel::crate_ownership::{
    validate_crate_ownership, CrateOwner, CrateOwnership, WriteAuthority,
};
use trpg_shared_kernel::TrpgError;

#[test]
fn crate_ownership_keeps_shared_kernel_owner_fixed() {
    let ownership = CrateOwnership {
        crate_name: "trpg-shared-kernel".to_owned(),
        owner: CrateOwner::SharedKernel,
        write_authority: WriteAuthority::SharedKernelCore,
    };

    validate_crate_ownership(&ownership).unwrap();
}

#[test]
fn crate_ownership_blocks_agent_formal_write_authority() {
    let ownership = CrateOwnership {
        crate_name: "trpg-agent-runtime".to_owned(),
        owner: CrateOwner::Agent,
        write_authority: WriteAuthority::DomainWorkflow,
    };

    assert!(matches!(
        validate_crate_ownership(&ownership),
        Err(TrpgError::CrateOwnershipViolation(_))
    ));
}

#[test]
fn crate_ownership_rejects_non_kernel_owner_for_shared_kernel() {
    let ownership = CrateOwnership {
        crate_name: "trpg-shared-kernel".to_owned(),
        owner: CrateOwner::Api,
        write_authority: WriteAuthority::ApiBoundary,
    };

    assert!(matches!(
        validate_crate_ownership(&ownership),
        Err(TrpgError::CrateOwnershipViolation(_))
    ));
}
