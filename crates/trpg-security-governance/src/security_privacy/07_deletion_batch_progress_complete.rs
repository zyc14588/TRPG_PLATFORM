
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

#[derive(Clone)]
pub struct S3ObjectDeletionSurface {
    bucket: Box<Bucket>,
}

impl std::fmt::Debug for S3ObjectDeletionSurface {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3ObjectDeletionSurface")
            .field("bucket", &self.bucket.name)
            .field("endpoint", &"[REDACTED]")
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
    ) -> Result<Self, PrivacyError> {
        validate_secure_service_url(endpoint, "http", "https")?;
        validate_id(region)?;
        validate_id(bucket_name)?;
        if access_key.trim().is_empty() || secret_key.len() < 8 {
            return Err(PrivacyError::InvalidInput);
        }
        let credentials = Credentials::new(Some(access_key), Some(secret_key), None, None, None)
            .map_err(|_| PrivacyError::Storage)?;
        let region = Region::Custom {
            region: region.to_owned(),
            endpoint: endpoint.trim_end_matches('/').to_owned(),
        };
        let bucket = Bucket::new(bucket_name, region, credentials)
            .map_err(|_| PrivacyError::Storage)?
            .with_path_style();
        if !bucket.exists().await.map_err(|_| PrivacyError::Storage)? {
            return Err(PrivacyError::Storage);
        }
        let surface = Self { bucket };
        surface.verify_unversioned_bucket_at_startup().await?;
        Ok(surface)
    }

    pub fn subject_prefix(subject_id: &str) -> Result<String, PrivacyError> {
        validate_id(subject_id)?;
        Ok(format!("subjects/{}/", sha256_hex(subject_id.as_bytes())))
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
        self.bucket
            .put_object(key, protected_payload)
            .await
            .map(|_| ())
            .map_err(|_| PrivacyError::Storage)
    }

    async fn subject_keys(&self, subject_id: &str) -> Result<Vec<String>, PrivacyError> {
        let prefix = Self::subject_prefix(subject_id)?;
        self.bucket
            .list(prefix, None)
            .await
            .map(|pages| {
                pages
                    .into_iter()
                    .flat_map(|page| page.contents)
                    .map(|object| object.key)
                    .collect()
            })
            .map_err(|_| PrivacyError::Storage)
    }

    /// The current adapter can prove deletion only for an unversioned bucket.
    /// Probe once while constructing the surface; a versioned bucket would
    /// make a normal DELETE retain recoverable historical bytes. Runtime
    /// bucket-policy changes require constructing a fresh surface.
    async fn verify_unversioned_bucket_at_startup(&self) -> Result<(), PrivacyError> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| PrivacyError::Storage)?
            .as_nanos();
        let key = format!(
            ".trpg-erasure-versioning-probe/{}-{nonce}",
            std::process::id()
        );
        let response = self
            .bucket
            .put_object(&key, b"versioning-probe")
            .await
            .map_err(|_| PrivacyError::Storage)?;
        let version_id = response
            .headers()
            .get("x-amz-version-id")
            .filter(|value| !value.trim().is_empty() && value.as_str() != "null")
            .cloned();
        if let Some(version_id) = version_id {
            let cleanup = self
                .bucket
                .delete_objects(vec![ObjectIdentifier::with_version(&key, version_id)])
                .await
                .map_err(|_| PrivacyError::Storage)?;
            if !cleanup.errors.is_empty() {
                return Err(PrivacyError::Storage);
            }
            return Err(PrivacyError::InvalidPersistedState);
        }
        self.bucket
            .delete_object(&key)
            .await
            .map_err(|_| PrivacyError::Storage)?;
        Ok(())
    }
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
        for key in self.subject_keys(context.subject_id()).await? {
            self.bucket
                .delete_object(key)
                .await
                .map_err(|_| PrivacyError::Storage)?;
        }
        DeletionBatchProgress::complete(cursor)
    }

    async fn verify_absent(&self, subject_id: &str) -> Result<bool, PrivacyError> {
        Ok(self.subject_keys(subject_id).await?.is_empty())
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
