fn agent_policy_and_audit_from_environment(
    secret_manager: &SecretManager<MountedFileSecretResolver>,
) -> Result<(OpenFgaOpaPolicyAdapter, FileAuditLog), String> {
    let openfga_address = required_environment("TRPG_OPENFGA_ADDRESS")?
        .parse()
        .map_err(|_| "TRPG_OPENFGA_ADDRESS_INVALID".to_owned())?;
    let openfga_store_id = required_environment_or_file(
        "TRPG_OPENFGA_STORE_ID",
        "TRPG_OPENFGA_STORE_ID_FILE",
    )?;
    let openfga_model_id = required_environment_or_file(
        "TRPG_OPENFGA_MODEL_ID",
        "TRPG_OPENFGA_MODEL_ID_FILE",
    )?;
    let opa_address = required_environment("TRPG_OPA_ADDRESS")?
        .parse()
        .map_err(|_| "TRPG_OPA_ADDRESS_INVALID".to_owned())?;
    let opa_revision = required_environment("TRPG_OPA_POLICY_REVISION")?;
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga_address,
            format!("/stores/{openfga_store_id}/check"),
            PolicyBackend::OpenFga,
            openfga_model_id,
        )
        .map_err(|_| "OPENFGA_POLICY_CONFIGURATION_INVALID".to_owned())?,
        HttpPolicyEndpoint::new(
            opa_address,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            opa_revision,
        )
        .map_err(|_| "OPA_POLICY_CONFIGURATION_INVALID".to_owned())?,
    )
    .map_err(|_| "POLICY_CONFIGURATION_INVALID".to_owned())?;
    let audit_path = required_environment("TRPG_AUDIT_LOG_PATH")?;
    let audit_key_id = required_environment("TRPG_AUDIT_HMAC_KEY_ID")?;
    let audit_key = resolve_mounted_secret(secret_manager, "TRPG_AUDIT_HMAC_KEY")?
        .to_key32()
        .map_err(|_| "AUDIT_HMAC_KEY_INVALID".to_owned())?;
    let audit = audit_key
        .expose_to(|key| FileAuditLog::open(&audit_path, &audit_key_id, key))
        .map_err(|_| "AUDIT_LOG_CONFIGURATION_INVALID".to_owned())?;
    Ok((policy, audit))
}

fn local_model_certification_from_environment(
    provider: &HttpModelProvider<MountedFileSecretResolver>,
    secret_manager: &SecretManager<MountedFileSecretResolver>,
    witness_url: &SecretValue,
) -> Result<Option<CertifiedLocalModel>, String> {
    if provider.provider_type() == ProviderType::Cloud {
        return Ok(None);
    }
    let certificate_path =
        PathBuf::from(required_environment("TRPG_LOCAL_MODEL_CERTIFICATE_PATH")?);
    let registry_path =
        PathBuf::from(required_environment("TRPG_LOCAL_MODEL_CERTIFICATION_REGISTRY_PATH")?);
    validate_regular_absolute_file(&certificate_path)?;
    validate_regular_absolute_file(&registry_path)?;
    let certificate: LocalModelCertificate = serde_json::from_slice(
        &fs::read(certificate_path)
            .map_err(|_| "LOCAL_MODEL_CERTIFICATE_UNREADABLE".to_owned())?,
    )
    .map_err(|_| "LOCAL_MODEL_CERTIFICATE_INVALID".to_owned())?;
    let signing_key_id =
        required_environment("TRPG_LOCAL_MODEL_CERTIFICATION_HMAC_KEY_ID")?;
    let signing_key =
        resolve_mounted_secret(secret_manager, "TRPG_LOCAL_MODEL_CERTIFICATION_HMAC_KEY")?
            .to_key32()
            .map_err(|_| "LOCAL_MODEL_CERTIFICATION_HMAC_KEY_INVALID".to_owned())?;
    let checkpoint = witness_url
        .expose_utf8_to(PostgresLedgerCheckpointStore::connect)
        .map_err(|_| "WITNESS_DATABASE_URL_SECRET_INVALID".to_owned())?
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_WITNESS_INVALID".to_owned())?;
    let mut authority = None;
    signing_key.expose_to(|key| {
        authority = Some(LocalModelCertificationAuthority::new_with_checkpoint(
            signing_key_id,
            key,
            registry_path,
            Arc::new(checkpoint),
        ));
    });
    let authority = authority
        .ok_or_else(|| "LOCAL_MODEL_CERTIFICATION_AUTHORITY_NOT_ATTEMPTED".to_owned())?
        .map_err(|error| error.code().to_owned())?;
    Ok(Some(CertifiedLocalModel::new(
        Arc::new(authority),
        certificate,
    )))
}

fn required_environment_or_file(name: &str, file_name: &str) -> Result<String, String> {
    if let Some(value) = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(value);
    }
    let path = PathBuf::from(required_environment(file_name)?);
    validate_regular_absolute_file(&path)?;
    fs::read_to_string(path)
        .map(|value| value.trim().to_owned())
        .map_err(|_| format!("{file_name}_UNREADABLE"))
        .and_then(|value| {
            if value.is_empty() {
                Err(format!("{name}_REQUIRED"))
            } else {
                Ok(value)
            }
        })
}

fn bounded_environment_u64(
    name: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    let value = match std::env::var(name) {
        Err(std::env::VarError::NotPresent) => default,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(format!("{name}_INVALID"));
        }
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| format!("{name}_INVALID"))?,
    };
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{name}_INVALID"));
    }
    Ok(value)
}

fn current_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(i64::MAX)
}
