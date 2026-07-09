use trpg_testing::{
    readme, record_contract_decision, TESTING_QUALITY_METRIC_MODULE,
    TESTING_QUALITY_REQUIRED_METRICS,
};

#[test]
fn readme_contract_lists_current_safe_testing_metrics() {
    record_contract_decision(&readme::contract()).expect("recorded");

    assert_eq!(TESTING_QUALITY_METRIC_MODULE, "testing_quality");
    assert_eq!(readme::required_metrics(), TESTING_QUALITY_REQUIRED_METRICS);
    assert!(readme::required_metrics()
        .iter()
        .all(|metric| metric.starts_with("trpg_testing_")));
    assert!(readme::required_metrics()
        .iter()
        .all(|metric| !metric.contains("v5") && !metric.contains("legacy")));
}
