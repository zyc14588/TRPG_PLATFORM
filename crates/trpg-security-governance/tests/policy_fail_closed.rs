// Source is organized into ordered, responsibility-focused sections.
// `include!` preserves this module's privacy boundary and public API.

#[test]
fn deletion_action_is_identical_in_rust_openfga_and_opa() {
    let (openfga, captured_openfga) = one_shot_policy_response_with_capture(
        r#"{"allowed":true}"#,
        "X-Request-Id: openfga-delete-personal-data\r\n",
    );
    let (opa, captured_opa) = one_shot_policy_response_with_capture(
        r#"{"result":{"allow":true,"decision_id":"opa-delete-personal-data","policy_revision":"opa-security-governance-v3"}}"#,
        "",
    );
    let policy = adapter(
        openfga,
        "/stores/test/check".to_owned(),
        "model-test".to_owned(),
        opa,
        "opa-security-governance-v3".to_owned(),
    );
    let path = audit_path("delete-personal-data-vocabulary");
    let mut audit = open_audit(&path);
    evaluate_with_trusted_workload(
        "policy_test",
        &mut SecurityGovernanceRepository::default(),
        &command(SecurityGovernanceAction::DeletePersonalData),
        &policy,
        &mut audit,
    )
    .unwrap();

    let request_json = |request: Vec<u8>| {
        let body_start = request
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("HTTP request boundary")
            + 4;
        serde_json::from_slice::<serde_json::Value>(&request[body_start..])
            .expect("policy request JSON")
    };
    let openfga_body = request_json(
        captured_openfga
            .recv_timeout(Duration::from_secs(2))
            .expect("capture OpenFGA request"),
    );
    let opa_body = request_json(
        captured_opa
            .recv_timeout(Duration::from_secs(2))
            .expect("capture OPA request"),
    );
    assert_eq!(
        openfga_body["tuple_key"]["relation"],
        "can_delete_personal_data"
    );
    assert_eq!(opa_body["input"]["action"], "delete_personal_data");

    let fga = include_str!("../../../policy/openfga/security_governance.fga");
    let rego = include_str!("../../../policy/opa/security_governance.rego");
    assert!(fga.contains("define can_delete_personal_data: workflow or system"));
    assert!(rego.contains("input.action in {\"delete_personal_data\""));
    assert!(!fga.contains("delete_retained_data"));
    assert!(!rego.contains("delete_retained_data"));
    cleanup_audit(&path);
}

include!("policy_fail_closed/01_module_prelude.rs");
include!("policy_fail_closed/02_openfga_request_does_not_recreate_the_callers_role_as_a_contextual_tuple.rs");
