use trpg_shared_kernel::dependency_direction::{
    validate_dependency_direction, CrateLayer, DependencyEdge,
};
use trpg_shared_kernel::TrpgError;

#[test]
fn dependency_direction_allows_upper_layers_to_depend_on_shared_kernel() {
    let edges = [DependencyEdge {
        from: CrateLayer::Api,
        to: CrateLayer::SharedKernel,
    }];

    validate_dependency_direction(&edges).unwrap();
}

#[test]
fn dependency_direction_blocks_shared_kernel_from_domain_or_agent() {
    let edges = [DependencyEdge {
        from: CrateLayer::SharedKernel,
        to: CrateLayer::Agent,
    }];

    assert!(matches!(
        validate_dependency_direction(&edges),
        Err(TrpgError::DependencyViolation(_))
    ));
}

#[test]
fn dependency_direction_blocks_lower_to_higher_dependency() {
    let edges = [DependencyEdge {
        from: CrateLayer::Domain,
        to: CrateLayer::Runtime,
    }];

    assert!(matches!(
        validate_dependency_direction(&edges),
        Err(TrpgError::DependencyViolation(_))
    ));
}

#[test]
fn contracts_cannot_depend_on_shared_kernel() {
    let edges = [DependencyEdge {
        from: CrateLayer::Contracts,
        to: CrateLayer::SharedKernel,
    }];

    assert!(matches!(
        validate_dependency_direction(&edges),
        Err(TrpgError::DependencyViolation(_))
    ));
}
