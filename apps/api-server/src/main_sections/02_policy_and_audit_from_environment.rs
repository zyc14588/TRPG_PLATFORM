
fn policy_and_audit_from_environment(
    secret_manager: &SecretManager<MountedFileSecretResolver>,
) -> Result<(OpenFgaOpaPolicyAdapter, FileAuditLog), &'static str> {
    let openfga_address = required_environment("TRPG_OPENFGA_ADDRESS")?
        .parse()
        .map_err(|_| "TRPG_OPENFGA_ADDRESS_INVALID")?;
    let openfga_store_id =
        required_environment_or_file("TRPG_OPENFGA_STORE_ID", "TRPG_OPENFGA_STORE_ID_FILE")?;
    let openfga_model_id =
        required_environment_or_file("TRPG_OPENFGA_MODEL_ID", "TRPG_OPENFGA_MODEL_ID_FILE")?;
    let opa_address = required_environment("TRPG_OPA_ADDRESS")?
        .parse()
        .map_err(|_| "TRPG_OPA_ADDRESS_INVALID")?;
    let opa_revision = required_environment("TRPG_OPA_POLICY_REVISION")?;
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga_address,
            format!("/stores/{openfga_store_id}/check"),
            PolicyBackend::OpenFga,
            openfga_model_id,
        )
        .map_err(|_| "OPENFGA_POLICY_CONFIGURATION_INVALID")?,
        HttpPolicyEndpoint::new(
            opa_address,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            opa_revision,
        )
        .map_err(|_| "OPA_POLICY_CONFIGURATION_INVALID")?,
    )
    .map_err(|_| "POLICY_CONFIGURATION_INVALID")?;

    let audit_path = required_environment("TRPG_AUDIT_LOG_PATH")?;
    let audit_key_id = required_environment("TRPG_AUDIT_HMAC_KEY_ID")?;
    let audit_secret = resolve_mounted_secret(secret_manager, "TRPG_AUDIT_HMAC_KEY")
        .map_err(|_| "AUDIT_HMAC_KEY_RESOLUTION_FAILED")?;
    let audit_key = audit_secret
        .to_key32()
        .map_err(|_| "AUDIT_HMAC_KEY_INVALID")?;
    let audit = audit_key
        .expose_to(|key| FileAuditLog::open(audit_path, audit_key_id, key))
        .map_err(|_| "AUDIT_LOG_CONFIGURATION_INVALID")?;
    Ok((policy, audit))
}

fn production_secret_manager() -> Result<SecretManager<MountedFileSecretResolver>, String> {
    let mount = required_environment("TRPG_SECRET_MOUNT")?;
    let catalog = required_environment("TRPG_SECRET_CATALOG_PATH")?;
    let resolver =
        MountedFileSecretResolver::new(mount).map_err(|_| "SECRET_MOUNT_INVALID".to_owned())?;
    SecretManager::new_durable(resolver, catalog).map_err(|_| "SECRET_CATALOG_INVALID".to_owned())
}

fn resolve_mounted_secret(
    manager: &SecretManager<MountedFileSecretResolver>,
    prefix: &str,
) -> Result<SecretValue, String> {
    let secret_id = required_environment(&format!("{prefix}_SECRET_ID"))?;
    let version = required_environment(&format!("{prefix}_SECRET_VERSION"))?
        .parse::<u64>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or_else(|| format!("{prefix}_SECRET_VERSION_INVALID"))?;
    let reference = SecretReference::mounted(secret_id, version)
        .map_err(|_| format!("{prefix}_SECRET_REFERENCE_INVALID"))?;
    manager
        .register(&reference)
        .map_err(|_| format!("{prefix}_SECRET_REGISTRATION_FAILED"))?;
    manager
        .resolve(&reference)
        .map_err(|_| format!("{prefix}_SECRET_RESOLUTION_FAILED"))
}

fn required_environment(name: &str) -> Result<String, &'static str> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or("REQUIRED_ENVIRONMENT_MISSING")
}

fn player_action_writes_enabled(value: Option<&str>) -> Result<bool, &'static str> {
    match value {
        None | Some("1" | "true") => Ok(true),
        Some("0" | "false") => Ok(false),
        Some(_) => Err("PLAYER_ACTION_WRITES_FLAG_INVALID"),
    }
}

fn required_environment_or_file(name: &str, file_name: &str) -> Result<String, &'static str> {
    if let Some(value) = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(value);
    }
    let path = required_environment(file_name)?;
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or("REQUIRED_ENVIRONMENT_FILE_INVALID")
}

fn run(
    kind: ServiceKind,
    runtime: Result<RoleRuntimeProbe, trpg_contracts::ServiceError>,
    application: ApiApplication,
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
    match run_service_with_handler(
        spec,
        vec![runtime],
        Box::new(move |request| application.handle(request)),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("service={} error={}", kind.as_str(), error.code);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::player_action_writes_enabled;

    #[test]
    fn player_action_write_flag_defaults_on_and_fails_closed_on_invalid_values() {
        assert_eq!(player_action_writes_enabled(None), Ok(true));
        assert_eq!(player_action_writes_enabled(Some("1")), Ok(true));
        assert_eq!(player_action_writes_enabled(Some("true")), Ok(true));
        assert_eq!(player_action_writes_enabled(Some("0")), Ok(false));
        assert_eq!(player_action_writes_enabled(Some("false")), Ok(false));
        assert_eq!(
            player_action_writes_enabled(Some("TRUE")),
            Err("PLAYER_ACTION_WRITES_FLAG_INVALID")
        );
    }
}
