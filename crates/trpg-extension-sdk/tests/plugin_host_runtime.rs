mod common;

use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use trpg_domain_core::command_cqrs::{CommandAcceptedPayload, DomainCommandKind};
use trpg_domain_core::ddd::{
    ActorRole as DomainActorRole, EventStore as DomainEventStore,
    FactProvenance as DomainFactProvenance, FactSource as DomainFactSource,
    ProvenanceKind as DomainProvenanceKind,
};
use trpg_domain_core::CommittedFactEvidence;
use trpg_extension_sdk::plugin_host::{
    HostedPluginManifest, PluginHost, PluginHostError, PluginOutputKind,
};
use trpg_extension_sdk::plugin_sdk::PluginInvocationContext;
use trpg_extension_sdk::tool_provider_sdk::{
    execute_granted_tool, tool_invocation_resource_id, TOOL_INVOCATION_ACTION,
    TOOL_INVOCATION_REQUESTED_ROLE, TOOL_INVOCATION_RESOURCE_TYPE,
};
use trpg_extension_sdk::{
    ExtensionCapability, ExtensionCapabilityGrantSet, ExtensionSdkError, ProvenanceKind, TrpgError,
    Visibility, VisibilityLabel,
};
use trpg_identity::{IdentityService, WorkloadRole};
use trpg_security_governance::formal_commit_audit::{
    FormalAuthorization, FormalCommitAudit, FormalCommitAuthorizer,
};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};

static NEXT_TOOL_AUDIT: AtomicU64 = AtomicU64::new(1);

