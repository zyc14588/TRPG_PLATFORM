impl LocalModelCertificationAuthority {
    pub fn new(
        signing_key_id: impl Into<String>,
        signing_key: &[u8; 32],
        registry_path: impl AsRef<Path>,
    ) -> AgentResult<Self> {
        let signing_key_id = signing_key_id.into();
        let registry_path = registry_path.as_ref();
        validate_registry_configuration(&signing_key_id, registry_path)?;
        let registry_file = open_or_create_private_file(registry_path)?;
        registry_file
            .sync_all()
            .map_err(|_| invalid_certification_configuration())?;
        sync_parent(registry_path)?;

        let anchor_path = companion_path(registry_path, ".head");
        validate_private_file_if_present(&anchor_path)?;
        let lock_path = companion_path(registry_path, ".lock");
        let lock_file = open_or_create_private_file(&lock_path)?;
        let authority = Self {
            signing_key_id,
            signing_key: Zeroizing::new(*signing_key),
            registry_path: registry_path.to_path_buf(),
            anchor_path,
            lock_file,
            observed_head: Mutex::new(None),
        };
        authority.validate_registry()?;
        Ok(authority)
    }

    /// Explicitly upgrades the previous line-MAC format after the operator
    /// supplies a trusted record count and whole-file digest. The migration is
    /// one-time: an existing high-water anchor always causes rejection.
    pub fn migrate_previous_registry(
        signing_key_id: impl Into<String>,
        signing_key: &[u8; 32],
        registry_path: impl AsRef<Path>,
        expected_records: u64,
        expected_registry_sha256: &str,
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
        let anchor_path = companion_path(registry_path, ".head");
        if anchor_path.exists() {
            return Err(invalid_certification_configuration());
        }
        let lock_path = companion_path(registry_path, ".lock");
        let authority = Self {
            signing_key_id,
            signing_key: Zeroizing::new(*signing_key),
            registry_path: registry_path.to_path_buf(),
            anchor_path,
            lock_file: open_or_create_private_file(&lock_path)?,
            observed_head: Mutex::new(None),
        };
        authority.with_registry_lock(|observed_head| {
            if authority.anchor_path.exists() {
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
            *observed_head = Some((latest.sequence, latest.record_hash.clone()));
            Ok(())
        })
    }

    pub fn issue_level4(
        &self,
        input: &CertificationInput,
        model_artifact_sha256: &str,
        suite_id: &str,
        ttl: Duration,
    ) -> AgentResult<LocalModelCertificate> {
        if certify_local_model(input) != LocalModelLevel::Level4
            || !valid_sha256(model_artifact_sha256)
            || !valid_identifier(suite_id)
            || ttl.is_zero()
            || ttl > MAX_CERTIFICATE_TTL
        {
            return Err(AgentError::LocalModelNotCertifiedForAiKp);
        }
        let issued_at_unix_ms = trusted_now_unix_ms()?;
        let expires_at_unix_ms = issued_at_unix_ms
            .checked_add(
                u64::try_from(ttl.as_millis())
                    .map_err(|_| invalid_certification_configuration())?,
            )
            .ok_or_else(invalid_certification_configuration)?;
        let mut certificate = LocalModelCertificate {
            certificate_id: certificate_id(
                &input.model_id,
                model_artifact_sha256,
                suite_id,
                issued_at_unix_ms,
            ),
            model_id: input.model_id.clone(),
            model_artifact_sha256: model_artifact_sha256.to_owned(),
            suite_id: suite_id.to_owned(),
            level: LocalModelLevel::Level4,
            issued_at_unix_ms,
            expires_at_unix_ms,
            signing_key_id: self.signing_key_id.clone(),
            signature: String::new(),
        };
        certificate.signature = self.certificate_signature(&certificate)?;
        self.append_registry_entry(&certificate, RegistryState::Active)?;
        Ok(certificate)
    }

    pub fn revoke(&self, certificate: &LocalModelCertificate) -> AgentResult<()> {
        self.verify_certificate_signature(certificate)?;
        self.append_registry_entry(certificate, RegistryState::Revoked)
    }

    pub fn ensure_ai_keeper_model(
        &self,
        certificate: &LocalModelCertificate,
        expected_model_id: &str,
        expected_artifact_sha256: &str,
    ) -> AgentResult<()> {
        self.verify_certificate_signature(certificate)?;
        if certificate.level != LocalModelLevel::Level4
            || certificate.model_id != expected_model_id
            || certificate.model_artifact_sha256 != expected_artifact_sha256
            || trusted_now_unix_ms()? >= certificate.expires_at_unix_ms
            || self.latest_registry_state(certificate)? != Some(RegistryState::Active)
        {
            return Err(AgentError::LocalModelNotCertifiedForAiKp);
        }
        Ok(())
    }

    fn validate_registry(&self) -> AgentResult<()> {
        self.with_registry_lock(|observed_head| {
            self.read_verified_registry(observed_head).map(|_| ())
        })
    }

    fn latest_registry_state(
        &self,
        certificate: &LocalModelCertificate,
    ) -> AgentResult<Option<RegistryState>> {
        self.with_registry_lock(|observed_head| {
            let records = self.read_verified_registry(observed_head)?;
            Ok(records
                .iter()
                .rev()
                .find(|record| {
                    record.certificate.certificate_id == certificate.certificate_id
                        && record.certificate.signature == certificate.signature
                })
                .map(|record| record.state))
        })
    }

    fn append_registry_entry(
        &self,
        certificate: &LocalModelCertificate,
        state: RegistryState,
    ) -> AgentResult<()> {
        self.with_registry_lock(|observed_head| {
            let records = self.read_verified_registry(observed_head)?;
            let sequence = records.last().map_or(Ok(1), |entry| {
                entry
                    .sequence
                    .checked_add(1)
                    .ok_or_else(invalid_certification_configuration)
            })?;
            let mut record = RegistryRecord {
                schema_version: REGISTRY_SCHEMA_VERSION,
                sequence,
                previous_hash: records.last().map_or_else(
                    || REGISTRY_GENESIS_HASH.to_owned(),
                    |entry| entry.record_hash.clone(),
                ),
                source: RegistryRecordSource::Native,
                certificate: certificate.clone(),
                state,
                record_hash: String::new(),
            };
            record.record_hash = self.registry_record_hash(&record)?;
            let mut encoded =
                serde_json::to_vec(&record).map_err(|_| invalid_certification_configuration())?;
            encoded.push(b'\n');
            let mut file = open_private_append(&self.registry_path)?;
            file.write_all(&encoded)
                .and_then(|()| file.sync_all())
                .map_err(|_| invalid_certification_configuration())?;
            self.write_anchor(&record)?;
            *observed_head = Some((record.sequence, record.record_hash));
            Ok(())
        })
    }

    fn certificate_signature(&self, certificate: &LocalModelCertificate) -> AgentResult<String> {
        let mut mac = HmacSha256::new_from_slice(self.signing_key.as_slice())
            .map_err(|_| invalid_certification_configuration())?;
        for field in [
            certificate.certificate_id.as_str(),
            certificate.model_id.as_str(),
            certificate.model_artifact_sha256.as_str(),
            certificate.suite_id.as_str(),
            certificate.level.as_str(),
            &certificate.issued_at_unix_ms.to_string(),
            &certificate.expires_at_unix_ms.to_string(),
            certificate.signing_key_id.as_str(),
        ] {
            mac.update(&(field.len() as u64).to_be_bytes());
            mac.update(field.as_bytes());
        }
        Ok(format!("hmac-sha256:{:x}", mac.finalize().into_bytes()))
    }

    fn verify_certificate_signature(&self, certificate: &LocalModelCertificate) -> AgentResult<()> {
        if certificate.signing_key_id != self.signing_key_id
            || !valid_sha256(&certificate.model_artifact_sha256)
            || certificate.signature != self.certificate_signature(certificate)?
        {
            return Err(AgentError::LocalModelNotCertifiedForAiKp);
        }
        Ok(())
    }

    fn registry_record_hash(&self, record: &RegistryRecord) -> AgentResult<String> {
        let payload = serde_json::to_vec(&RegistryIntegrityPayload {
            schema_version: record.schema_version,
            sequence: record.sequence,
            previous_hash: &record.previous_hash,
            source: record.source,
            certificate: &record.certificate,
            state: record.state,
        })
        .map_err(|_| invalid_certification_configuration())?;
        self.hmac_label(&payload)
    }

    fn registry_anchor_mac(&self, anchor: &RegistryHeadAnchor) -> AgentResult<String> {
        let payload = serde_json::to_vec(&RegistryAnchorIntegrityPayload {
            schema_version: anchor.schema_version,
            sequence: anchor.sequence,
            chain_head: &anchor.chain_head,
            signing_key_id: &anchor.signing_key_id,
        })
        .map_err(|_| invalid_certification_configuration())?;
        self.hmac_label(&payload)
    }

    fn hmac_label(&self, payload: &[u8]) -> AgentResult<String> {
        let mut mac = HmacSha256::new_from_slice(self.signing_key.as_slice())
            .map_err(|_| invalid_certification_configuration())?;
        mac.update(payload);
        Ok(format!("hmac-sha256:{:x}", mac.finalize().into_bytes()))
    }

    fn verify_previous_registry_mac(&self, entry: &PreviousRegistryEntry) -> AgentResult<()> {
        let mut mac = HmacSha256::new_from_slice(self.signing_key.as_slice())
            .map_err(|_| invalid_certification_configuration())?;
        for field in [
            entry.certificate.certificate_id.as_str(),
            entry.certificate.signature.as_str(),
            match entry.state {
                RegistryState::Active => "active",
                RegistryState::Revoked => "revoked",
            },
        ] {
            mac.update(&(field.len() as u64).to_be_bytes());
            mac.update(field.as_bytes());
        }
        let expected = format!("hmac-sha256:{:x}", mac.finalize().into_bytes());
        if entry.registry_mac != expected {
            return Err(invalid_certification_configuration());
        }
        Ok(())
    }
}

pub fn ensure_ai_keeper_model(
    authority: &LocalModelCertificationAuthority,
    certificate: &LocalModelCertificate,
    expected_model_id: &str,
    expected_artifact_sha256: &str,
) -> AgentResult<()> {
    authority.ensure_ai_keeper_model(certificate, expected_model_id, expected_artifact_sha256)
}

fn certificate_id(model_id: &str, artifact: &str, suite: &str, issued_at: u64) -> String {
    let mut digest = Sha256::new();
    for field in [model_id, artifact, suite, &issued_at.to_string()] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
    format!("local_model_certificate_{:x}", digest.finalize())
}

fn trusted_now_unix_ms() -> AgentResult<u64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid_certification_configuration())?;
    u64::try_from(elapsed.as_millis()).map_err(|_| invalid_certification_configuration())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn invalid_certification_configuration() -> AgentError {
    AgentError::Core(trpg_shared_kernel::TrpgError::InvalidConfiguration(
        "local_model_certification_registry_invalid",
    ))
}
