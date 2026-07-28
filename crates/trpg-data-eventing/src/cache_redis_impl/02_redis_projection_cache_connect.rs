
impl RedisProjectionCache {
    pub async fn connect(
        redis_url: &str,
        namespace: &str,
        key_reference: &str,
        encryption_key: &[u8],
    ) -> Result<Self, RedisProjectionError> {
        Self::connect_with_tls(
            redis_url,
            namespace,
            key_reference,
            encryption_key,
            None,
            None,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn connect_with_tls(
        redis_url: &str,
        namespace: &str,
        key_reference: &str,
        encryption_key: &[u8],
        root_certificate: Option<&[u8]>,
        client_certificate: Option<&[u8]>,
        client_private_key: Option<&[u8]>,
    ) -> Result<Self, RedisProjectionError> {
        validate_redis_url(redis_url)?;
        validate_name(namespace, "namespace_required")?;
        let cryptography = Arc::new(CacheCryptography::new(key_reference, encryption_key)?);
        let client = redis_client(
            redis_url,
            root_certificate,
            client_certificate,
            client_private_key,
        )?;
        let connection = ConnectionManager::new(client)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        let mut cache = Self {
            connection,
            namespace: namespace.to_owned(),
            cryptography,
        };
        cache.check_readiness().await?;
        Ok(cache)
    }

    pub async fn check_readiness(&mut self) -> Result<(), RedisProjectionError> {
        let response: String = redis::cmd("PING")
            .query_async(&mut self.connection)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        if response == "PONG" {
            Ok(())
        } else {
            Err(RedisProjectionError::Unavailable)
        }
    }

    pub async fn put(&self, entry: &ProjectionCacheEntry) -> Result<(), RedisProjectionError> {
        let normalized = normalize_entry(entry)?;
        let stored = self.protect(&normalized)?;
        let encoded = serde_json::to_string(&stored)
            .map_err(|_| RedisProjectionError::InvalidEntry("serialization_failed"))?;
        let mut connection = self.connection.clone();
        let outcome: i64 = redis::Script::new(MONOTONIC_PROJECTION_SCRIPT)
            .key(self.redis_key(normalized.key()))
            .key(&stored.subject_index_key)
            .arg(normalized.version())
            .arg(encoded)
            .arg(normalized.ttl_seconds())
            .arg(&stored.entry_digest)
            .invoke_async(&mut connection)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        match outcome {
            1 => Ok(()),
            -1 => Err(RedisProjectionError::VersionRegression),
            -2 => Err(RedisProjectionError::VersionCollision),
            _ => Err(RedisProjectionError::Unavailable),
        }
    }

    /// Reads only through a live, campaign-bound identity capability. Session,
    /// membership, and private-group revocation are therefore rechecked before
    /// any decrypted value can leave the cache adapter.
    pub async fn get_authorized(
        &self,
        key: &str,
        authorization: &ReplayAuthorization,
        now_unix_ms: u64,
    ) -> Result<Option<ProjectionCacheEntry>, RedisProjectionError> {
        validate_name(key, "cache_key_required")?;
        let Some(stored) = self.get_stored(key).await? else {
            return Ok(None);
        };
        validate_stored(&stored, self.subject_index_key_from_stored(&stored)?)?;
        let entry = self.unprotect(key, stored)?;
        let campaign_id = EntityId::new(entry.campaign_id())
            .map_err(|_| RedisProjectionError::InvalidEntry("stored_campaign_invalid"))?;
        let subject =
            (entry.visibility_subject() != "not_applicable").then_some(entry.visibility_subject());
        let visibility = Visibility::try_from_parts(entry.visibility_label(), subject)
            .map_err(|_| RedisProjectionError::InvalidEntry("stored_visibility_invalid"))?;
        let permitted = authorization
            .can_view(&campaign_id, &visibility, now_unix_ms)
            .map_err(|_| RedisProjectionError::AuthorizationDenied)?;
        if !permitted {
            return Err(RedisProjectionError::AuthorizationDenied);
        }
        Ok(Some(entry))
    }

    pub async fn invalidate(&self, key: &str) -> Result<(), RedisProjectionError> {
        validate_name(key, "cache_key_required")?;
        let mut connection = self.connection.clone();
        redis::Script::new(INVALIDATE_ENTRY_SCRIPT)
            .key(self.redis_key(key))
            .invoke_async::<i64>(&mut connection)
            .await
            .map(|_| ())
            .map_err(|_| RedisProjectionError::Unavailable)
    }

    /// Immediately removes all production-cache values indexed to a data
    /// subject without a global Redis SCAN.
    pub async fn invalidate_subject(
        &self,
        data_subject_id: &str,
    ) -> Result<u64, RedisProjectionError> {
        validate_name(data_subject_id, "data_subject_required")?;
        let mut connection = self.connection.clone();
        let removed = redis::Script::new(INVALIDATE_SUBJECT_SCRIPT)
            .key(self.subject_index_key(data_subject_id))
            .invoke_async::<i64>(&mut connection)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        u64::try_from(removed).map_err(|_| RedisProjectionError::Unavailable)
    }

    fn protect(
        &self,
        entry: &ProjectionCacheEntry,
    ) -> Result<StoredProjectionCacheEntry, RedisProjectionError> {
        let subject_index_key = self.subject_index_key(&entry.data_subject_id);
        let protected = ProtectedProjectionCacheEntry {
            key: entry.key.clone(),
            campaign_id: entry.campaign_id.clone(),
            data_subject_id: entry.data_subject_id.clone(),
            visibility_label: entry.visibility_label.clone(),
            visibility_subject: entry.visibility_subject.clone(),
            provenance_kind: entry.provenance_kind.clone(),
            provenance_reference: entry.provenance_reference.clone(),
            value_json: entry.value_json.clone(),
        };
        let plaintext = serde_json::to_vec(&protected)
            .map_err(|_| RedisProjectionError::InvalidEntry("serialization_failed"))?;
        let version = entry.version.to_string();
        let ttl = entry.ttl_seconds.to_string();
        let aad = [
            "redis_projection_cache_v1",
            self.namespace.as_str(),
            version.as_str(),
            ttl.as_str(),
            subject_index_key.as_str(),
        ];
        let protected_entry = self
            .cryptography
            .cipher
            .encrypt_json(&plaintext, &aad)
            .map_err(|_| RedisProjectionError::Cryptography)?;
        let entry_digest = self.cryptography.digest(&plaintext, &aad)?;
        Ok(StoredProjectionCacheEntry {
            schema_version: CACHE_SCHEMA_VERSION,
            version: entry.version,
            ttl_seconds: entry.ttl_seconds,
            subject_index_key,
            entry_digest,
            protected_entry,
        })
    }

    fn unprotect(
        &self,
        requested_key: &str,
        stored: StoredProjectionCacheEntry,
    ) -> Result<ProjectionCacheEntry, RedisProjectionError> {
        let version = stored.version.to_string();
        let ttl = stored.ttl_seconds.to_string();
        let aad = [
            "redis_projection_cache_v1",
            self.namespace.as_str(),
            version.as_str(),
            ttl.as_str(),
            stored.subject_index_key.as_str(),
        ];
        let decrypted = self
            .cryptography
            .cipher
            .decrypt_json(&stored.protected_entry, &aad)
            .map_err(|_| RedisProjectionError::Cryptography)?;
        if self.cryptography.digest(decrypted.as_bytes(), &aad)? != stored.entry_digest {
            return Err(RedisProjectionError::Cryptography);
        }
        let protected: ProtectedProjectionCacheEntry = serde_json::from_slice(decrypted.as_bytes())
            .map_err(|_| RedisProjectionError::InvalidEntry("stored_entry_invalid"))?;
        if protected.key != requested_key
            || stored.subject_index_key != self.subject_index_key(&protected.data_subject_id)
        {
            return Err(RedisProjectionError::InvalidEntry("stored_binding_invalid"));
        }
        ProjectionCacheEntry::new(
            protected.key,
            protected.campaign_id,
            protected.data_subject_id,
            stored.version,
            protected.visibility_label,
            protected.visibility_subject,
            protected.provenance_kind,
            protected.provenance_reference,
            protected.value_json,
            stored.ttl_seconds,
        )
    }

    async fn get_stored(
        &self,
        key: &str,
    ) -> Result<Option<StoredProjectionCacheEntry>, RedisProjectionError> {
        let mut connection = self.connection.clone();
        let encoded: Option<String> = redis::cmd("GET")
            .arg(self.redis_key(key))
            .query_async(&mut connection)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        encoded
            .map(|encoded| {
                serde_json::from_str(&encoded)
                    .map_err(|_| RedisProjectionError::InvalidEntry("stored_entry_invalid"))
            })
            .transpose()
    }

    fn subject_index_key_from_stored<'a>(
        &self,
        stored: &'a StoredProjectionCacheEntry,
    ) -> Result<&'a str, RedisProjectionError> {
        let prefix = format!("{}:subject:", self.namespace);
        if stored.subject_index_key.starts_with(&prefix)
            && stored.subject_index_key.len() == prefix.len() + 64
            && stored.subject_index_key[prefix.len()..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            Ok(stored.subject_index_key.as_str())
        } else {
            Err(RedisProjectionError::InvalidEntry(
                "stored_subject_index_invalid",
            ))
        }
    }