fn digest(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in hash {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("sha256:{encoded}")
}

fn proposal_module() -> Vec<u8> {
    let output = r#"{"kind":"proposal","payload":{"text":"inspect the ledger"}}"#;
    wat::parse_str(format!(
        r#"
        (module
          (memory (export "memory") 2 2)
          (data (i32.const 65536) "{}")
          (func (export "trpg_plugin_alloc") (param i32) (result i32)
            i32.const 0)
          (func (export "trpg_plugin_invoke") (param i32 i32) (result i64)
            i64.const {}))
        "#,
        output.replace('"', "\\22"),
        ((65_536_u64) << 32) | output.len() as u64,
    ))
    .unwrap()
}

fn invocation_context(
    input_json: &str,
    required_capability: ExtensionCapability,
) -> PluginInvocationContext {
    invocation_context_for_fact(input_json, required_capability, "plugin_source_fact_1")
}

fn invocation_context_for_fact(
    input_json: &str,
    required_capability: ExtensionCapability,
    source_fact_id: &str,
) -> PluginInvocationContext {
    let contract = common::authority_contract();
    let mut command = trpg_test_support::governed_command_for_contract(
        &contract,
        "verified plugin input",
        DomainActorRole::RulesEngine,
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.fact_provenance = DomainFactProvenance::new(
        DomainProvenanceKind::RulesEngineDecision,
        "plugin_input_decision_1",
        "rules_engine_plugin_input",
    )
    .unwrap();
    let mut store = DomainEventStore::default();
    let event = store
        .append(
            &command,
            "DecisionCommitted",
            CommandAcceptedPayload {
                kind: DomainCommandKind::RecordDecision,
                fact_source: DomainFactSource::DecisionRecord,
                target_fact_id: source_fact_id.to_owned(),
            },
        )
        .unwrap();
    let evidence = CommittedFactEvidence::load(&store, event.sequence, source_fact_id).unwrap();

    let identity = IdentityService::new(&[0x5e; 32], 60_000).unwrap();
    let credential = identity
        .issue_workload_credential("plugin_host_worker", WorkloadRole::AgentWorker, 1, 20_000)
        .unwrap();
    let authentication = identity.authenticate_workload(&credential, 2).unwrap();
    let authorization = identity
        .verifier()
        .authorize_replay(
            &authentication,
            &trpg_extension_sdk::EntityId::new("campaign_extension_001").unwrap(),
            3,
        )
        .unwrap();
    PluginInvocationContext::from_committed_sources(
        "plugin_request_1",
        "coc7_wasm_plugin",
        required_capability,
        input_json,
        &[evidence],
        &authorization,
        &authorization,
        4,
    )
    .unwrap()
}

fn output_module(output: &str) -> Vec<u8> {
    wat::parse_str(format!(
        r#"
        (module
          (memory (export "memory") 2 2)
          (data (i32.const 65536) "{}")
          (func (export "trpg_plugin_alloc") (param i32) (result i32)
            i32.const 0)
          (func (export "trpg_plugin_invoke") (param i32 i32) (result i64)
            i64.const {}))
        "#,
        output.replace('"', "\\22"),
        ((65_536_u64) << 32) | output.len() as u64,
    ))
    .unwrap()
}

fn manifest(bytes: &[u8]) -> HostedPluginManifest {
    HostedPluginManifest {
        plugin_id: "coc7_wasm_plugin".to_owned(),
        module_sha256: digest(bytes),
        requested_capabilities: vec![ExtensionCapability::EmitProposedDecision],
    }
}

fn tool_authorization(
    request: &trpg_extension_sdk::plugin_host::PluginOutput,
    provider_id: &str,
    tool_id: &str,
    tool_schema_version: &str,
) -> FormalAuthorization {
    let contract = common::authority_contract();
    let (identity_verifier, workflow_authentication) =
        trpg_test_support::formal_commit_identity_for_contract(&contract);
    let endpoints = trpg_test_support::formal_commit_policy_endpoints();
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            endpoints.openfga,
            "/stores/test/check",
            PolicyBackend::OpenFga,
            endpoints.openfga_model,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            endpoints.opa,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            endpoints.opa_revision,
        )
        .unwrap(),
    )
    .unwrap();
    let nonce = NEXT_TOOL_AUDIT.fetch_add(1, Ordering::Relaxed);
    let audit_path = std::env::temp_dir().join(format!(
        "trpg-extension-tool-audit-{}-{nonce}.jsonl",
        std::process::id()
    ));
    let audit =
        FormalCommitAudit::open(&audit_path, "extension-tool-test-v1", &[0x6d; 32]).unwrap();
    let authorizer = FormalCommitAuthorizer::new(identity_verifier, policy, audit);
    let command = common::governed_command(
        "authorize exact plugin tool invocation",
        0,
        "idem_exact_tool_authorization",
        Visibility::new(VisibilityLabel::SystemOnly),
    );
    let resource_id =
        tool_invocation_resource_id(request, provider_id, tool_id, tool_schema_version).unwrap();
    let authorization = authorizer
        .authorize_scoped_action(
            &workflow_authentication,
            None,
            &command,
            TOOL_INVOCATION_ACTION,
            TOOL_INVOCATION_RESOURCE_TYPE,
            &resource_id,
            TOOL_INVOCATION_REQUESTED_ROLE,
            4,
        )
        .unwrap();
    drop(authorizer);
    let _ = std::fs::remove_file(audit_path);
    authorization
}

