impl LocalModelCertificationAuthority {
    fn with_registry_lock<T>(
        &self,
        operation: impl FnOnce(&mut Option<(u64, String)>) -> AgentResult<T>,
    ) -> AgentResult<T> {
        let mut observed_head = self
            .observed_head
            .lock()
            .map_err(|_| invalid_certification_configuration())?;
        let _file_lock = RegistryFileLock::acquire(&self.lock_file)?;
        operation(&mut observed_head)
    }

    fn read_verified_registry(
        &self,
        observed_head: &mut Option<(u64, String)>,
    ) -> AgentResult<Vec<RegistryRecord>> {
        let records = self.read_registry_records()?;
        let anchor = self.read_anchor()?;
        let checkpoint = self.read_checkpoint()?;
        ensure_registry_anchor_matches(&records, anchor.as_ref())?;
        ensure_registry_checkpoint_matches(&records, checkpoint.as_ref())?;
        ensure_registry_not_rolled_back(&records, observed_head.as_ref())?;
        *observed_head = records
            .last()
            .map(|record| (record.sequence, record.record_hash.clone()));
        Ok(records)
    }

    fn read_registry_records(&self) -> AgentResult<Vec<RegistryRecord>> {
        let encoded = read_private_file(&self.registry_path)?;
        let lines = complete_lines(&encoded)?;
        let mut records = Vec::with_capacity(lines.len());
        let mut previous_hash = REGISTRY_GENESIS_HASH.to_owned();
        for (index, line) in lines.into_iter().enumerate() {
            let record: RegistryRecord =
                serde_json::from_str(line).map_err(|_| invalid_certification_configuration())?;
            let expected_sequence = u64::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(invalid_certification_configuration)?;
            self.verify_certificate_signature(&record.certificate)
                .map_err(|_| invalid_certification_configuration())?;
            if record.schema_version != REGISTRY_SCHEMA_VERSION
                || record.sequence != expected_sequence
                || record.previous_hash != previous_hash
                || record.record_hash != self.registry_record_hash(&record)?
            {
                return Err(invalid_certification_configuration());
            }
            previous_hash = record.record_hash.clone();
            records.push(record);
        }
        Ok(records)
    }

    fn read_anchor(&self) -> AgentResult<Option<RegistryHeadAnchor>> {
        if !self.anchor_path.exists() {
            return Ok(None);
        }
        let encoded = read_private_file(&self.anchor_path)?;
        let lines = complete_lines(&encoded)?;
        if lines.len() != 1 {
            return Err(invalid_certification_configuration());
        }
        let anchor: RegistryHeadAnchor = serde_json::from_str(lines[0])
            .map_err(|_| invalid_certification_configuration())?;
        if anchor.schema_version != REGISTRY_SCHEMA_VERSION
            || anchor.sequence == 0
            || anchor.signing_key_id != self.signing_key_id
            || anchor.chain_head.trim().is_empty()
            || anchor.anchor_mac != self.registry_anchor_mac(&anchor)?
        {
            return Err(invalid_certification_configuration());
        }
        Ok(Some(anchor))
    }

    fn read_checkpoint(&self) -> AgentResult<Option<LedgerCheckpoint>> {
        let checkpoint = self
            .checkpoint_store
            .latest(&self.ledger_id)
            .map_err(AgentError::Core)?;
        if let Some(checkpoint) = checkpoint.as_ref() {
            if checkpoint.integrity_key_id() != self.signing_key_id
                || self.registry_checkpoint_mac(checkpoint)?
                    != checkpoint.checkpoint_mac()
            {
                return Err(invalid_certification_configuration());
            }
        }
        Ok(checkpoint)
    }

    fn write_anchor(&self, record: &RegistryRecord) -> AgentResult<()> {
        let mut anchor = RegistryHeadAnchor {
            schema_version: REGISTRY_SCHEMA_VERSION,
            sequence: record.sequence,
            chain_head: record.record_hash.clone(),
            signing_key_id: self.signing_key_id.clone(),
            anchor_mac: String::new(),
        };
        anchor.anchor_mac = self.registry_anchor_mac(&anchor)?;
        let mut encoded =
            serde_json::to_vec(&anchor).map_err(|_| invalid_certification_configuration())?;
        encoded.push(b'\n');
        write_private_atomic(&self.anchor_path, &encoded)
    }

