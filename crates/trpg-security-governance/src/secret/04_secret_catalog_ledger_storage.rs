const SECRET_CATALOG_SCHEMA_VERSION: u32 = 1;
const SECRET_CATALOG_GENESIS_HASH: &str =
    "hmac-sha256:0000000000000000000000000000000000000000000000000000000000000000";
const PREVIOUS_SECRET_CATALOG_GENESIS_HASH: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Copy, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum SecretCatalogRecordSource {
    Native,
    PreviousFormatMigration,
    PreviousAnchoredFormatMigration,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct SecretCatalogRecord {
    schema_version: u32,
    sequence: u64,
    previous_hash: String,
    source: SecretCatalogRecordSource,
    mutation: CatalogMutation,
    record_hash: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct SecretCatalogHeadAnchor {
    schema_version: u32,
    sequence: u64,
    chain_head: String,
    anchor_hash: String,
}

#[derive(serde::Serialize)]
struct SecretCatalogIntegrityPayload<'a> {
    schema_version: u32,
    sequence: u64,
    previous_hash: &'a str,
    source: SecretCatalogRecordSource,
    mutation: &'a CatalogMutation,
}

#[derive(serde::Serialize)]
struct SecretCatalogAnchorIntegrityPayload<'a> {
    schema_version: u32,
    sequence: u64,
    chain_head: &'a str,
}

#[derive(serde::Serialize)]
struct SecretCatalogCheckpointIntegrityPayload<'a> {
    schema_version: u32,
    ledger_id: &'a str,
    sequence: u64,
    previous_chain_head: &'a str,
    chain_head: &'a str,
    integrity_key_id: &'a str,
}

fn read_verified_secret_catalog(
    path: &Path,
    anchor_path: &Path,
    ledger_id: &str,
    integrity_key_id: &str,
    integrity_key: &[u8],
    checkpoint_store: &dyn LedgerCheckpointStore,
    observed_head: &mut Option<(u64, String)>,
) -> KernelResult<(SecretCatalog, Vec<SecretCatalogRecord>)> {
    let (catalog, records) = read_secret_records(path, integrity_key)?;
    let anchor = read_secret_anchor(anchor_path)?;
    let checkpoint = checkpoint_store.latest(ledger_id)?;
    verify_secret_checkpoint(
        checkpoint.as_ref(),
        ledger_id,
        integrity_key_id,
        integrity_key,
    )?;
    ensure_secret_anchor_matches(&records, anchor.as_ref())?;
    ensure_secret_checkpoint_matches(&records, checkpoint.as_ref())?;
    ensure_secret_catalog_not_rolled_back(&records, observed_head.as_ref())?;
    *observed_head = records
        .last()
        .map(|record| (record.sequence, record.record_hash.clone()));
    Ok((catalog, records))
}

fn read_secret_records(
    path: &Path,
    integrity_key: &[u8],
) -> KernelResult<(SecretCatalog, Vec<SecretCatalogRecord>)> {
    let encoded = read_secret_file(path)?;
    let lines = complete_secret_lines(&encoded)?;
    let mut catalog = SecretCatalog::default();
    let mut records = Vec::with_capacity(lines.len());
    let mut previous_hash = SECRET_CATALOG_GENESIS_HASH.to_owned();
    for (index, line) in lines.into_iter().enumerate() {
        let record: SecretCatalogRecord =
            serde_json::from_str(line).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let expected_sequence = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(TrpgError::AuditIntegrityViolation)?;
        if record.schema_version != SECRET_CATALOG_SCHEMA_VERSION
            || record.sequence != expected_sequence
            || record.previous_hash != previous_hash
            || record.record_hash != secret_catalog_record_hash(&record, integrity_key)?
        {
            return Err(TrpgError::AuditIntegrityViolation);
        }
        record.mutation.apply(&mut catalog)?;
        previous_hash = record.record_hash.clone();
        records.push(record);
    }
    Ok((catalog, records))
}

fn read_previous_anchored_secret_records(
    path: &Path,
) -> KernelResult<(SecretCatalog, Vec<SecretCatalogRecord>)> {
    let encoded = read_secret_file(path)?;
    let lines = complete_secret_lines(&encoded)?;
    let mut catalog = SecretCatalog::default();
    let mut records = Vec::with_capacity(lines.len());
    let mut previous_hash = PREVIOUS_SECRET_CATALOG_GENESIS_HASH.to_owned();
    for (index, line) in lines.into_iter().enumerate() {
        let record: SecretCatalogRecord =
            serde_json::from_str(line).map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let expected_sequence = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(TrpgError::AuditIntegrityViolation)?;
        if record.schema_version != SECRET_CATALOG_SCHEMA_VERSION
            || record.sequence != expected_sequence
            || record.previous_hash != previous_hash
            || record.record_hash != previous_secret_catalog_record_hash(&record)?
        {
            return Err(TrpgError::AuditIntegrityViolation);
        }
        record.mutation.apply(&mut catalog)?;
        previous_hash = record.record_hash.clone();
        records.push(record);
    }
    Ok((catalog, records))
}

