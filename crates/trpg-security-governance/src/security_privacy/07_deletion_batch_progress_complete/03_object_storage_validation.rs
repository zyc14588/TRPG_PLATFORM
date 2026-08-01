
fn validate_s3_tls_binding(endpoint: &str, ca_bundle_path: &Path) -> Result<(), PrivacyError> {
    let parsed = Url::parse(endpoint).map_err(|_| PrivacyError::InvalidInput)?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
    {
        return Err(PrivacyError::InvalidInput);
    }
    if !ca_bundle_path.is_absolute() {
        return Err(PrivacyError::InvalidInput);
    }
    let metadata =
        std::fs::symlink_metadata(ca_bundle_path).map_err(|_| PrivacyError::InvalidInput)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(PrivacyError::InvalidInput);
    }
    let canonical_ca =
        std::fs::canonicalize(ca_bundle_path).map_err(|_| PrivacyError::InvalidInput)?;
    let configured_ca = std::env::var_os("SSL_CERT_FILE")
        .map(PathBuf::from)
        .ok_or(PrivacyError::InvalidInput)?;
    let canonical_configured =
        std::fs::canonicalize(configured_ca).map_err(|_| PrivacyError::InvalidInput)?;
    if canonical_ca != canonical_configured {
        return Err(PrivacyError::InvalidInput);
    }
    let pem = std::fs::read(ca_bundle_path).map_err(|_| PrivacyError::InvalidInput)?;
    if !pem
        .windows(b"-----BEGIN CERTIFICATE-----".len())
        .any(|window| window == b"-----BEGIN CERTIFICATE-----")
        || !pem
            .windows(b"-----END CERTIFICATE-----".len())
            .any(|window| window == b"-----END CERTIFICATE-----")
    {
        return Err(PrivacyError::InvalidInput);
    }
    Ok(())
}

fn s3_erasure_manifest_sha256(
    bucket: &str,
    prefix: &str,
    identifiers: &HashSet<S3VersionIdentifier>,
) -> String {
    let mut identifiers = identifiers.iter().collect::<Vec<_>>();
    identifiers.sort_by(|left, right| {
        (&left.key, &left.version_id).cmp(&(&right.key, &right.version_id))
    });
    let mut manifest = Sha256::new();
    manifest.update((bucket.len() as u64).to_be_bytes());
    manifest.update(bucket.as_bytes());
    manifest.update((prefix.len() as u64).to_be_bytes());
    manifest.update(prefix.as_bytes());
    for identifier in identifiers {
        manifest.update((identifier.key.len() as u64).to_be_bytes());
        manifest.update(identifier.key.as_bytes());
        match identifier.version_id.as_deref() {
            Some(version_id) => {
                manifest.update([1]);
                manifest.update((version_id.len() as u64).to_be_bytes());
                manifest.update(version_id.as_bytes());
            }
            None => manifest.update([0]),
        }
    }
    format!("{:x}", manifest.finalize())
}

#[async_trait]
impl DeletionSurface for S3ObjectDeletionSurface {
    fn target(&self) -> DeletionTarget {
        DeletionTarget::ObjectStorage
    }

    async fn delete_subject_batch(
        &self,
        context: &DeletionExecutionContext,
        cursor: u64,
    ) -> Result<DeletionBatchProgress, PrivacyError> {
        if cursor != 1 {
            return Err(PrivacyError::InvalidPersistedState);
        }
        self.erase_subject(context.subject_id()).await?;
        DeletionBatchProgress::complete(cursor)
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        let mode = self.versioning_mode().await?;
        let prefix = Self::subject_prefix(subject_id)?;
        Ok(self
            .list_subject_objects(mode, &prefix)
            .await?
            .identifiers
            .is_empty())
    }
}
