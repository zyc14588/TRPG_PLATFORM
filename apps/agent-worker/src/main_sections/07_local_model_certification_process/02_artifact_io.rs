fn persist_terminal_failure_if_possible(error: &str) -> Result<(), String> {
    let request_id = required_environment("TRPG_LOCAL_MODEL_CERTIFICATION_REQUEST_ID")?;
    validate_certification_identifier(&request_id)?;
    let certificate_path =
        PathBuf::from(required_environment("TRPG_LOCAL_MODEL_CERTIFICATE_PATH")?);
    let state_directory = certificate_path
        .parent()
        .ok_or_else(|| "LOCAL_MODEL_CERTIFICATION_STATE_PATH_INVALID".to_owned())?;
    ensure_private_directory(state_directory)?;
    let _process_lock = acquire_certification_process_lock(state_directory)?;
    let result_path = state_directory.join(format!("{request_id}.result.json"));
    let existing = read_process_record(&result_path)?;
    if existing.as_ref().is_some_and(|record| {
        matches!(
            record.state,
            CertificationProcessState::Succeeded | CertificationProcessState::Failed
        )
    }) {
        return Ok(());
    }
    let attempt = existing.as_ref().map_or(1, |record| record.attempt.max(1));
    let mut failed = existing.unwrap_or_else(|| CertificationProcessRecord {
        schema_version: CERTIFICATION_PROCESS_SCHEMA_VERSION,
        request_id,
        state: CertificationProcessState::Claimed,
        attempt,
        claim_owner: "local-model-certifier".to_owned(),
        model_id: std::env::var("TRPG_MODEL_ID").unwrap_or_default(),
        model_artifact_sha256: std::env::var("TRPG_MODEL_ARTIFACT_SHA256").unwrap_or_default(),
        provider_id: std::env::var("TRPG_MODEL_PROVIDER_ID").unwrap_or_default(),
        provider_type: std::env::var("TRPG_MODEL_PROVIDER_TYPE").unwrap_or_default(),
        provider_runtime_sha256: "unavailable".to_owned(),
        certificate_path: None,
        certificate_id: None,
        evidence_path: None,
        evidence_sha256: None,
        error_code: None,
    });
    failed.state = CertificationProcessState::Failed;
    failed.error_code = Some(error.to_owned());
    write_process_record(&result_path, &failed)
}

fn certification_authority_from_environment(
    secret_manager: &SecretManager<MountedFileSecretResolver>,
    registry_path: PathBuf,
) -> Result<LocalModelCertificationAuthority, String> {
    let witness_url = resolve_mounted_secret(secret_manager, "TRPG_WITNESS_DATABASE_URL")?;
    let checkpoint = witness_url
        .expose_utf8_to(PostgresLedgerCheckpointStore::connect)
        .map_err(|_| "WITNESS_DATABASE_URL_SECRET_INVALID".to_owned())?
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_WITNESS_INVALID".to_owned())?;
    let signing_key_id =
        required_environment("TRPG_LOCAL_MODEL_CERTIFICATION_HMAC_KEY_ID")?;
    let signing_key =
        resolve_mounted_secret(secret_manager, "TRPG_LOCAL_MODEL_CERTIFICATION_HMAC_KEY")?
            .to_key32()
            .map_err(|_| "LOCAL_MODEL_CERTIFICATION_HMAC_KEY_INVALID".to_owned())?;
    let mut authority = None;
    signing_key.expose_to(|key| {
        authority = Some(LocalModelCertificationAuthority::new_with_checkpoint(
            signing_key_id,
            key,
            registry_path,
            Arc::new(checkpoint),
        ));
    });
    authority
        .ok_or_else(|| "LOCAL_MODEL_CERTIFICATION_AUTHORITY_NOT_ATTEMPTED".to_owned())?
        .map_err(|error| error.code().to_owned())
}

fn recover_certificate(
    authority: &LocalModelCertificationAuthority,
    provider: &dyn ExecutableModelProvider,
    certificate_path: &Path,
    state_directory: &Path,
    record: &CertificationProcessRecord,
) -> Result<(), String> {
    let mut recovered = CertificationProcessRecord::for_provider(
        &CertificationRequestArtifact {
            request_id: record.request_id.clone(),
            model_id: record.model_id.clone(),
            model_artifact_sha256: record.model_artifact_sha256.clone(),
        },
        provider,
        CertificationProcessState::Succeeded,
        record.attempt,
    );
    populate_recovered_record(
        authority,
        provider,
        certificate_path,
        state_directory,
        &mut recovered,
    )?;
    if record.certificate_path != recovered.certificate_path
        || record.certificate_id != recovered.certificate_id
        || record.evidence_path != recovered.evidence_path
        || record.evidence_sha256 != recovered.evidence_sha256
        || record.error_code.is_some()
    {
        return Err("LOCAL_MODEL_CERTIFICATION_RESULT_INVALID".to_owned());
    }
    Ok(())
}

