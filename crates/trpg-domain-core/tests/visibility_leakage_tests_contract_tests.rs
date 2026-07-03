use trpg_domain_core::ddd::{PrincipalScope, Visibility, VisibilityLabel};
use trpg_domain_core::visibility_fact_provenance::DerivedObject;
use trpg_domain_core::visibility_leakage_tests::{
    detect_visibility_leakage, VisibilityLeakageProbe,
};

#[test]
fn visibility_leakage_tests_detect_no_keeper_only_player_export_leak() {
    let probe = VisibilityLeakageProbe {
        derived_object: DerivedObject::PlayerExport,
        principal: PrincipalScope::PartyMember,
    };

    assert!(!detect_visibility_leakage(
        &Visibility::new(VisibilityLabel::KeeperOnly),
        &probe
    ));
}

#[test]
fn visibility_leakage_tests_allow_public_player_export() {
    let probe = VisibilityLeakageProbe {
        derived_object: DerivedObject::PlayerExport,
        principal: PrincipalScope::PartyMember,
    };

    assert!(detect_visibility_leakage(
        &Visibility::new(VisibilityLabel::Public),
        &probe
    ));
}
