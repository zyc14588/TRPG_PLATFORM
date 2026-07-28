
fn validate_manifest(manifest: &HostedPluginManifest) -> Result<(), PluginHostError> {
    if manifest.plugin_id.trim().is_empty()
        || manifest.plugin_id.len() > 128
        || !manifest
            .plugin_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        || manifest.requested_capabilities.is_empty()
        || manifest
            .requested_capabilities
            .iter()
            .enumerate()
            .any(|(index, capability)| {
                capability.is_forbidden()
                    || !matches!(
                        capability,
                        ExtensionCapability::EmitProposedDecision
                            | ExtensionCapability::InvokeGrantedTool
                            | ExtensionCapability::ReadProjection
                    )
                    || manifest.requested_capabilities[..index].contains(capability)
            })
        || !valid_sha256(&manifest.module_sha256)
    {
        Err(PluginHostError::ManifestInvalid)
    } else {
        Ok(())
    }
}

fn validate_output(
    manifest: &HostedPluginManifest,
    output: &UntrustedPluginOutput,
) -> Result<(), PluginHostError> {
    let required_capability = match output.kind {
        PluginOutputKind::Proposal => ExtensionCapability::EmitProposedDecision,
        PluginOutputKind::ToolRequest => ExtensionCapability::InvokeGrantedTool,
    };
    if !manifest
        .requested_capabilities
        .contains(&required_capability)
    {
        return Err(PluginHostError::CapabilityDenied);
    }
    if !matches!(output.payload, Value::Object(_)) {
        return Err(PluginHostError::OutputInvalid);
    }
    Ok(())
}

fn plugin_provenance_reference(
    manifest: &HostedPluginManifest,
    context: &PluginInvocationContext,
    visibility: &Visibility,
    output_kind: PluginOutputKind,
    output_bytes: &[u8],
) -> String {
    let mut digest = Sha256::new();
    update_digest_field(&mut digest, b"trpg-plugin-provenance-v2");
    for value in [
        manifest.plugin_id.as_bytes(),
        manifest.module_sha256.as_bytes(),
        context.request_id().as_str().as_bytes(),
        context.campaign_id().as_str().as_bytes(),
        context.required_capability().as_str().as_bytes(),
        context.input_sha256().as_bytes(),
        visibility.label().as_str().as_bytes(),
        visibility
            .subject_id()
            .map(EntityId::as_str)
            .unwrap_or("")
            .as_bytes(),
        match output_kind {
            PluginOutputKind::Proposal => b"proposal".as_slice(),
            PluginOutputKind::ToolRequest => b"tool_request".as_slice(),
        },
        sha256(output_bytes).as_bytes(),
    ] {
        update_digest_field(&mut digest, value);
    }
    update_digest_field(
        &mut digest,
        &u64::try_from(manifest.requested_capabilities.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for capability in &manifest.requested_capabilities {
        update_digest_field(&mut digest, capability.as_str().as_bytes());
    }
    update_digest_field(
        &mut digest,
        &u64::try_from(context.source_fact_ids().len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for fact_id in context.source_fact_ids() {
        update_digest_field(&mut digest, fact_id.as_str().as_bytes());
    }
    format!("plugin_request_{:x}", digest.finalize())
}

fn update_digest_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

fn map_execution_error(error: wasmi::Error) -> PluginHostError {
    if error.as_trap_code() == Some(TrapCode::OutOfFuel) {
        PluginHostError::ExecutionLimitExceeded
    } else {
        PluginHostError::AbiInvalid
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in hash {
        use fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("sha256:{encoded}")
}
