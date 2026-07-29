
impl DeletionBatchProgress {
    fn complete(cursor: u64) -> Result<Self, PrivacyError> {
        if cursor == 0 {
            return Err(PrivacyError::InvalidPersistedState);
        }
        Ok(Self {
            next_cursor: cursor,
            complete: true,
        })
    }
}

const S3_LIST_PAGE_SIZE: i32 = 100;
const S3_DELETE_REQUEST_SIZE: usize = 1_000;
const S3_MAX_LIST_PAGES: usize = 100_000;
const S3_MAX_ERASURE_PASSES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum S3VersioningMode {
    NeverVersioned,
    VersionHistory,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct S3VersionIdentifier {
    key: String,
    version_id: Option<String>,
}

#[derive(Clone, Debug)]
struct S3ListedObjects {
    identifiers: Vec<S3VersionIdentifier>,
    page_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct S3DeleteReceiptSummary {
    pub request_count: u64,
    pub requested_count: u64,
    pub confirmed_count: u64,
    pub error_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct S3ErasureEvidence {
    pub bucket: String,
    pub prefix: String,
    pub page_count: u64,
    pub version_count: u64,
    pub delete_receipt_summary: S3DeleteReceiptSummary,
    pub final_verified_at_unix_ms: u64,
    pub manifest_sha256: String,
}

#[derive(Clone)]
pub struct S3ObjectDeletionSurface {
    client: S3Client,
    bucket: String,
    last_evidence: std::sync::Arc<std::sync::Mutex<Option<S3ErasureEvidence>>>,
}

impl std::fmt::Debug for S3ObjectDeletionSurface {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3ObjectDeletionSurface")
            .field("bucket", &self.bucket)
            .field("endpoint", &"[REDACTED]")
            .field("credentials", &"[REDACTED]")
            .finish()
    }
}

impl S3ObjectDeletionSurface {
    pub async fn connect(
        endpoint: &str,
        region: &str,
        bucket_name: &str,
        access_key: &str,
        secret_key: &str,
        ca_bundle_path: &Path,
    ) -> Result<Self, PrivacyError> {
        validate_s3_tls_binding(endpoint, ca_bundle_path)?;
        validate_id(region)?;
        validate_id(bucket_name)?;
        if access_key.trim().is_empty() || secret_key.len() < 8 {
            return Err(PrivacyError::InvalidInput);
        }
        let credentials = Credentials::new(
            access_key,
            secret_key,
            None,
            None,
            "trpg-object-erasure-service-account",
        );
        // The default HTTPS client loads roots through rustls-native-certs. The
        // validation above pins that loader's SSL_CERT_FILE to this exact bundle.
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(region.to_owned()))
            .credentials_provider(credentials)
            .endpoint_url(endpoint.trim_end_matches('/'))
            .force_path_style(true)
            .build();
        let surface = Self {
            client: S3Client::from_conf(config),
            bucket: bucket_name.to_owned(),
            last_evidence: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };

        // Root and broadly administrative credentials can perform both calls.
        // The deletion worker must fail closed instead of accepting them.
        if surface.client.list_buckets().send().await.is_ok()
            || surface
                .client
                .get_bucket_acl()
                .bucket(&surface.bucket)
                .send()
                .await
                .is_ok()
        {
            return Err(PrivacyError::InvalidInput);
        }
        surface.versioning_mode().await?;
        Ok(surface)
    }

    pub fn subject_prefix(subject_id: &str) -> Result<String, PrivacyError> {
        validate_id(subject_id)?;
        Ok(format!("subjects/{}/", sha256_hex(subject_id.as_bytes())))
    }

    pub fn last_erasure_evidence(&self) -> Result<Option<S3ErasureEvidence>, PrivacyError> {
        self.last_evidence
            .lock()
            .map(|evidence| evidence.clone())
            .map_err(|_| PrivacyError::Storage)
    }

    pub async fn put_protected_object(
        &self,
        subject_id: &str,
        object_id: &str,
        protected_payload: &[u8],
    ) -> Result<(), PrivacyError> {
        validate_id(object_id)?;
        if protected_payload.is_empty() {
            return Err(PrivacyError::InvalidInput);
        }
        let key = format!("{}{}", Self::subject_prefix(subject_id)?, object_id);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(protected_payload.to_vec()))
            .send()
            .await
            .map(|_| ())
            .map_err(|_| PrivacyError::Storage)
    }

    async fn versioning_mode(&self) -> Result<S3VersioningMode, PrivacyError> {
        let output = self
            .client
            .get_bucket_versioning()
            .bucket(&self.bucket)
            .send()
            .await
            .map_err(|_| PrivacyError::Storage)?;
        match output.status() {
            None => Ok(S3VersioningMode::NeverVersioned),
            Some(BucketVersioningStatus::Enabled | BucketVersioningStatus::Suspended) => {
                Ok(S3VersioningMode::VersionHistory)
            }
            Some(_) => Err(PrivacyError::InvalidPersistedState),
        }
    }

    async fn list_version_history(
        &self,
        prefix: &str,
    ) -> Result<S3ListedObjects, PrivacyError> {
        let mut identifiers = Vec::new();
        let mut seen = HashSet::new();
        let mut key_marker = None;
        let mut version_id_marker = None;
        let mut page_count = 0_u64;
        loop {
            page_count = page_count
                .checked_add(1)
                .ok_or(PrivacyError::InvalidPersistedState)?;
            if page_count as usize > S3_MAX_LIST_PAGES {
                return Err(PrivacyError::InvalidPersistedState);
            }
            let output = self
                .client
                .list_object_versions()
                .bucket(&self.bucket)
                .prefix(prefix)
                .max_keys(S3_LIST_PAGE_SIZE)
                .set_key_marker(key_marker.clone())
                .set_version_id_marker(version_id_marker.clone())
                .send()
                .await
                .map_err(|_| PrivacyError::Storage)?;
            for (key, version_id) in output
                .versions()
                .iter()
                .map(|entry| (entry.key(), entry.version_id()))
                .chain(
                    output
                        .delete_markers()
                        .iter()
                        .map(|entry| (entry.key(), entry.version_id())),
                )
            {
                let key = key
                    .filter(|key| key.starts_with(prefix))
                    .ok_or(PrivacyError::InvalidPersistedState)?;
                let version_id = version_id
                    .filter(|version_id| !version_id.trim().is_empty())
                    .ok_or(PrivacyError::InvalidPersistedState)?;
                let identifier = S3VersionIdentifier {
                    key: key.to_owned(),
                    version_id: Some(version_id.to_owned()),
                };
                if !seen.insert(identifier.clone()) {
                    return Err(PrivacyError::InvalidPersistedState);
                }
                identifiers.push(identifier);
            }
            let truncated = output
                .is_truncated()
                .ok_or(PrivacyError::InvalidPersistedState)?;
            if !truncated {
                break;
            }
            let next_key_marker = output
                .next_key_marker()
                .filter(|marker| !marker.is_empty())
                .ok_or(PrivacyError::InvalidPersistedState)?
                .to_owned();
            let next_version_id_marker =
                output.next_version_id_marker().map(ToOwned::to_owned);
            if key_marker.as_deref() == Some(next_key_marker.as_str())
                && version_id_marker == next_version_id_marker
            {
                return Err(PrivacyError::InvalidPersistedState);
            }
            key_marker = Some(next_key_marker);
            version_id_marker = next_version_id_marker;
        }
        Ok(S3ListedObjects {
            identifiers,
            page_count,
        })
    }

    async fn list_unversioned(
        &self,
        prefix: &str,
    ) -> Result<S3ListedObjects, PrivacyError> {
        let mut identifiers = Vec::new();
        let mut seen = HashSet::new();
        let mut continuation_token = None;
        let mut page_count = 0_u64;
        loop {
            page_count = page_count
                .checked_add(1)
                .ok_or(PrivacyError::InvalidPersistedState)?;
            if page_count as usize > S3_MAX_LIST_PAGES {
                return Err(PrivacyError::InvalidPersistedState);
            }
            let output = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix)
                .max_keys(S3_LIST_PAGE_SIZE)
                .set_continuation_token(continuation_token.clone())
                .send()
                .await
                .map_err(|_| PrivacyError::Storage)?;
            for key in output.contents().iter().map(|entry| entry.key()) {
                let key = key
                    .filter(|key| key.starts_with(prefix))
                    .ok_or(PrivacyError::InvalidPersistedState)?;
                let identifier = S3VersionIdentifier {
                    key: key.to_owned(),
                    version_id: None,
                };
                if !seen.insert(identifier.clone()) {
                    return Err(PrivacyError::InvalidPersistedState);
                }
                identifiers.push(identifier);
            }
            let truncated = output
                .is_truncated()
                .ok_or(PrivacyError::InvalidPersistedState)?;
            if !truncated {
                break;
            }
            let next_token = output
                .next_continuation_token()
                .filter(|token| !token.is_empty())
                .ok_or(PrivacyError::InvalidPersistedState)?
                .to_owned();
            if continuation_token.as_deref() == Some(next_token.as_str()) {
                return Err(PrivacyError::InvalidPersistedState);
            }
            continuation_token = Some(next_token);
        }
        Ok(S3ListedObjects {
            identifiers,
            page_count,
        })
    }

    async fn list_subject_objects(
        &self,
        mode: S3VersioningMode,
        prefix: &str,
    ) -> Result<S3ListedObjects, PrivacyError> {
        match mode {
            S3VersioningMode::NeverVersioned => self.list_unversioned(prefix).await,
            S3VersioningMode::VersionHistory => self.list_version_history(prefix).await,
        }
    }

    async fn delete_identifiers(
        &self,
        identifiers: &[S3VersionIdentifier],
        summary: &mut S3DeleteReceiptSummary,
    ) -> Result<(), PrivacyError> {
        for chunk in identifiers.chunks(S3_DELETE_REQUEST_SIZE) {
            let objects = chunk
                .iter()
                .map(|identifier| {
                    ObjectIdentifier::builder()
                        .key(&identifier.key)
                        .set_version_id(identifier.version_id.clone())
                        .build()
                        .map_err(|_| PrivacyError::InvalidPersistedState)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let delete = Delete::builder()
                .set_objects(Some(objects))
                .quiet(false)
                .build()
                .map_err(|_| PrivacyError::InvalidPersistedState)?;
            summary.request_count = summary
                .request_count
                .checked_add(1)
                .ok_or(PrivacyError::InvalidPersistedState)?;
            summary.requested_count = summary
                .requested_count
                .checked_add(chunk.len() as u64)
                .ok_or(PrivacyError::InvalidPersistedState)?;
            let output = self
                .client
                .delete_objects()
                .bucket(&self.bucket)
                .delete(delete)
                .send()
                .await
                .map_err(|_| PrivacyError::Storage)?;
            summary.error_count = summary
                .error_count
                .checked_add(output.errors().len() as u64)
                .ok_or(PrivacyError::InvalidPersistedState)?;
            if !output.errors().is_empty() {
                return Err(PrivacyError::Storage);
            }
            let expected = chunk
                .iter()
                .cloned()
                .collect::<HashSet<S3VersionIdentifier>>();
            let confirmed = output
                .deleted()
                .iter()
                .map(|deleted| {
                    let key = deleted
                        .key()
                        .filter(|key| !key.is_empty())
                        .ok_or(PrivacyError::InvalidPersistedState)?;
                    Ok(S3VersionIdentifier {
                        key: key.to_owned(),
                        version_id: deleted.version_id().map(ToOwned::to_owned),
                    })
                })
                .collect::<Result<HashSet<_>, PrivacyError>>()?;
            if confirmed != expected {
                return Err(PrivacyError::Storage);
            }
            summary.confirmed_count = summary
                .confirmed_count
                .checked_add(confirmed.len() as u64)
                .ok_or(PrivacyError::InvalidPersistedState)?;
        }
        Ok(())
    }

    async fn erase_subject(&self, subject_id: &str) -> Result<(), PrivacyError> {
        let prefix = Self::subject_prefix(subject_id)?;
        let mut all_identifiers = HashSet::new();
        let mut page_count = 0_u64;
        let mut summary = S3DeleteReceiptSummary {
            request_count: 0,
            requested_count: 0,
            confirmed_count: 0,
            error_count: 0,
        };
        for _ in 0..S3_MAX_ERASURE_PASSES {
            // Re-read on every pass so a concurrent NeverVersioned -> Enabled
            // transition cannot leave a newly created historical version behind.
            let mode = self.versioning_mode().await?;
            let listed = self.list_subject_objects(mode, &prefix).await?;
            page_count = page_count
                .checked_add(listed.page_count)
                .ok_or(PrivacyError::InvalidPersistedState)?;
            if listed.identifiers.is_empty() {
                let manifest_sha256 =
                    s3_erasure_manifest_sha256(&self.bucket, &prefix, &all_identifiers);
                let evidence = S3ErasureEvidence {
                    bucket: self.bucket.clone(),
                    prefix,
                    page_count,
                    version_count: all_identifiers.len() as u64,
                    delete_receipt_summary: summary,
                    final_verified_at_unix_ms: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(|_| PrivacyError::Storage)?
                        .as_millis()
                        .try_into()
                        .map_err(|_| PrivacyError::InvalidPersistedState)?,
                    manifest_sha256,
                };
                let encoded =
                    serde_json::to_string(&evidence).map_err(|_| PrivacyError::Storage)?;
                *self
                    .last_evidence
                    .lock()
                    .map_err(|_| PrivacyError::Storage)? = Some(evidence);
                eprintln!("trpg_object_erasure_evidence={encoded}");
                return Ok(());
            }
            all_identifiers.extend(listed.identifiers.iter().cloned());
            self.delete_identifiers(&listed.identifiers, &mut summary)
                .await?;
        }
        Err(PrivacyError::Storage)
    }
}

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

const REDIS_DELETE_SUBJECT_KEYS: &str = r#"
local members = redis.call('SMEMBERS', KEYS[1])
local removed = 0
for _, key in ipairs(members) do
  removed = removed + redis.call('DEL', key)
end
redis.call('DEL', KEYS[1])
return removed
"#;

const REDIS_COUNT_SUBJECT_KEYS: &str = r#"
return redis.call('SCARD', KEYS[1]) + redis.call('EXISTS', KEYS[1])
"#;

const REDIS_AUDIT_SCAN_COUNT: usize = 256;
const REDIS_MAX_AUDIT_ENTRIES: usize = 100_000;

#[derive(Clone)]
pub struct RedisCacheDeletionSurface {
    connection: ConnectionManager,
    namespace: String,
}

impl RedisCacheDeletionSurface {
    pub async fn connect(redis_url: &str, namespace: &str) -> Result<Self, PrivacyError> {
        Self::connect_with_tls(redis_url, namespace, None, None, None).await
    }

    pub async fn connect_with_tls(
        redis_url: &str,
        namespace: &str,
        root_certificate: Option<&[u8]>,
        client_certificate: Option<&[u8]>,
        client_private_key: Option<&[u8]>,
    ) -> Result<Self, PrivacyError> {
        validate_secure_service_url(redis_url, "redis", "rediss")?;
        validate_redis_namespace(namespace)?;
        let client = build_redis_client(
            redis_url,
            root_certificate,
            client_certificate,
            client_private_key,
        )?;
        let mut connection = ConnectionManager::new(client)
            .await
            .map_err(|_| PrivacyError::Cache)?;
        let pong: String = redis::cmd("PING")
            .query_async(&mut connection)
            .await
            .map_err(|_| PrivacyError::Cache)?;
        if pong != "PONG" {
            return Err(PrivacyError::Cache);
        }
        Ok(Self {
            connection,
            namespace: namespace.to_owned(),
        })
    }

    pub async fn put_for_test(
        &self,
        subject_id: &str,
        record_key: &str,
        protected_payload: &[u8],
    ) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        validate_id(record_key)?;
        if protected_payload.is_empty() {
            return Err(PrivacyError::InvalidInput);
        }
        let mut connection = self.connection.clone();
        let entry_key = self.key(record_key);
        let subject_index_key = self.subject_index_key(subject_id);
        redis::cmd("SET")
            .arg(&entry_key)
            .arg(protected_payload)
            .query_async::<()>(&mut connection)
            .await
            .map_err(|_| PrivacyError::Cache)?;
        redis::cmd("SADD")
            .arg(subject_index_key)
            .arg(entry_key)
            .query_async::<i64>(&mut connection)
            .await
            .map(|_| ())
            .map_err(|_| PrivacyError::Cache)
    }

    fn key(&self, record_key: &str) -> String {
        format!(
            "{}:entry:{}",
            self.namespace,
            sha256_hex(record_key.as_bytes())
        )
    }

    fn subject_index_key(&self, subject_id: &str) -> String {
        format!(
            "{}:subject:{}",
            self.namespace,
            sha256_hex(subject_id.as_bytes())
        )
    }

    async fn independently_classified_subject_entries(
        &self,
        subject_id: &str,
    ) -> Result<Vec<String>, PrivacyError> {
        validate_id(subject_id)?;
        let expected_index = self.subject_index_key(subject_id);
        let pattern = format!("{}:entry:*", self.namespace);
        let mut connection = self.connection.clone();
        let mut cursor = 0_u64;
        let mut observed_cursors = std::collections::HashSet::new();
        let mut entry_keys = std::collections::BTreeSet::new();
        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(REDIS_AUDIT_SCAN_COUNT)
                .query_async(&mut connection)
                .await
                .map_err(|_| PrivacyError::Cache)?;
            entry_keys.extend(keys);
            if entry_keys.len() > REDIS_MAX_AUDIT_ENTRIES {
                return Err(PrivacyError::InvalidPersistedState);
            }
            if next_cursor == 0 {
                break;
            }
            if !observed_cursors.insert(next_cursor) {
                return Err(PrivacyError::InvalidPersistedState);
            }
            cursor = next_cursor;
        }

        let mut matching = Vec::new();
        for entry_key in entry_keys {
            let payload: Option<Vec<u8>> = redis::cmd("GET")
                .arg(&entry_key)
                .query_async(&mut connection)
                .await
                .map_err(|_| PrivacyError::Cache)?;
            let Some(payload) = payload else {
                continue;
            };
            let stored = serde_json::from_slice::<Value>(&payload)
                .map_err(|_| PrivacyError::InvalidPersistedState)?;
            let subject_index_key = stored
                .get("subject_index_key")
                .and_then(Value::as_str)
                .ok_or(PrivacyError::InvalidPersistedState)?;
            if subject_index_key == expected_index {
                matching.push(entry_key);
            }
        }
        Ok(matching)
    }

    pub async fn remove_subject_index_for_test(
        &self,
        subject_id: &str,
    ) -> Result<(), PrivacyError> {
        validate_id(subject_id)?;
        let mut connection = self.connection.clone();
        redis::cmd("DEL")
            .arg(self.subject_index_key(subject_id))
            .query_async::<i64>(&mut connection)
            .await
            .map(|_| ())
            .map_err(|_| PrivacyError::Cache)
    }
}

fn validate_redis_namespace(value: &str) -> Result<(), PrivacyError> {
    if value.trim().is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.'))
    {
        Err(PrivacyError::InvalidInput)
    } else {
        Ok(())
    }
}