fn populate_recovered_record(
    authority: &LocalModelCertificationAuthority,
    provider: &dyn ExecutableModelProvider,
    certificate_path: &Path,
    state_directory: &Path,
    record: &mut CertificationProcessRecord,
) -> Result<(), String> {
    validate_regular_absolute_file(certificate_path)?;
    let certificate: LocalModelCertificate = serde_json::from_slice(
        &fs::read(certificate_path)
            .map_err(|_| "LOCAL_MODEL_CERTIFICATE_UNREADABLE".to_owned())?,
    )
    .map_err(|_| "LOCAL_MODEL_CERTIFICATE_INVALID".to_owned())?;
    authority
        .ensure_ai_keeper_provider(&certificate, provider)
        .map_err(|error| error.code().to_owned())?;
    let evidence = evidence_path(
        state_directory,
        certificate.certification_binding().evidence_sha256(),
    )?;
    validate_regular_absolute_file(&evidence)
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_EVIDENCE_MISSING".to_owned())?;
    record.certificate_path = Some(certificate_path.display().to_string());
    record.certificate_id = Some(certificate.certificate_id().to_owned());
    record.evidence_path = Some(evidence.display().to_string());
    record.evidence_sha256 = Some(
        certificate
            .certification_binding()
            .evidence_sha256()
            .to_owned(),
    );
    Ok(())
}

fn decode_certification_request(encoded: &[u8]) -> Result<CertificationRequestArtifact, String> {
    let mut remaining = encoded;
    let request_id = decode_certification_field(&mut remaining)?;
    let model_id = decode_certification_field(&mut remaining)?;
    let model_artifact_sha256 = decode_certification_field(&mut remaining)?;
    if !remaining.is_empty() {
        return Err("LOCAL_MODEL_CERTIFICATION_REQUEST_INVALID".to_owned());
    }
    validate_certification_identifier(&request_id)?;
    Ok(CertificationRequestArtifact {
        request_id,
        model_id,
        model_artifact_sha256,
    })
}

fn decode_certification_field(encoded: &mut &[u8]) -> Result<String, String> {
    let length_bytes: [u8; 8] = encoded
        .get(..8)
        .ok_or_else(|| "LOCAL_MODEL_CERTIFICATION_REQUEST_INVALID".to_owned())?
        .try_into()
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_REQUEST_INVALID".to_owned())?;
    *encoded = &encoded[8..];
    let length = usize::try_from(u64::from_be_bytes(length_bytes))
        .ok()
        .filter(|length| *length <= 256)
        .ok_or_else(|| "LOCAL_MODEL_CERTIFICATION_REQUEST_INVALID".to_owned())?;
    let value = encoded
        .get(..length)
        .ok_or_else(|| "LOCAL_MODEL_CERTIFICATION_REQUEST_INVALID".to_owned())?;
    *encoded = &encoded[length..];
    std::str::from_utf8(value)
        .ok()
        .filter(|value| !value.is_empty() && value.trim() == *value)
        .map(str::to_owned)
        .ok_or_else(|| "LOCAL_MODEL_CERTIFICATION_REQUEST_INVALID".to_owned())
}

fn validate_certification_identifier(value: &str) -> Result<(), String> {
    if !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        Ok(())
    } else {
        Err("LOCAL_MODEL_CERTIFICATION_REQUEST_ID_INVALID".to_owned())
    }
}

fn evidence_path(state_directory: &Path, evidence_sha256: &str) -> Result<PathBuf, String> {
    let digest = evidence_sha256
        .strip_prefix("sha256:")
        .filter(|digest| {
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .ok_or_else(|| "LOCAL_MODEL_CERTIFICATION_EVIDENCE_HASH_INVALID".to_owned())?;
    Ok(state_directory.join(format!("{}.evidence.json", digest.to_ascii_lowercase())))
}

fn read_process_record(path: &Path) -> Result<Option<CertificationProcessRecord>, String> {
    match fs::read(path) {
        Ok(encoded) => serde_json::from_slice(&encoded)
            .map(Some)
            .map_err(|_| "LOCAL_MODEL_CERTIFICATION_RESULT_INVALID".to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("LOCAL_MODEL_CERTIFICATION_RESULT_UNREADABLE".to_owned()),
    }
}

fn write_process_record(
    path: &Path,
    record: &CertificationProcessRecord,
) -> Result<(), String> {
    let mut encoded = serde_json::to_vec_pretty(record)
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_RESULT_SERIALIZATION_FAILED".to_owned())?;
    encoded.push(b'\n');
    write_private_atomic(path, &encoded, true)
}

fn write_private_once(path: &Path, encoded: &[u8]) -> Result<(), String> {
    if path.exists() {
        reject_symlink_if_present(path)?;
        return if fs::read(path).ok().as_deref() == Some(encoded) {
            Ok(())
        } else {
            Err("LOCAL_MODEL_CERTIFICATION_ARTIFACT_CONFLICT".to_owned())
        };
    }
    write_private_atomic(path, encoded, false)
}

fn acquire_certification_process_lock(state_directory: &Path) -> Result<fs::File, String> {
    let lock_path = state_directory.join("certification-process.lock");
    reject_symlink_if_present(&lock_path)?;
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600);
    let file = options
        .open(&lock_path)
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_PROCESS_LOCK_INVALID".to_owned())?;
    if !file
        .metadata()
        .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
    {
        return Err("LOCAL_MODEL_CERTIFICATION_PROCESS_LOCK_INVALID".to_owned());
    }
    fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600))
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_PROCESS_LOCK_INVALID".to_owned())?;
    file.lock()
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_PROCESS_LOCK_FAILED".to_owned())?;
    Ok(file)
}

