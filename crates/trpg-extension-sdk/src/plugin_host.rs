use std::fmt;

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use wasmi::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, TrapCode};

use crate::{ExtensionCapability, ExtensionCapabilityGrantSet};

const MAX_MODULE_BYTES: usize = 1_048_576;
const MAX_INPUT_BYTES: usize = 65_536;
const MAX_OUTPUT_BYTES: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedPluginManifest {
    pub plugin_id: String,
    pub module_sha256: String,
    pub requested_capabilities: Vec<ExtensionCapability>,
}

#[derive(Clone, Debug)]
pub struct HostedPlugin {
    manifest: HostedPluginManifest,
    module_bytes: Vec<u8>,
}

impl HostedPlugin {
    pub fn manifest(&self) -> &HostedPluginManifest {
        &self.manifest
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginOutputKind {
    Proposal,
    ToolRequest,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginOutput {
    pub kind: PluginOutputKind,
    pub visibility_label: String,
    pub visibility_subject: String,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginHostError {
    Configuration(&'static str),
    ManifestInvalid,
    ModuleTooLarge,
    ModuleDigestMismatch,
    ModuleInvalid,
    HostImportsForbidden,
    CapabilityDenied,
    AbiInvalid,
    MemoryLimitExceeded,
    ExecutionLimitExceeded,
    InputInvalid,
    OutputInvalid,
}

impl fmt::Display for PluginHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(formatter, "plugin host configuration: {reason}"),
            Self::ManifestInvalid => formatter.write_str("plugin manifest invalid"),
            Self::ModuleTooLarge => formatter.write_str("plugin module too large"),
            Self::ModuleDigestMismatch => formatter.write_str("plugin module digest mismatch"),
            Self::ModuleInvalid => formatter.write_str("plugin module invalid"),
            Self::HostImportsForbidden => formatter.write_str("plugin host imports forbidden"),
            Self::CapabilityDenied => formatter.write_str("plugin capability denied"),
            Self::AbiInvalid => formatter.write_str("plugin ABI invalid"),
            Self::MemoryLimitExceeded => formatter.write_str("plugin memory limit exceeded"),
            Self::ExecutionLimitExceeded => formatter.write_str("plugin execution limit exceeded"),
            Self::InputInvalid => formatter.write_str("plugin input invalid"),
            Self::OutputInvalid => formatter.write_str("plugin output invalid"),
        }
    }
}

impl std::error::Error for PluginHostError {}

#[derive(Clone)]
pub struct PluginHost {
    engine: Engine,
    fuel_limit: u64,
    memory_limit_bytes: usize,
}

impl fmt::Debug for PluginHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PluginHost")
            .field("engine", &"[WASMI ENGINE; NO HOST IMPORTS]")
            .field("fuel_limit", &self.fuel_limit)
            .field("memory_limit_bytes", &self.memory_limit_bytes)
            .finish()
    }
}

#[derive(Debug)]
struct HostState {
    limits: StoreLimits,
}

impl PluginHost {
    pub fn new(fuel_limit: u64, memory_limit_bytes: usize) -> Result<Self, PluginHostError> {
        if fuel_limit == 0 || !(65_536..=64 * 1024 * 1024).contains(&memory_limit_bytes) {
            return Err(PluginHostError::Configuration(
                "invalid_fuel_or_memory_limit",
            ));
        }
        let mut config = Config::default();
        config.consume_fuel(true);
        config.floats(false);
        Ok(Self {
            engine: Engine::new(&config),
            fuel_limit,
            memory_limit_bytes,
        })
    }

    pub fn register(
        &self,
        manifest: HostedPluginManifest,
        module_bytes: &[u8],
        grants: &ExtensionCapabilityGrantSet,
    ) -> Result<HostedPlugin, PluginHostError> {
        validate_manifest(&manifest)?;
        if module_bytes.len() > MAX_MODULE_BYTES {
            return Err(PluginHostError::ModuleTooLarge);
        }
        if sha256(module_bytes) != manifest.module_sha256 {
            return Err(PluginHostError::ModuleDigestMismatch);
        }
        for capability in &manifest.requested_capabilities {
            grants
                .require(*capability)
                .map_err(|_| PluginHostError::CapabilityDenied)?;
        }
        let module =
            Module::new(&self.engine, module_bytes).map_err(|_| PluginHostError::ModuleInvalid)?;
        if module.imports().next().is_some() {
            return Err(PluginHostError::HostImportsForbidden);
        }
        let export_names = module
            .exports()
            .map(|export| export.name())
            .collect::<Vec<_>>();
        for required in ["memory", "trpg_plugin_alloc", "trpg_plugin_invoke"] {
            if !export_names.contains(&required) {
                return Err(PluginHostError::AbiInvalid);
            }
        }
        Ok(HostedPlugin {
            manifest,
            module_bytes: module_bytes.to_vec(),
        })
    }

