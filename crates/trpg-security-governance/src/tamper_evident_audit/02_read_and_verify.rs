
fn read_and_verify(
    path: &Path,
    expected_key_id: &str,
    integrity_key: &[u8; AUDIT_KEY_BYTES],
) -> KernelResult<Vec<AuditRecord>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let metadata = path
        .symlink_metadata()
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let file = File::open(path).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let mut records = Vec::new();
    let mut previous_hash = GENESIS_HASH.to_owned();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let record: AuditRecord =
            serde_json::from_str(&line).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        validate_fields(&record)?;
        if record.sequence != index as u64 + 1
            || record.integrity_key_id != expected_key_id
            || record.previous_hash != previous_hash
            || record.record_hash != hash_record(&record, integrity_key)?
        {
            return Err(TrpgError::AuditIntegrityViolation);
        }
        previous_hash = record.record_hash.clone();
        records.push(record);
    }
    Ok(records)
}

fn validate_fields(record: &AuditRecord) -> KernelResult<()> {
    let required = [
        record.actor_id.as_str(),
        record.actor_origin.as_str(),
        record.authentication_reference.as_str(),
        record.campaign_id.as_str(),
        record.resource_type.as_str(),
        record.resource_id.as_str(),
        record.action.as_str(),
        record.requested_role.as_str(),
        record.visibility_label.as_str(),
        record.visibility_subject.as_str(),
        record.provenance_kind.as_str(),
        record.provenance_reference.as_str(),
        record.provenance_recorded_by.as_str(),
        record.openfga_decision_id.as_str(),
        record.openfga_policy_revision.as_str(),
        record.opa_decision_id.as_str(),
        record.opa_policy_revision.as_str(),
        record.trace_id.as_str(),
        record.integrity_key_id.as_str(),
        record.previous_hash.as_str(),
    ];
    if record.sequence == 0
        || record.timestamp_unix_ms == 0
        || required.iter().any(|value| value.trim().is_empty())
    {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    Ok(())
}

#[derive(Serialize)]
struct AuditIntegrityPayload<'a> {
    sequence: u64,
    actor_id: &'a str,
    actor_origin: &'a str,
    authentication_reference: &'a str,
    campaign_id: &'a str,
    resource_type: &'a str,
    resource_id: &'a str,
    action: &'a str,
    requested_role: &'a str,
    visibility_label: &'a str,
    visibility_subject: &'a str,
    provenance_kind: &'a str,
    provenance_reference: &'a str,
    provenance_recorded_by: &'a str,
    decision: AuditDecision,
    openfga_decision_id: &'a str,
    openfga_policy_revision: &'a str,
    opa_decision_id: &'a str,
    opa_policy_revision: &'a str,
    timestamp_unix_ms: u64,
    trace_id: &'a str,
    integrity_key_id: &'a str,
    previous_hash: &'a str,
}

#[derive(Serialize)]
struct AuditHeadIntegrityPayload<'a> {
    sequence: u64,
    record_hash: &'a str,
    integrity_key_id: &'a str,
}

fn hash_record(
    record: &AuditRecord,
    integrity_key: &[u8; AUDIT_KEY_BYTES],
) -> KernelResult<String> {
    let payload = serde_json::to_vec(&AuditIntegrityPayload {
        sequence: record.sequence,
        actor_id: &record.actor_id,
        actor_origin: &record.actor_origin,
        authentication_reference: &record.authentication_reference,
        campaign_id: &record.campaign_id,
        resource_type: &record.resource_type,
        resource_id: &record.resource_id,
        action: &record.action,
        requested_role: &record.requested_role,
        visibility_label: &record.visibility_label,
        visibility_subject: &record.visibility_subject,
        provenance_kind: &record.provenance_kind,
        provenance_reference: &record.provenance_reference,
        provenance_recorded_by: &record.provenance_recorded_by,
        decision: record.decision,
        openfga_decision_id: &record.openfga_decision_id,
        openfga_policy_revision: &record.openfga_policy_revision,
        opa_decision_id: &record.opa_decision_id,
        opa_policy_revision: &record.opa_policy_revision,
        timestamp_unix_ms: record.timestamp_unix_ms,
        trace_id: &record.trace_id,
        integrity_key_id: &record.integrity_key_id,
        previous_hash: &record.previous_hash,
    })
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let mut mac = HmacSha256::new_from_slice(integrity_key)
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    mac.update(&payload);
    Ok(format!(
        "hmac-sha256:{}",
        hex_encode(&mac.finalize().into_bytes())
    ))
}

fn hash_anchor(
    anchor: &AuditHeadAnchor,
    integrity_key: &[u8; AUDIT_KEY_BYTES],
) -> KernelResult<String> {
    let payload = serde_json::to_vec(&AuditHeadIntegrityPayload {
        sequence: anchor.sequence,
        record_hash: &anchor.record_hash,
        integrity_key_id: &anchor.integrity_key_id,
    })
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    let mut mac = HmacSha256::new_from_slice(integrity_key)
        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    mac.update(&payload);
    Ok(format!(
        "hmac-sha256:{}",
        hex_encode(&mac.finalize().into_bytes())
    ))
}

struct AuditLock {
    path: PathBuf,
}

impl AuditLock {
    fn acquire(audit_path: &Path) -> KernelResult<Self> {
        let mut lock_name = audit_path.as_os_str().to_os_string();
        lock_name.push(".lock");
        let path = PathBuf::from(lock_name);
        for _ in 0..LOCK_RETRY_COUNT {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    writeln!(file, "{}", std::process::id())
                        .and_then(|()| file.sync_data())
                        .map_err(|_| TrpgError::AuditIntegrityViolation)?;
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return Err(TrpgError::AuditIntegrityViolation),
            }
        }
        Err(TrpgError::AuditIntegrityViolation)
    }
}

impl Drop for AuditLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn now_unix_ms() -> KernelResult<u64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TrpgError::AuditIntegrityViolation)?
        .as_millis();
    u64::try_from(millis).map_err(|_| TrpgError::AuditIntegrityViolation)
}