fn write_private_atomic(path: &Path, encoded: &[u8], replace: bool) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("LOCAL_MODEL_CERTIFICATION_ARTIFACT_PATH_INVALID".to_owned());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "LOCAL_MODEL_CERTIFICATION_ARTIFACT_PATH_INVALID".to_owned())?;
    validate_absolute_directory(parent)?;
    reject_symlink_if_present(path)?;
    if !replace && path.exists() {
        return Err("LOCAL_MODEL_CERTIFICATION_ARTIFACT_CONFLICT".to_owned());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_CLOCK_INVALID".to_owned())?
        .as_nanos();
    let temporary = parent.join(format!(
        ".certification-write-{}-{nonce}.tmp",
        std::process::id()
    ));
    let write_result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true).mode(0o600);
        let mut file = options
            .open(&temporary)
            .map_err(|_| "LOCAL_MODEL_CERTIFICATION_ARTIFACT_WRITE_FAILED".to_owned())?;
        file.write_all(encoded)
            .and_then(|()| file.sync_all())
            .map_err(|_| "LOCAL_MODEL_CERTIFICATION_ARTIFACT_WRITE_FAILED".to_owned())?;
        fs::rename(&temporary, path)
            .map_err(|_| "LOCAL_MODEL_CERTIFICATION_ARTIFACT_COMMIT_FAILED".to_owned())?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| "LOCAL_MODEL_CERTIFICATION_ARTIFACT_PERMISSION_FAILED".to_owned())?;
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| "LOCAL_MODEL_CERTIFICATION_ARTIFACT_SYNC_FAILED".to_owned())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

fn ensure_private_directory(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("LOCAL_MODEL_CERTIFICATION_STATE_PATH_INVALID".to_owned());
    }
    fs::create_dir_all(path)
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_STATE_DIRECTORY_CREATE_FAILED".to_owned())?;
    reject_symlink_if_present(path)?;
    let metadata = fs::metadata(path)
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_STATE_DIRECTORY_INVALID".to_owned())?;
    if !metadata.is_dir() {
        return Err("LOCAL_MODEL_CERTIFICATION_STATE_DIRECTORY_INVALID".to_owned());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| "LOCAL_MODEL_CERTIFICATION_STATE_DIRECTORY_PERMISSION_FAILED".to_owned())
}

fn validate_absolute_directory(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("LOCAL_MODEL_CERTIFICATION_DIRECTORY_INVALID".to_owned());
    }
    reject_symlink_if_present(path)?;
    if fs::metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
        Ok(())
    } else {
        Err("LOCAL_MODEL_CERTIFICATION_DIRECTORY_INVALID".to_owned())
    }
}

fn reject_symlink_if_present(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err("LOCAL_MODEL_CERTIFICATION_SYMLINK_FORBIDDEN".to_owned())
        }
        Ok(_) | Err(_) if !path.exists() => Ok(()),
        Ok(_) => Ok(()),
        Err(_) => Err("LOCAL_MODEL_CERTIFICATION_PATH_INSPECTION_FAILED".to_owned()),
    }
}

#[cfg(test)]
mod local_model_certification_process_tests {
    use super::*;

    fn encoded_request(fields: &[&str]) -> Vec<u8> {
        let mut encoded = Vec::new();
        for field in fields {
            encoded.extend_from_slice(&(field.len() as u64).to_be_bytes());
            encoded.extend_from_slice(field.as_bytes());
        }
        encoded
    }

    #[test]
    fn admin_request_artifact_is_decoded_strictly() {
        let encoded = encoded_request(&[
            "bootstrap-model-certification",
            "model-exact",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ]);
        let decoded = decode_certification_request(&encoded).unwrap();
        assert_eq!(decoded.request_id, "bootstrap-model-certification");
        assert_eq!(decoded.model_id, "model-exact");
        assert!(decoded.model_artifact_sha256.starts_with("sha256:"));

        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode_certification_request(&trailing).is_err());
    }

    #[test]
    fn startup_mode_is_fail_closed() {
        assert_eq!(
            AgentWorkerStartupMode::parse(None).unwrap(),
            AgentWorkerStartupMode::Ready
        );
        assert_eq!(
            AgentWorkerStartupMode::parse(Some("certification-only")).unwrap(),
            AgentWorkerStartupMode::CertificationOnly
        );
        assert!(AgentWorkerStartupMode::parse(Some("unrestricted")).is_err());
        assert!(validate_certification_identifier("../escape").is_err());
    }
}
