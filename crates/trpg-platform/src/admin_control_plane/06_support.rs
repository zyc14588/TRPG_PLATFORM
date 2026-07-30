fn required_environment(name: &'static str) -> Result<String, AdminControlPlaneError> {
    optional_environment(name)
        .ok_or(AdminControlPlaneError::Configuration("ADMIN_REQUIRED_ENVIRONMENT_MISSING"))
}

fn optional_environment(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn bounded_environment_u64(
    name: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, AdminControlPlaneError> {
    let value = optional_environment(name)
        .map(|value| value.parse::<u64>())
        .transpose()
        .map_err(|_| AdminControlPlaneError::Configuration("ADMIN_LIMIT_INVALID"))?
        .unwrap_or(default);
    if !(minimum..=maximum).contains(&value) {
        return Err(AdminControlPlaneError::Configuration(
            "ADMIN_LIMIT_INVALID",
        ));
    }
    Ok(value)
}

fn optional_regular_file(name: &str) -> Result<Option<Vec<u8>>, AdminControlPlaneError> {
    let Some(path) = optional_environment(name) else {
        return Ok(None);
    };
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| AdminControlPlaneError::Configuration("ADMIN_TLS_FILE_UNAVAILABLE"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AdminControlPlaneError::Configuration(
            "ADMIN_TLS_REGULAR_FILE_REQUIRED",
        ));
    }
    let bytes = fs::read(path)
        .map_err(|_| AdminControlPlaneError::Configuration("ADMIN_TLS_FILE_UNAVAILABLE"))?;
    if bytes.is_empty() || bytes.len() > 1024 * 1024 {
        return Err(AdminControlPlaneError::Configuration(
            "ADMIN_TLS_FILE_INVALID",
        ));
    }
    Ok(Some(bytes))
}

fn resolve_mounted_secret(
    manager: &SecretManager<MountedFileSecretResolver>,
    prefix: &'static str,
) -> Result<SecretValue, AdminControlPlaneError> {
    let id_name = format!("{prefix}_SECRET_ID");
    let version_name = format!("{prefix}_SECRET_VERSION");
    let id = optional_environment(&id_name).ok_or(AdminControlPlaneError::Configuration(
        "ADMIN_SECRET_REFERENCE_MISSING",
    ))?;
    let version = optional_environment(&version_name)
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .ok_or(AdminControlPlaneError::Configuration(
            "ADMIN_SECRET_VERSION_INVALID",
        ))?;
    let reference = SecretReference::mounted(id, version).map_err(|_| {
        AdminControlPlaneError::Configuration("ADMIN_SECRET_REFERENCE_INVALID")
    })?;
    manager.register(&reference).map_err(|_| {
        AdminControlPlaneError::Configuration("ADMIN_SECRET_REGISTRATION_FAILED")
    })?;
    manager.resolve(&reference).map_err(|_| {
        AdminControlPlaneError::Configuration("ADMIN_SECRET_RESOLUTION_FAILED")
    })
}

fn validate_bootstrap_token(token: &SecretValue) -> Result<(), AdminControlPlaneError> {
    let valid = token.expose_to(|bytes| {
        (32..=4096).contains(&bytes.len())
            && bytes
                .iter()
                .all(|byte| byte.is_ascii_graphic() && !byte.is_ascii_whitespace())
    });
    if !valid {
        return Err(AdminControlPlaneError::Configuration(
            "ADMIN_BOOTSTRAP_TOKEN_INVALID",
        ));
    }
    Ok(())
}

fn prepare_private_parent(path: &Path) -> Result<(), AdminControlPlaneError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or(AdminControlPlaneError::Configuration(
            "ADMIN_PRIVATE_PATH_INVALID",
        ))?;
    fs::create_dir_all(parent)
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_PRIVATE_DIRECTORY_FAILED"))?;
    let metadata = fs::symlink_metadata(parent)
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_PRIVATE_DIRECTORY_FAILED"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(AdminControlPlaneError::Persistence(
            "ADMIN_PRIVATE_DIRECTORY_INVALID",
        ));
    }
    set_private_permissions(parent, 0o700)?;
    if path.exists() {
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_PRIVATE_FILE_INVALID"))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(AdminControlPlaneError::Persistence(
                "ADMIN_PRIVATE_FILE_INVALID",
            ));
        }
        set_private_permissions(path, 0o600)?;
    }
    Ok(())
}

fn set_private_permissions(path: &Path, mode: u32) -> Result<(), AdminControlPlaneError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|_| {
            AdminControlPlaneError::Persistence("ADMIN_PRIVATE_PERMISSIONS_FAILED")
        })
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Err(AdminControlPlaneError::Configuration(
            "ADMIN_UNIX_RUNTIME_REQUIRED",
        ))
    }
}

fn sync_parent(path: &Path) -> Result<(), AdminControlPlaneError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_STATE_DIRECTORY_SYNC_FAILED"))
}

fn now_unix_ms() -> Result<u64, AdminControlPlaneError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_CLOCK_INVALID"))?
        .as_millis();
    u64::try_from(millis)
        .map_err(|_| AdminControlPlaneError::Persistence("ADMIN_CLOCK_INVALID"))
}