    fn write_checkpoint(&self, record: &RegistryRecord) -> AgentResult<()> {
        let provisional = LedgerCheckpoint::new(
            record.sequence,
            &record.previous_hash,
            &record.record_hash,
            &self.signing_key_id,
            REGISTRY_GENESIS_HASH,
        )
        .map_err(AgentError::Core)?;
        let checkpoint = LedgerCheckpoint::new(
            record.sequence,
            &record.previous_hash,
            &record.record_hash,
            &self.signing_key_id,
            self.registry_checkpoint_mac(&provisional)?,
        )
        .map_err(AgentError::Core)?;
        self.checkpoint_store
            .append(&self.ledger_id, &checkpoint)
            .map_err(AgentError::Core)
    }
}

fn ensure_registry_anchor_matches(
    records: &[RegistryRecord],
    anchor: Option<&RegistryHeadAnchor>,
) -> AgentResult<()> {
    match (records.last(), anchor) {
        (None, None) => Ok(()),
        (Some(record), Some(anchor))
            if record.sequence == anchor.sequence && record.record_hash == anchor.chain_head =>
        {
            Ok(())
        }
        _ => Err(invalid_certification_configuration()),
    }
}

fn ensure_registry_checkpoint_matches(
    records: &[RegistryRecord],
    checkpoint: Option<&LedgerCheckpoint>,
) -> AgentResult<()> {
    match (records.last(), checkpoint) {
        (None, None) => Ok(()),
        (Some(record), Some(checkpoint))
            if record.sequence == checkpoint.sequence()
                && record.previous_hash == checkpoint.previous_chain_head()
                && record.record_hash == checkpoint.chain_head() =>
        {
            Ok(())
        }
        _ => Err(invalid_certification_configuration()),
    }
}

fn ensure_registry_not_rolled_back(
    records: &[RegistryRecord],
    observed_head: Option<&(u64, String)>,
) -> AgentResult<()> {
    let Some((sequence, record_hash)) = observed_head else {
        return Ok(());
    };
    let index = usize::try_from(sequence.saturating_sub(1))
        .map_err(|_| invalid_certification_configuration())?;
    if records
        .get(index)
        .is_none_or(|record| record.sequence != *sequence || record.record_hash != *record_hash)
    {
        return Err(invalid_certification_configuration());
    }
    Ok(())
}

struct RegistryFileLock<'a> {
    file: &'a File,
}

impl<'a> RegistryFileLock<'a> {
    fn acquire(file: &'a File) -> AgentResult<Self> {
        file.lock()
            .map_err(|_| invalid_certification_configuration())?;
        Ok(Self { file })
    }
}

impl Drop for RegistryFileLock<'_> {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

impl LocalModelCertificationAuthority {
    /// The legacy migration has no external witness target and fails closed.
    #[deprecated(note = "use migrate_previous_registry_with_checkpoint")]
    pub fn migrate_previous_registry(
        _signing_key_id: impl Into<String>,
        _signing_key: &[u8; 32],
        _registry_path: impl AsRef<Path>,
        _expected_records: u64,
        _expected_registry_sha256: &str,
    ) -> AgentResult<()> {
        Err(invalid_certification_configuration())
    }

