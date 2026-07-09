use trpg_extension_sdk::extension_compatibility_matrix::CompatibilityMatrix;
use trpg_extension_sdk::SdkCompatibilityReport;

#[test]
fn extension_compatibility_matrix_stage_gate_accepts_coc7_fixture() {
    let matrix = CompatibilityMatrix::current();
    let report =
        SdkCompatibilityReport::compatible("coc7_sample_extension", "7e", "tool_schema.v1");

    matrix
        .evaluate(&report)
        .expect("S12 compatibility fixture is current");
}