fn ensure_expected_version(
    state: &AdminState,
    metadata: &MutationMetadata,
) -> Result<(), AdminControlPlaneError> {
    if state.version != metadata.expected_version {
        return Err(AdminControlPlaneError::Conflict(
            "ADMIN_EXPECTED_VERSION_CONFLICT",
        ));
    }
    Ok(())
}

fn insert_receipt(
    state: &mut AdminState,
    metadata: &MutationMetadata,
    action: &str,
    descriptor: String,
    result_code: &str,
    resource_id: &str,
    response_fields: BTreeMap<String, Value>,
) -> Result<(), AdminControlPlaneError> {
    if state.receipts.len() >= MAX_RECEIPTS {
        return Err(AdminControlPlaneError::Persistence(
            "ADMIN_RECEIPT_CAPACITY_EXCEEDED",
        ));
    }
    state.receipts.insert(
        metadata.idempotency_key.clone(),
        AdminReceipt {
            action: action.to_owned(),
            descriptor,
            result_code: result_code.to_owned(),
            resource_id: resource_id.to_owned(),
            response_fields,
        },
    );
    Ok(())
}

fn commit_receipt(
    state: &mut AdminState,
    metadata: &MutationMetadata,
    action: &str,
    descriptor: String,
    result_code: &str,
    resource_id: &str,
    response_fields: BTreeMap<String, Value>,
) -> Result<(), AdminControlPlaneError> {
    state.version = state
        .version
        .checked_add(1)
        .ok_or(AdminControlPlaneError::Persistence(
            "ADMIN_STATE_VERSION_OVERFLOW",
        ))?;
    insert_receipt(
        state,
        metadata,
        action,
        descriptor,
        result_code,
        resource_id,
        response_fields,
    )
}

fn replay_response(
    state: &AdminState,
    metadata: &MutationMetadata,
    action: &str,
    descriptor: &str,
) -> Result<Option<AdminHttpResponse>, AdminControlPlaneError> {
    let Some(receipt) = state.receipts.get(&metadata.idempotency_key) else {
        return Ok(None);
    };
    if receipt.action != action || receipt.descriptor != descriptor {
        return Err(AdminControlPlaneError::Conflict(
            "ADMIN_IDEMPOTENCY_KEY_REUSED",
        ));
    }
    let mut body = json!({
            "result": receipt.result_code,
            "resource_id": receipt.resource_id,
            "replayed": true,
            "state_version": state.version
        });
    if let Some(body) = body.as_object_mut() {
        for (key, value) in &receipt.response_fields {
            body.insert(key.clone(), value.clone());
        }
    }
    Ok(Some(AdminHttpResponse { status: 200, body }))
}

fn evidence_response_fields(evidence: &AdminOperationEvidence) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "artifact_reference".to_owned(),
            json!(evidence.artifact_reference),
        ),
        ("digest".to_owned(), json!(evidence.digest)),
    ])
}

fn ensure_empty_body(request: &AdminHttpRequest) -> Result<(), AdminControlPlaneError> {
    if !request.body.is_empty() {
        return Err(AdminControlPlaneError::InvalidRequest(
            "ADMIN_REQUEST_BODY_MUST_BE_EMPTY",
        ));
    }
    Ok(())
}

fn validate_short_identifier(
    value: &str,
    code: &'static str,
) -> Result<(), AdminControlPlaneError> {
    if value.trim().is_empty()
        || value.len() > 256
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Err(AdminControlPlaneError::InvalidRequest(code));
    }
    Ok(())
}

fn validate_restore_request(
    request: &AdminRestoreRequest,
) -> Result<(), AdminControlPlaneError> {
    validate_short_identifier(
        &request.expected_schema_version,
        "ADMIN_RESTORE_REQUEST_INVALID",
    )?;
    validate_short_identifier(
        &request.safety_point_id,
        "ADMIN_RESTORE_REQUEST_INVALID",
    )?;
    let path = Path::new(&request.manifest_path);
    if !path.is_absolute() || request.manifest_path.len() > 4096 {
        return Err(AdminControlPlaneError::InvalidRequest(
            "ADMIN_RESTORE_MANIFEST_PATH_INVALID",
        ));
    }
    Ok(())
}

pub fn provider_probe_confirms_model(body: &[u8], expected_model_id: &str) -> bool {
    if body.is_empty() || body.len() > 1024 * 1024 || expected_model_id.trim().is_empty() {
        return false;
    }
    let Ok(document) = serde_json::from_slice::<Value>(body) else {
        return false;
    };
    let openai_match = document
        .get("data")
        .and_then(Value::as_array)
        .is_some_and(|models| {
            models.iter().any(|model| {
                model.get("id").and_then(Value::as_str) == Some(expected_model_id)
            })
        });
    let local_match = document
        .get("models")
        .and_then(Value::as_array)
        .is_some_and(|models| {
            models.iter().any(|model| {
                ["name", "model"].iter().any(|field| {
                    model.get(field).and_then(Value::as_str) == Some(expected_model_id)
                })
            })
        });
    openai_match || local_match
}