fn read_secret_anchor(path: &Path) -> KernelResult<Option<SecretCatalogHeadAnchor>> {
    if !path.exists() {
        return Ok(None);
    }
    let encoded = read_secret_file(path)?;
    let lines = complete_secret_lines(&encoded)?;
    if lines.len() != 1 {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    let anchor: SecretCatalogHeadAnchor =
        serde_json::from_str(lines[0]).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    if anchor.schema_version != SECRET_CATALOG_SCHEMA_VERSION
        || anchor.sequence == 0
        || anchor.chain_head.trim().is_empty()
        || anchor.anchor_hash != secret_catalog_anchor_hash(&anchor)?
    {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    Ok(Some(anchor))
}

fn write_secret_anchor(path: &Path, record: &SecretCatalogRecord) -> KernelResult<()> {
    let mut anchor = SecretCatalogHeadAnchor {
        schema_version: SECRET_CATALOG_SCHEMA_VERSION,
        sequence: record.sequence,
        chain_head: record.record_hash.clone(),
        anchor_hash: String::new(),
    };
    anchor.anchor_hash = secret_catalog_anchor_hash(&anchor)?;
    let mut encoded =
        serde_json::to_vec(&anchor).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    encoded.push(b'\n');
    write_secret_atomic(path, &encoded)
}

fn secret_catalog_record_hash(
    record: &SecretCatalogRecord,
    integrity_key: &[u8],
) -> KernelResult<String> {
    let encoded = serde_json::to_vec(&SecretCatalogIntegrityPayload {
        schema_version: record.schema_version,
        sequence: record.sequence,
        previous_hash: &record.previous_hash,
        source: record.source,
        mutation: &record.mutation,
    })
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    secret_hmac_sha256_label(integrity_key, &encoded)
}

fn previous_secret_catalog_record_hash(record: &SecretCatalogRecord) -> KernelResult<String> {
    let encoded = serde_json::to_vec(&SecretCatalogIntegrityPayload {
        schema_version: record.schema_version,
        sequence: record.sequence,
        previous_hash: &record.previous_hash,
        source: record.source,
        mutation: &record.mutation,
    })
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    Ok(secret_sha256_label(&encoded))
}

fn secret_catalog_anchor_hash(anchor: &SecretCatalogHeadAnchor) -> KernelResult<String> {
    let encoded = serde_json::to_vec(&SecretCatalogAnchorIntegrityPayload {
        schema_version: anchor.schema_version,
        sequence: anchor.sequence,
        chain_head: &anchor.chain_head,
    })
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    Ok(secret_sha256_label(&encoded))
}

fn write_secret_checkpoint(
    checkpoint_store: &dyn LedgerCheckpointStore,
    ledger_id: &str,
    integrity_key_id: &str,
    integrity_key: &[u8],
    record: &SecretCatalogRecord,
) -> KernelResult<()> {
    let provisional = LedgerCheckpoint::new(
        record.sequence,
        &record.previous_hash,
        &record.record_hash,
        integrity_key_id,
        SECRET_CATALOG_GENESIS_HASH,
    )?;
    let checkpoint = LedgerCheckpoint::new(
        record.sequence,
        &record.previous_hash,
        &record.record_hash,
        integrity_key_id,
        secret_checkpoint_mac(ledger_id, integrity_key, &provisional)?,
    )?;
    checkpoint_store.append(ledger_id, &checkpoint)
}

fn verify_secret_checkpoint(
    checkpoint: Option<&LedgerCheckpoint>,
    ledger_id: &str,
    integrity_key_id: &str,
    integrity_key: &[u8],
) -> KernelResult<()> {
    if let Some(checkpoint) = checkpoint {
        if checkpoint.integrity_key_id() != integrity_key_id
            || checkpoint.checkpoint_mac()
                != secret_checkpoint_mac(ledger_id, integrity_key, checkpoint)?
        {
            return Err(TrpgError::AuditIntegrityViolation);
        }
    }
    Ok(())
}

fn secret_checkpoint_mac(
    ledger_id: &str,
    integrity_key: &[u8],
    checkpoint: &LedgerCheckpoint,
) -> KernelResult<String> {
    let encoded = serde_json::to_vec(&SecretCatalogCheckpointIntegrityPayload {
        schema_version: SECRET_CATALOG_SCHEMA_VERSION,
        ledger_id,
        sequence: checkpoint.sequence(),
        previous_chain_head: checkpoint.previous_chain_head(),
        chain_head: checkpoint.chain_head(),
        integrity_key_id: checkpoint.integrity_key_id(),
    })
    .map_err(|_| TrpgError::AuditIntegrityViolation)?;
    secret_hmac_sha256_label(integrity_key, &encoded)
}

fn ensure_secret_anchor_matches(
    records: &[SecretCatalogRecord],
    anchor: Option<&SecretCatalogHeadAnchor>,
) -> KernelResult<()> {
    match (records.last(), anchor) {
        (None, None) => Ok(()),
        (Some(record), Some(anchor))
            if record.sequence == anchor.sequence && record.record_hash == anchor.chain_head =>
        {
            Ok(())
        }
        _ => Err(TrpgError::AuditIntegrityViolation),
    }
}

fn ensure_secret_checkpoint_matches(
    records: &[SecretCatalogRecord],
    checkpoint: Option<&LedgerCheckpoint>,
) -> KernelResult<()> {
    match (records.last(), checkpoint) {
        (None, None) => Ok(()),
        (Some(record), Some(checkpoint))
            if record.sequence == checkpoint.sequence()
                && record.previous_hash == checkpoint.previous_chain_head()
                && record.record_hash == checkpoint.chain_head() =>
        {
            Ok(())
        }
        _ => Err(TrpgError::AuditIntegrityViolation),
    }
}

fn ensure_secret_catalog_not_rolled_back(
    records: &[SecretCatalogRecord],
    observed_head: Option<&(u64, String)>,
) -> KernelResult<()> {
    let Some((sequence, record_hash)) = observed_head else {
        return Ok(());
    };
    let index =
        usize::try_from(sequence.saturating_sub(1)).map_err(|_| TrpgError::AuditIntegrityViolation)?;
    if records
        .get(index)
        .is_none_or(|record| record.sequence != *sequence || record.record_hash != *record_hash)
    {
        return Err(TrpgError::AuditIntegrityViolation);
    }
    Ok(())
}
