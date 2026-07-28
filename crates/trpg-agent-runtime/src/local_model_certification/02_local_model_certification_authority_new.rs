
impl LocalModelCertificationAuthority {
    pub fn new(
        signing_key_id: impl Into<String>,
        signing_key: &[u8; 32],
        registry_path: impl AsRef<Path>,
    ) -> AgentResult<Self> {
        let signing_key_id = signing_key_id.into();
        let registry_path = registry_path.as_ref();
        if signing_key_id.trim().is_empty()
            || signing_key_id.len() > 128
            || !registry_path.is_absolute()
            || registry_path.file_name().is_none()
        {
            return Err(invalid_certification_configuration());
        }
        let parent = registry_path
            .parent()
            .ok_or_else(invalid_certification_configuration)?;
        let parent_metadata =
            std::fs::symlink_metadata(parent).map_err(|_| invalid_certification_configuration())?;
        if !parent_metadata.is_dir() || parent_metadata.file_type().is_symlink() {
            return Err(invalid_certification_configuration());
        }
        if registry_path.exists() {
            let metadata = std::fs::symlink_metadata(registry_path)
                .map_err(|_| invalid_certification_configuration())?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(invalid_certification_configuration());
            }
        } else {
            let file = create_registry_file(registry_path)?;
            file.sync_all()
                .map_err(|_| invalid_certification_configuration())?;
        }
        let authority = Self {
            signing_key_id,
            signing_key: Zeroizing::new(*signing_key),
            registry_path: registry_path.to_path_buf(),
        };
        authority.validate_registry()?;
        Ok(authority)
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
        let file = open_registry_read(&self.registry_path)?;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|_| invalid_certification_configuration())?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: RegistryEntry =
                serde_json::from_str(&line).map_err(|_| invalid_certification_configuration())?;
            self.verify_certificate_signature(&entry.certificate)?;
            self.verify_registry_mac(&entry)?;
        }
        Ok(())
    }

    fn latest_registry_state(
        &self,
        certificate: &LocalModelCertificate,
    ) -> AgentResult<Option<RegistryState>> {
        let file = open_registry_read(&self.registry_path)?;
        let mut latest = None;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|_| invalid_certification_configuration())?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: RegistryEntry =
                serde_json::from_str(&line).map_err(|_| invalid_certification_configuration())?;
            self.verify_certificate_signature(&entry.certificate)?;
            self.verify_registry_mac(&entry)?;
            if entry.certificate.certificate_id == certificate.certificate_id
                && entry.certificate.signature == certificate.signature
            {
                latest = Some(entry.state);
            }
        }
        Ok(latest)
    }

    fn append_registry_entry(
        &self,
        certificate: &LocalModelCertificate,
        state: RegistryState,
    ) -> AgentResult<()> {
        let mut entry = RegistryEntry {
            certificate: certificate.clone(),
            state,
            registry_mac: String::new(),
        };
        entry.registry_mac = self.registry_mac(&entry)?;
        let encoded =
            serde_json::to_vec(&entry).map_err(|_| invalid_certification_configuration())?;
        let mut file = open_registry_append(&self.registry_path)?;
        file.write_all(&encoded)
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|_| invalid_certification_configuration())
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

    fn registry_mac(&self, entry: &RegistryEntry) -> AgentResult<String> {
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
        Ok(format!("hmac-sha256:{:x}", mac.finalize().into_bytes()))
    }

    fn verify_registry_mac(&self, entry: &RegistryEntry) -> AgentResult<()> {
        if entry.registry_mac != self.registry_mac(entry)? {
            return Err(AgentError::LocalModelNotCertifiedForAiKp);
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
    use sha2::Digest as _;
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

fn create_registry_file(path: &Path) -> AgentResult<File> {
    let mut options = OpenOptions::new();
    options.create_new(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|_| invalid_certification_configuration())
}

fn open_registry_read(path: &Path) -> AgentResult<File> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|_| invalid_certification_configuration())
}

fn open_registry_append(path: &Path) -> AgentResult<File> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| invalid_certification_configuration())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(invalid_certification_configuration());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(invalid_certification_configuration());
        }
    }
    OpenOptions::new()
        .append(true)
        .read(true)
        .open(path)
        .map_err(|_| invalid_certification_configuration())
}

fn invalid_certification_configuration() -> AgentError {
    AgentError::Core(trpg_shared_kernel::TrpgError::InvalidConfiguration(
        "local_model_certification_registry_invalid",
    ))
}
