use sha2::{Digest, Sha256};
use trpg_extension_sdk::plugin_host::{
    HostedPluginManifest, PluginHost, PluginHostError, PluginOutputKind,
};
use trpg_extension_sdk::{ExtensionCapability, ExtensionCapabilityGrantSet};

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
    let output = r#"{"kind":"proposal","visibility_label":"keeper_only","visibility_subject":"not_applicable","provenance_kind":"agent_proposal","provenance_reference":"plugin_run_1","provenance_recorded_by":"coc7_wasm_plugin","payload":{"text":"inspect the ledger"}}"#;
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

#[test]
fn wasm_plugin_runs_without_wasi_or_privileged_host_imports() {
    let module = proposal_module();
    let grants =
        ExtensionCapabilityGrantSet::with_grants(&[ExtensionCapability::EmitProposedDecision])
            .unwrap();
    let host = PluginHost::new(200_000, 4 * 1024 * 1024).unwrap();
    let plugin = host.register(manifest(&module), &module, &grants).unwrap();
    let output = host
        .invoke(&plugin, r#"{"campaign_id":"campaign_plugin"}"#)
        .unwrap();
    assert_eq!(output.kind, PluginOutputKind::Proposal);
    assert_eq!(output.visibility_label, "keeper_only");
    assert_eq!(output.provenance_kind, "agent_proposal");
    assert_eq!(output.payload["text"], "inspect the ledger");
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
        host.invoke(&plugin, "{}").unwrap_err(),
        PluginHostError::ExecutionLimitExceeded
    );
}