    /// Explicitly upgrades the previous line-MAC format after the operator
    /// supplies a trusted record count and whole-file digest.
    pub fn migrate_previous_registry_with_checkpoint(
        signing_key_id: impl Into<String>,
        signing_key: &[u8; 32],
        registry_path: impl AsRef<Path>,
        expected_records: u64,
        expected_registry_sha256: &str,
        checkpoint_store: Arc<dyn LedgerCheckpointStore>,
    ) -> AgentResult<()> {
        let signing_key_id = signing_key_id.into();
        let registry_path = registry_path.as_ref();
        validate_registry_configuration(&signing_key_id, registry_path)?;
        if expected_records == 0 || !valid_sha256_label(expected_registry_sha256) {
            return Err(invalid_certification_configuration());
        }
        validate_private_file_if_present(registry_path)?;
        if !registry_path.exists() {
            return Err(invalid_certification_configuration());
        }
        let ledger_id = ledger_checkpoint_id("local-model-certification", registry_path)
            .map_err(AgentError::Core)?;
        if checkpoint_store
            .latest(&ledger_id)
            .map_err(AgentError::Core)?
            .is_some()
        {
            return Err(invalid_certification_configuration());
        }
        let anchor_path = companion_path(registry_path, ".head");
        if anchor_path.exists() {
            return Err(invalid_certification_configuration());
        }
        let authority = Self {
            signing_key_id,
            signing_key: Zeroizing::new(*signing_key),
            registry_path: registry_path.to_path_buf(),
            anchor_path,
            ledger_id,
            checkpoint_store,
            lock_file: open_or_create_private_file(&companion_path(registry_path, ".lock"))?,
            observed_head: Mutex::new(None),
        };
        authority.with_registry_lock(|observed_head| {
            if authority
                .checkpoint_store
                .latest(&authority.ledger_id)
                .map_err(AgentError::Core)?
                .is_some()
            {
                return Err(invalid_certification_configuration());
            }
            let encoded = read_private_file(&authority.registry_path)?;
            if !sha256_label(&encoded).eq_ignore_ascii_case(expected_registry_sha256) {
                return Err(invalid_certification_configuration());
            }
            let lines = complete_lines(&encoded)?;
            if u64::try_from(lines.len()).ok() != Some(expected_records) {
                return Err(invalid_certification_configuration());
            }
            let mut records = Vec::with_capacity(lines.len());
            let mut previous_hash = REGISTRY_GENESIS_HASH.to_owned();
            for (index, line) in lines.into_iter().enumerate() {
                let previous: PreviousRegistryEntry = serde_json::from_str(line)
                    .map_err(|_| invalid_certification_configuration())?;
                authority
                    .verify_certificate_signature(&previous.certificate)
                    .map_err(|_| invalid_certification_configuration())?;
                authority.verify_previous_registry_mac(&previous)?;
                let mut record = RegistryRecord {
                    schema_version: REGISTRY_SCHEMA_VERSION,
                    sequence: index as u64 + 1,
                    previous_hash,
                    source: RegistryRecordSource::PreviousFormatMigration,
                    certificate: previous.certificate,
                    state: previous.state,
                    record_hash: String::new(),
                };
                record.record_hash = authority.registry_record_hash(&record)?;
                previous_hash = record.record_hash.clone();
                records.push(record);
            }
            write_private_atomic(&authority.registry_path, &encode_registry_records(&records)?)?;
            let latest = records
                .last()
                .ok_or_else(invalid_certification_configuration)?;
            authority.write_anchor(latest)?;
            for record in &records {
                authority.write_checkpoint(record)?;
            }
            *observed_head = Some((latest.sequence, latest.record_hash.clone()));
            Ok(())
        })
    }

    /// Seeds the independent witness for the immediately previous chained
    /// registry format after operator attestation of the complete local file.
    pub fn migrate_anchored_registry_checkpoint(
        signing_key_id: impl Into<String>,
        signing_key: &[u8; 32],
        registry_path: impl AsRef<Path>,
        expected_records: u64,
        expected_registry_sha256: &str,
        checkpoint_store: Arc<dyn LedgerCheckpointStore>,
    ) -> AgentResult<()> {
        let signing_key_id = signing_key_id.into();
        let registry_path = registry_path.as_ref();
        validate_registry_configuration(&signing_key_id, registry_path)?;
        if expected_records == 0 || !valid_sha256_label(expected_registry_sha256) {
            return Err(invalid_certification_configuration());
        }
        validate_private_file_if_present(registry_path)?;
        let anchor_path = companion_path(registry_path, ".head");
        validate_private_file_if_present(&anchor_path)?;
        if !registry_path.exists() || !anchor_path.exists() {
            return Err(invalid_certification_configuration());
        }
        let ledger_id = ledger_checkpoint_id("local-model-certification", registry_path)
            .map_err(AgentError::Core)?;
        if checkpoint_store
            .latest(&ledger_id)
            .map_err(AgentError::Core)?
            .is_some()
        {
            return Err(invalid_certification_configuration());
        }
        let authority = Self {
            signing_key_id,
            signing_key: Zeroizing::new(*signing_key),
            registry_path: registry_path.to_path_buf(),
            anchor_path,
            ledger_id,
            checkpoint_store,
            lock_file: open_or_create_private_file(&companion_path(registry_path, ".lock"))?,
            observed_head: Mutex::new(None),
        };
        authority.with_registry_lock(|observed_head| {
            if authority
                .checkpoint_store
                .latest(&authority.ledger_id)
                .map_err(AgentError::Core)?
                .is_some()
            {
                return Err(invalid_certification_configuration());
            }
            let encoded = read_private_file(&authority.registry_path)?;
            if !sha256_label(&encoded).eq_ignore_ascii_case(expected_registry_sha256) {
                return Err(invalid_certification_configuration());
            }
            let records = authority.read_registry_records()?;
            if u64::try_from(records.len()).ok() != Some(expected_records) {
                return Err(invalid_certification_configuration());
            }
            let anchor = authority
                .read_anchor()?
                .ok_or_else(invalid_certification_configuration)?;
            ensure_registry_anchor_matches(&records, Some(&anchor))?;
            for record in &records {
                authority.write_checkpoint(record)?;
            }
            let latest = records
                .last()
                .ok_or_else(invalid_certification_configuration)?;
            *observed_head = Some((latest.sequence, latest.record_hash.clone()));
            Ok(())
        })
    }
}