    fn redis_key(&self, key: &str) -> String {
        format!("{}:entry:{}", self.namespace, sha256_hex(key.as_bytes()))
    }

    fn subject_index_key(&self, data_subject_id: &str) -> String {
        format!(
            "{}:subject:{}",
            self.namespace,
            sha256_hex(data_subject_id.as_bytes())
        )
    }
}

fn redis_client(
    redis_url: &str,
    root_certificate: Option<&[u8]>,
    client_certificate: Option<&[u8]>,
    client_private_key: Option<&[u8]>,
) -> Result<redis::Client, RedisProjectionError> {
    let tls_fields = [
        root_certificate.is_some(),
        client_certificate.is_some(),
        client_private_key.is_some(),
    ];
    if tls_fields.iter().any(|present| *present) && !tls_fields.iter().all(|present| *present) {
        return Err(RedisProjectionError::Configuration(
            "redis_mtls_material_incomplete",
        ));
    }
    let uses_tls = Url::parse(redis_url)
        .map_err(|_| RedisProjectionError::Configuration("invalid_redis_url"))?
        .scheme()
        == "rediss";
    if uses_tls {
        let (root_certificate, client_certificate, client_private_key) = (
            root_certificate.ok_or(RedisProjectionError::Configuration(
                "redis_mtls_material_required",
            ))?,
            client_certificate.ok_or(RedisProjectionError::Configuration(
                "redis_mtls_material_required",
            ))?,
            client_private_key.ok_or(RedisProjectionError::Configuration(
                "redis_mtls_material_required",
            ))?,
        );
        redis::Client::build_with_tls(
            redis_url,
            redis::TlsCertificates {
                client_tls: Some(redis::ClientTlsConfig {
                    client_cert: client_certificate.to_vec(),
                    client_key: client_private_key.to_vec(),
                }),
                root_cert: Some(root_certificate.to_vec()),
            },
        )
        .map_err(|_| RedisProjectionError::Configuration("invalid_redis_tls_material"))
    } else {
        redis::Client::open(redis_url)
            .map_err(|_| RedisProjectionError::Configuration("invalid_redis_url"))
    }
}
