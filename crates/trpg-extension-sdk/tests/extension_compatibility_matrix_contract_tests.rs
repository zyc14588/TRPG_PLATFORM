mod common;

use trpg_extension_sdk::extension_compatibility_matrix::{
    append_extension_compatibility_matrix_event, contract, record_compatible_extension,
    CompatibilityMatrix, ExtensionCompatibilityMatrixCommand,
};
use trpg_extension_sdk::{
    CompatibilityResult, ExtensionEvent, ExtensionEventStore, SdkCompatibilityReport, Visibility,
    VisibilityLabel,
};

#[test]
fn extension_compatibility_matrix_records_governed_event() {
    common::assert_extension_contract(
        contract(),
        ExtensionCompatibilityMatrixCommand::record("compatibility matrix"),
        append_extension_compatibility_matrix_event,
    );
}

#[test]
fn extension_compatibility_matrix_requires_fixture_fields() {
    let matrix = CompatibilityMatrix::current();
    let report =
        SdkCompatibilityReport::compatible("coc7_sample_extension", "7e", "tool_schema.v1");

    assert!(matrix.evaluate(&report).is_ok());

    let rejected = SdkCompatibilityReport {
        compatibility_result: CompatibilityResult::Incompatible,
        ..report
    };
    assert_eq!(
        matrix
            .evaluate(&rejected)
            .expect_err("incompatible extension is rejected")
            .code(),
        "EXTENSION_COMPATIBILITY_REJECTED"
    );
}

#[test]
fn extension_compatibility_matrix_records_compatible_report_as_event() {
    let authority = common::authority_contract();
    let mut store = ExtensionEventStore::default();
    let command = common::governed_command(
        ExtensionCompatibilityMatrixCommand::record("record report"),
        0,
        "idem_compat_report",
        Visibility::new(VisibilityLabel::SystemOnly),
    );
    let report =
        SdkCompatibilityReport::compatible("coc7_sample_extension", "7e", "tool_schema.v1");

    let event = record_compatible_extension(&mut store, &authority, &command, report)
        .expect("compatible report is recorded");

    match event.payload {
        ExtensionEvent::CompatibilityChecked(report) => {
            assert!(report.has_required_fields());
            assert_eq!(report.compatibility_result, CompatibilityResult::Compatible);
        }
        other => panic!("unexpected event payload: {other:?}"),
    }
}