    pub fn invoke(
        &self,
        plugin: &HostedPlugin,
        input_json: &str,
    ) -> Result<PluginOutput, PluginHostError> {
        if input_json.len() > MAX_INPUT_BYTES
            || !matches!(
                serde_json::from_str::<Value>(input_json),
                Ok(Value::Object(_))
            )
        {
            return Err(PluginHostError::InputInvalid);
        }
        if sha256(&plugin.module_bytes) != plugin.manifest.module_sha256 {
            return Err(PluginHostError::ModuleDigestMismatch);
        }
        let module = Module::new(&self.engine, &plugin.module_bytes)
            .map_err(|_| PluginHostError::ModuleInvalid)?;
        if module.imports().next().is_some() {
            return Err(PluginHostError::HostImportsForbidden);
        }

        let limits = StoreLimitsBuilder::new()
            .memory_size(self.memory_limit_bytes)
            .table_elements(1_024)
            .instances(1)
            .memories(1)
            .tables(1)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(&self.engine, HostState { limits });
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(self.fuel_limit)
            .map_err(|_| PluginHostError::ExecutionLimitExceeded)?;
        let instance = Linker::<HostState>::new(&self.engine)
            .instantiate_and_start(&mut store, &module)
            .map_err(map_execution_error)?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or(PluginHostError::AbiInvalid)?;
        if memory.data_size(&store) > self.memory_limit_bytes {
            return Err(PluginHostError::MemoryLimitExceeded);
        }
        let allocate = instance
            .get_typed_func::<i32, i32>(&store, "trpg_plugin_alloc")
            .map_err(|_| PluginHostError::AbiInvalid)?;
        let invoke = instance
            .get_typed_func::<(i32, i32), i64>(&store, "trpg_plugin_invoke")
            .map_err(|_| PluginHostError::AbiInvalid)?;
        let input_length =
            i32::try_from(input_json.len()).map_err(|_| PluginHostError::InputInvalid)?;
        let input_pointer = allocate
            .call(&mut store, input_length)
            .map_err(map_execution_error)?;
        let input_offset =
            usize::try_from(input_pointer).map_err(|_| PluginHostError::AbiInvalid)?;
        memory
            .write(&mut store, input_offset, input_json.as_bytes())
            .map_err(|_| PluginHostError::MemoryLimitExceeded)?;
        let packed = invoke
            .call(&mut store, (input_pointer, input_length))
            .map_err(map_execution_error)? as u64;
        let output_offset =
            usize::try_from(packed >> 32).map_err(|_| PluginHostError::AbiInvalid)?;
        let output_length = usize::try_from(packed & u64::from(u32::MAX))
            .map_err(|_| PluginHostError::AbiInvalid)?;
        if output_length == 0 || output_length > MAX_OUTPUT_BYTES {
            return Err(PluginHostError::OutputInvalid);
        }
        let mut output_bytes = vec![0_u8; output_length];
        memory
            .read(&store, output_offset, &mut output_bytes)
            .map_err(|_| PluginHostError::OutputInvalid)?;
        let output: PluginOutput =
            serde_json::from_slice(&output_bytes).map_err(|_| PluginHostError::OutputInvalid)?;
        validate_output(&plugin.manifest, &output)?;
        Ok(output)
    }
}

fn validate_manifest(manifest: &HostedPluginManifest) -> Result<(), PluginHostError> {
    if manifest.plugin_id.trim().is_empty()
        || manifest.plugin_id.len() > 128
        || !manifest
            .plugin_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        || manifest.requested_capabilities.is_empty()
        || manifest.requested_capabilities.iter().any(|capability| {
            capability.is_forbidden()
                || !matches!(
                    capability,
                    ExtensionCapability::EmitProposedDecision
                        | ExtensionCapability::InvokeGrantedTool
                        | ExtensionCapability::ReadProjection
                )
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
    output: &PluginOutput,
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
    if !matches!(
        output.visibility_label.as_str(),
        "public"
            | "party_visible"
            | "keeper_only"
            | "private_to_player"
            | "investigator_private"
            | "ai_internal"
    ) {
        return Err(PluginHostError::OutputInvalid);
    }
    let private = matches!(
        output.visibility_label.as_str(),
        "private_to_player" | "investigator_private"
    );
    if private == (output.visibility_subject == "not_applicable")
        || output.provenance_recorded_by != manifest.plugin_id
        || output.provenance_reference.trim().is_empty()
        || !matches!(
            (output.kind, output.provenance_kind.as_str()),
            (PluginOutputKind::Proposal, "agent_proposal")
                | (PluginOutputKind::ToolRequest, "tool_result")
        )
    {
        return Err(PluginHostError::OutputInvalid);
    }
    Ok(())
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
