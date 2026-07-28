
#[test]
fn plugin_tool_request_cannot_preclaim_tool_result_provenance() {
    let input = r#"{"campaign_id":"campaign_plugin"}"#;
    let module = output_module(
        r#"{"kind":"tool_request","visibility_label":"keeper_only","visibility_subject":"not_applicable","provenance_kind":"tool_result","provenance_reference":"result_before_execution","provenance_recorded_by":"coc7_wasm_plugin","payload":{"tool":"lookup"}}"#,
    );
    let grants =
        ExtensionCapabilityGrantSet::with_grants(&[ExtensionCapability::InvokeGrantedTool])
            .unwrap();
    let host = PluginHost::new(200_000, 4 * 1024 * 1024).unwrap();
    let mut tool_manifest = manifest(&module);
    tool_manifest.requested_capabilities = vec![ExtensionCapability::InvokeGrantedTool];
    let plugin = host.register(tool_manifest, &module, &grants).unwrap();

    assert_eq!(
        host.invoke(
            &plugin,
            input,
            &invocation_context(input, ExtensionCapability::InvokeGrantedTool),
        )
        .unwrap_err(),
        PluginHostError::OutputInvalid
    );
}

#[test]
fn tool_result_provenance_is_minted_only_after_successful_execution() {
    let input = r#"{"campaign_id":"campaign_plugin"}"#;
    let module = output_module(r#"{"kind":"tool_request","payload":{"query":"harbor ledger"}}"#);
    let grants =
        ExtensionCapabilityGrantSet::with_grants(&[ExtensionCapability::InvokeGrantedTool])
            .unwrap();
    let host = PluginHost::new(200_000, 4 * 1024 * 1024).unwrap();
    let mut tool_manifest = manifest(&module);
    tool_manifest.requested_capabilities = vec![ExtensionCapability::InvokeGrantedTool];
    let plugin = host.register(tool_manifest, &module, &grants).unwrap();
    let request = host
        .invoke(
            &plugin,
            input,
            &invocation_context(input, ExtensionCapability::InvokeGrantedTool),
        )
        .unwrap();

    assert_eq!(request.kind(), PluginOutputKind::ToolRequest);
    assert_eq!(
        request.fact_provenance().kind,
        ProvenanceKind::AgentProposal
    );

    let authorization = tool_authorization(
        &request,
        "coc7_search_provider",
        "search_records",
        "tool_schema.v1",
    );
    let result = execute_granted_tool(
        &authorization,
        &request,
        "coc7_search_provider",
        "search_records",
        "tool_schema.v1",
        |input| {
            assert_eq!(input["query"], "harbor ledger");
            Ok(serde_json::json!({"matches": ["ledger-17"]}))
        },
    )
    .unwrap();

    assert_eq!(result.visibility(), request.visibility());
    assert_eq!(result.fact_provenance().kind, ProvenanceKind::ToolResult);
    assert_eq!(
        result.fact_provenance().recorded_by.as_str(),
        "coc7_search_provider"
    );
    assert_eq!(result.receipt().request_id(), request.request_id());
    assert_eq!(result.receipt().tool_schema_version(), "tool_schema.v1");
    assert_ne!(
        result.receipt().input_sha256(),
        result.receipt().output_sha256()
    );
    assert_eq!(result.payload()["matches"][0], "ledger-17");

    let mismatch = execute_granted_tool(
        &authorization,
        &request,
        "different_provider",
        "search_records",
        "tool_schema.v1",
        |_| panic!("a mismatched authorization must fail before execution"),
    )
    .expect_err("provider substitution must invalidate exact authorization");
    assert_eq!(mismatch.code(), TrpgError::PolicyEvidenceUntrusted.code());
}

#[test]
fn failed_tool_execution_cannot_produce_tool_result_provenance() {
    let module = output_module(r#"{"kind":"tool_request","payload":{"query":"fail"}}"#);
    let grants =
        ExtensionCapabilityGrantSet::with_grants(&[ExtensionCapability::InvokeGrantedTool])
            .unwrap();
    let host = PluginHost::new(200_000, 4 * 1024 * 1024).unwrap();
    let mut tool_manifest = manifest(&module);
    tool_manifest.requested_capabilities = vec![ExtensionCapability::InvokeGrantedTool];
    let plugin = host.register(tool_manifest, &module, &grants).unwrap();
    let request = host
        .invoke(
            &plugin,
            "{}",
            &invocation_context("{}", ExtensionCapability::InvokeGrantedTool),
        )
        .unwrap();

    let authorization = tool_authorization(
        &request,
        "coc7_search_provider",
        "search_records",
        "tool_schema.v1",
    );
    let failure = execute_granted_tool(
        &authorization,
        &request,
        "coc7_search_provider",
        "search_records",
        "tool_schema.v1",
        |_| Err(ExtensionSdkError::Kernel(TrpgError::PolicyDenied)),
    )
    .expect_err("failed execution must not create a trusted tool result");

    assert_eq!(failure.code(), TrpgError::PolicyDenied.code());
}
