
fn background_cycle_error<T>(
    delivery: &Result<
        PublishBatchResult,
        trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxError,
    >,
    projection: &Result<T, trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxError>,
    deletion: &Result<
        Vec<trpg_security_governance::security_privacy::DeletionJob>,
        trpg_security_governance::security_privacy::PrivacyError,
    >,
    export: &Result<
        CampaignExportOutcome,
        trpg_data_eventing::campaign_export_worker::CampaignExportWorkerError,
    >,
) -> Option<String> {
    let delivery_error = match delivery {
        Ok(result) if result.requires_operator_attention() => {
            let alert = result.alert_code().unwrap_or("OUTBOX_DELIVERY_ALERT");
            Some(format!(
                "{alert}:dead_lettered={}:dead_letter_total={}:failed={}:claimed={}",
                result.dead_lettered, result.dead_letter_total, result.failed, result.claimed
            ))
        }
        Ok(_) => None,
        Err(error) => Some(format!("EVENTING_DELIVERY_CYCLE_FAILED:{error}")),
    };
    let projection_error = projection
        .as_ref()
        .err()
        .map(|error| format!("PROJECTION_REBUILD_FAILED:{error}"));
    let deletion_error = deletion
        .as_ref()
        .err()
        .map(|error| format!("PRIVACY_DELETION_CYCLE_FAILED:{}", error.code()));
    let export_error = match export {
        Err(error) => Some(format!("CAMPAIGN_EXPORT_CYCLE_FAILED:{}", error.code())),
        Ok(CampaignExportOutcome::TerminalFailure {
            export_id,
            error_code,
        }) => Some(format!(
            "CAMPAIGN_EXPORT_TERMINAL_FAILURE:{export_id}:{error_code}"
        )),
        Ok(_) => None,
    };
    let errors = [
        delivery_error,
        projection_error,
        deletion_error,
        export_error,
    ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if errors.is_empty() {
        None
    } else {
        Some(errors.join(";"))
    }
}

struct BackgroundWorker {
    shutdown_sender: Sender<()>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for BackgroundWorker {
    fn drop(&mut self) {
        let _ = self.shutdown_sender.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct PluginRuntime {
    _host: PluginHost,
    plugins: Vec<HostedPlugin>,
}

impl PluginRuntime {
    fn load(registry_path: &Path) -> Result<Self, String> {
        validate_regular_absolute_file(registry_path)?;
        let document: PluginRegistryDocument = serde_json::from_slice(
            &fs::read(registry_path).map_err(|_| "PLUGIN_REGISTRY_UNREADABLE".to_owned())?,
        )
        .map_err(|_| "PLUGIN_REGISTRY_INVALID".to_owned())?;
        if document.plugins.len() > 128 {
            return Err("PLUGIN_REGISTRY_LIMIT_EXCEEDED".to_owned());
        }
        let host = PluginHost::new(document.fuel_limit, document.memory_limit_bytes)
            .map_err(|_| "PLUGIN_HOST_CONFIGURATION_INVALID".to_owned())?;
        let mut plugins = Vec::with_capacity(document.plugins.len());
        for registration in document.plugins {
            let module_path = PathBuf::from(&registration.module_path);
            validate_regular_absolute_file(&module_path)?;
            let requested_capabilities = registration
                .requested_capabilities
                .iter()
                .map(|value| parse_capability(value))
                .collect::<Result<Vec<_>, _>>()?;
            let granted_capabilities = registration
                .granted_capabilities
                .iter()
                .map(|value| parse_capability(value))
                .collect::<Result<Vec<_>, _>>()?;
            let grants = ExtensionCapabilityGrantSet::with_grants(&granted_capabilities)
                .map_err(|_| "PLUGIN_CAPABILITY_GRANT_INVALID".to_owned())?;
            let module =
                fs::read(module_path).map_err(|_| "PLUGIN_MODULE_UNREADABLE".to_owned())?;
            plugins.push(
                host.register(
                    HostedPluginManifest {
                        plugin_id: registration.plugin_id,
                        module_sha256: registration.module_sha256,
                        requested_capabilities,
                    },
                    &module,
                    &grants,
                )
                .map_err(|_| "PLUGIN_REGISTRATION_REJECTED".to_owned())?,
            );
        }
        Ok(Self {
            _host: host,
            plugins,
        })
    }

    fn check_readiness(&self) -> Result<(), String> {
        if self
            .plugins
            .iter()
            .any(|plugin| plugin.manifest().plugin_id.trim().is_empty())
        {
            Err("plugin registry integrity failure".to_owned())
        } else {
            Ok(())
        }
    }

    fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginRegistryDocument {
    fuel_limit: u64,
    memory_limit_bytes: usize,
    plugins: Vec<PluginRegistration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginRegistration {
    plugin_id: String,
    module_path: String,
    module_sha256: String,
    requested_capabilities: Vec<String>,
    granted_capabilities: Vec<String>,
}

fn parse_capability(value: &str) -> Result<ExtensionCapability, String> {
    match value {
        "invoke_granted_tool" => Ok(ExtensionCapability::InvokeGrantedTool),
        "read_projection" => Ok(ExtensionCapability::ReadProjection),
        "emit_proposed_decision" => Ok(ExtensionCapability::EmitProposedDecision),
        _ => Err("PLUGIN_CAPABILITY_FORBIDDEN".to_owned()),
    }
}

fn validate_regular_absolute_file(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("ABSOLUTE_CONFIGURATION_PATH_REQUIRED".to_owned());
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "REQUIRED_CONFIGURATION_FILE_MISSING".to_owned())?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err("REGULAR_CONFIGURATION_FILE_REQUIRED".to_owned());
    }
    Ok(())
}

fn optional_path(name: &str) -> Result<Option<PathBuf>, String> {
    let Some(value) = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    validate_regular_absolute_file(&path)?;
    Ok(Some(path))
}

fn optional_file_bytes(name: &str) -> Result<Option<Vec<u8>>, String> {
    optional_path(name)?
        .map(|path| fs::read(path).map_err(|_| format!("{name}_UNREADABLE")))
        .transpose()
}

fn required_environment(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name}_REQUIRED"))
}

fn production_secret_manager() -> Result<SecretManager<MountedFileSecretResolver>, String> {
    let resolver = MountedFileSecretResolver::new(required_environment("TRPG_SECRET_MOUNT")?)
        .map_err(|_| "SECRET_MOUNT_INVALID".to_owned())?;
    SecretManager::new_durable(resolver, required_environment("TRPG_SECRET_CATALOG_PATH")?)
        .map_err(|_| "SECRET_CATALOG_INVALID".to_owned())
}

fn resolve_mounted_secret(
    manager: &SecretManager<MountedFileSecretResolver>,
    prefix: &str,
) -> Result<SecretValue, String> {
    let id = required_environment(&format!("{prefix}_SECRET_ID"))?;
    let version = required_environment(&format!("{prefix}_SECRET_VERSION"))?
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{prefix}_SECRET_VERSION_INVALID"))?;
    let reference = SecretReference::mounted(id, version)
        .map_err(|_| format!("{prefix}_SECRET_REFERENCE_INVALID"))?;
    manager
        .register(&reference)
        .map_err(|_| format!("{prefix}_SECRET_REGISTRATION_FAILED"))?;
    manager
        .resolve(&reference)
        .map_err(|_| format!("{prefix}_SECRET_RESOLUTION_FAILED"))
}

fn boolean_environment(name: &str, default: bool) -> Result<bool, String> {
    match std::env::var(name) {
        Ok(value) => parse_boolean_environment_value(name, &value),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{name}_MUST_BE_BOOLEAN")),
    }
}

fn require_eventing_workers_enabled(enabled: bool) -> Result<(), String> {
    if enabled {
        Ok(())
    } else {
        Err("TRPG_P04_EVENTING_WORKERS_DISABLED_FAIL_CLOSED".to_owned())
    }
}

fn parse_boolean_environment_value(name: &str, value: &str) -> Result<bool, String> {
    if value.eq_ignore_ascii_case("true") || value == "1" {
        Ok(true)
    } else if value.eq_ignore_ascii_case("false") || value == "0" {
        Ok(false)
    } else {
        Err(format!("{name}_MUST_BE_BOOLEAN"))
    }
}

fn run(
    kind: ServiceKind,
    runtime: Result<RoleRuntimeProbe, trpg_contracts::ServiceError>,
    _background_worker: BackgroundWorker,
) -> ExitCode {
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            return ExitCode::FAILURE;
        }
    };
    let spec = match ServiceSpec::from_environment(kind, env!("CARGO_PKG_VERSION")) {
        Ok(spec) => spec,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            return ExitCode::FAILURE;
        }
    };
    match run_service(spec, vec![runtime]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            ExitCode::FAILURE
        }
    }
}
