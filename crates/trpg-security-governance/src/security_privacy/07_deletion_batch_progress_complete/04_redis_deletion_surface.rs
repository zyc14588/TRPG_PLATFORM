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