#[test]
fn wasm_plugin_runs_without_wasi_or_privileged_host_imports() {
    let input = r#"{"campaign_id":"campaign_plugin"}"#;
    let module = proposal_module();
    let grants =
        ExtensionCapabilityGrantSet::with_grants(&[ExtensionCapability::EmitProposedDecision])
            .unwrap();
    let host = PluginHost::new(200_000, 4 * 1024 * 1024).unwrap();
    let plugin = host.register(manifest(&module), &module, &grants).unwrap();
    let output = host
        .invoke(
            &plugin,
            input,
            &invocation_context(input, ExtensionCapability::EmitProposedDecision),
        )
        .unwrap();
    assert_eq!(output.kind(), PluginOutputKind::Proposal);
    assert_eq!(output.visibility().label(), &VisibilityLabel::KeeperOnly);
    assert_eq!(output.fact_provenance().kind, ProvenanceKind::AgentProposal);
    assert_eq!(
        output.fact_provenance().recorded_by.as_str(),
        "coc7_wasm_plugin"
    );
    assert_eq!(output.payload()["text"], "inspect the ledger");

    let alternate_source = host
        .invoke(
            &plugin,
            input,
            &invocation_context_for_fact(
                input,
                ExtensionCapability::EmitProposedDecision,
                "plugin_source_fact_2",
            ),
        )
        .unwrap();
    assert_ne!(
        output.fact_provenance().reference,
        alternate_source.fact_provenance().reference,
        "plugin provenance must bind the complete committed source manifest"
    );

    let bound_context = invocation_context(input, ExtensionCapability::EmitProposedDecision);
    assert_eq!(
        host.invoke(
            &plugin,
            r#"{"campaign_id":"different_campaign"}"#,
            &bound_context,
        )
        .unwrap_err(),
        PluginHostError::InputBindingMismatch
    );
}

#[test]
fn imports_digest_mismatch_and_fuel_exhaustion_fail_closed() {
    let grants =
        ExtensionCapabilityGrantSet::with_grants(&[ExtensionCapability::EmitProposedDecision])
            .unwrap();
    let host = PluginHost::new(10_000, 4 * 1024 * 1024).unwrap();

    let imported = wat::parse_str(
        r#"
        (module
          (import "env" "database_write" (func $database_write))
          (memory (export "memory") 1 1)
          (func (export "trpg_plugin_alloc") (param i32) (result i32) i32.const 0)
          (func (export "trpg_plugin_invoke") (param i32 i32) (result i64) i64.const 0))
        "#,
    )
    .unwrap();
    assert_eq!(
        host.register(manifest(&imported), &imported, &grants)
            .unwrap_err(),
        PluginHostError::HostImportsForbidden
    );

    let valid = proposal_module();
    let mut wrong_digest = manifest(&valid);
    wrong_digest.module_sha256 = format!("sha256:{}", "0".repeat(64));
    assert_eq!(
        host.register(wrong_digest, &valid, &grants).unwrap_err(),
        PluginHostError::ModuleDigestMismatch
    );

    let looping = wat::parse_str(
        r#"
        (module
          (memory (export "memory") 1 1)
          (func (export "trpg_plugin_alloc") (param i32) (result i32) i32.const 0)
          (func (export "trpg_plugin_invoke") (param i32 i32) (result i64)
            (loop $forever (br $forever))
            i64.const 0))
        "#,
    )
    .unwrap();
    let plugin = host
        .register(manifest(&looping), &looping, &grants)
        .unwrap();
    assert_eq!(
        host.invoke(
            &plugin,
            "{}",
            &invocation_context("{}", ExtensionCapability::EmitProposedDecision),
        )
        .unwrap_err(),
        PluginHostError::ExecutionLimitExceeded
    );
}

#[test]
fn untrusted_plugin_cannot_self_assign_public_visibility() {
    let input = r#"{"campaign_id":"campaign_plugin"}"#;
    let module = output_module(
        r#"{"kind":"proposal","visibility_label":"public","visibility_subject":"not_applicable","provenance_kind":"agent_proposal","provenance_reference":"plugin_run_public","provenance_recorded_by":"coc7_wasm_plugin","payload":{"text":"keeper secret"}}"#,
    );
    let grants =
        ExtensionCapabilityGrantSet::with_grants(&[ExtensionCapability::EmitProposedDecision])
            .unwrap();
    let host = PluginHost::new(200_000, 4 * 1024 * 1024).unwrap();
    let plugin = host.register(manifest(&module), &module, &grants).unwrap();

    assert_eq!(
        host.invoke(
            &plugin,
            input,
            &invocation_context(input, ExtensionCapability::EmitProposedDecision),
        )
        .unwrap_err(),
        PluginHostError::OutputInvalid
    );
}

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
