
impl CacheCryptography {
    fn new(key_reference: &str, encryption_key: &[u8]) -> Result<Self, RedisProjectionError> {
        if encryption_key.len() != 32 {
            return Err(RedisProjectionError::Configuration(
                "redis_cache_key_must_be_32_bytes",
            ));
        }
        let cipher = PayloadCipher::new(key_reference, encryption_key)
            .map_err(|_| RedisProjectionError::Configuration("invalid_redis_cache_key"))?;
        let mut digest_key = Zeroizing::new([0_u8; 32]);
        let mut derivation = Sha256::new();
        derivation.update(b"trpg.redis.cache.digest.v1");
        derivation.update(encryption_key);
        digest_key.copy_from_slice(&derivation.finalize());
        Ok(Self { cipher, digest_key })
    }

    fn digest(&self, plaintext: &[u8], aad: &[&str]) -> Result<String, RedisProjectionError> {
        let mut mac = HmacSha256::new_from_slice(self.digest_key.as_slice())
            .map_err(|_| RedisProjectionError::Cryptography)?;
        for field in aad {
            let length =
                u32::try_from(field.len()).map_err(|_| RedisProjectionError::Cryptography)?;
            mac.update(&length.to_be_bytes());
            mac.update(field.as_bytes());
        }
        mac.update(plaintext);
        Ok(format!(
            "hmac-sha256:{}",
            hex_bytes(&mac.finalize().into_bytes())
        ))
    }
}

fn validate_redis_url(redis_url: &str) -> Result<(), RedisProjectionError> {
    let url = Url::parse(redis_url)
        .map_err(|_| RedisProjectionError::Configuration("invalid_redis_url"))?;
    let host = url
        .host_str()
        .ok_or(RedisProjectionError::Configuration("redis_host_required"))?;
    let local = matches!(host, "localhost" | "127.0.0.1" | "::1");
    if local && matches!(url.scheme(), "redis" | "rediss") {
        return Ok(());
    }
    if !local && url.scheme() == "rediss" {
        return Ok(());
    }
    Err(RedisProjectionError::Configuration(
        "remote_redis_requires_tls",
    ))
}

fn normalize_entry(
    entry: &ProjectionCacheEntry,
) -> Result<ProjectionCacheEntry, RedisProjectionError> {
    validate_name(&entry.key, "cache_key_required")?;
    validate_name(&entry.campaign_id, "campaign_id_required")?;
    validate_name(&entry.data_subject_id, "data_subject_required")?;
    if entry.version < 0 {
        return Err(RedisProjectionError::InvalidEntry(
            "non_negative_version_required",
        ));
    }
    if entry.ttl_seconds == 0 || entry.ttl_seconds > 86_400 {
        return Err(RedisProjectionError::InvalidEntry("invalid_ttl"));
    }
    let subject =
        (entry.visibility_subject != "not_applicable").then_some(entry.visibility_subject.as_str());
    Visibility::try_from_parts(&entry.visibility_label, subject)
        .map_err(|_| RedisProjectionError::InvalidEntry("visibility_subject_mismatch"))?;
    if !matches!(
        entry.provenance_kind.as_str(),
        "user_statement"
            | "human_keeper_statement"
            | "rules_engine_decision"
            | "tool_result"
            | "agent_proposal"
            | "imported_source"
            | "system_fixture"
    ) || entry.provenance_reference.trim().is_empty()
    {
        return Err(RedisProjectionError::InvalidEntry("provenance_required"));
    }
    if entry.value_json.len() > MAX_CACHE_VALUE_BYTES {
        return Err(RedisProjectionError::InvalidEntry("value_too_large"));
    }
    let value: Value = serde_json::from_str(&entry.value_json)
        .map_err(|_| RedisProjectionError::InvalidEntry("value_must_be_json"))?;
    Ok(ProjectionCacheEntry {
        key: entry.key.clone(),
        campaign_id: entry.campaign_id.clone(),
        data_subject_id: entry.data_subject_id.clone(),
        version: entry.version,
        visibility_label: entry.visibility_label.clone(),
        visibility_subject: entry.visibility_subject.clone(),
        provenance_kind: entry.provenance_kind.clone(),
        provenance_reference: entry.provenance_reference.clone(),
        value_json: serde_json::to_string(&value)
            .map_err(|_| RedisProjectionError::InvalidEntry("value_must_be_json"))?,
        ttl_seconds: entry.ttl_seconds,
    })
}

fn validate_stored(
    stored: &StoredProjectionCacheEntry,
    subject_index_key: &str,
) -> Result<(), RedisProjectionError> {
    if stored.schema_version != CACHE_SCHEMA_VERSION
        || stored.version < 0
        || stored.ttl_seconds == 0
        || stored.ttl_seconds > 86_400
        || stored.entry_digest.len() != 76
        || !stored.entry_digest.starts_with("hmac-sha256:")
        || !stored.entry_digest[12..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || stored.subject_index_key != subject_index_key
    {
        return Err(RedisProjectionError::InvalidEntry("stored_entry_invalid"));
    }
    Ok(())
}

fn validate_name(value: &str, reason: &'static str) -> Result<(), RedisProjectionError> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_whitespace) {
        Err(RedisProjectionError::InvalidEntry(reason))
    } else {
        Ok(())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_bytes(&Sha256::digest(bytes))
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private_entry() -> ProjectionCacheEntry {
        ProjectionCacheEntry::new(
            "campaign:one:clues",
            "campaign_one",
            "player_one",
            2,
            "keeper_only",
            "not_applicable",
            "rules_engine_decision",
            "decision_secret",
            r#"{"clue":"harbor ledger"}"#,
            60,
        )
        .unwrap()
    }

    #[test]
    fn remote_plaintext_redis_is_rejected() {
        assert_eq!(
            validate_redis_url("redis://cache.example.invalid/"),
            Err(RedisProjectionError::Configuration(
                "remote_redis_requires_tls"
            ))
        );
        assert!(validate_redis_url("rediss://cache.example.invalid/").is_ok());
    }

    #[test]
    fn protected_cache_encoding_contains_no_value_or_sensitive_metadata() {
        let cache_crypto = CacheCryptography::new("redis-cache-test", &[0x42; 32]).unwrap();
        let namespace = "cache:test";
        let entry = private_entry();
        let version = entry.version().to_string();
        let ttl = entry.ttl_seconds().to_string();
        let subject_index_key = format!(
            "{namespace}:subject:{}",
            sha256_hex(entry.data_subject_id().as_bytes())
        );
        let protected = ProtectedProjectionCacheEntry {
            key: entry.key().to_owned(),
            campaign_id: entry.campaign_id().to_owned(),
            data_subject_id: entry.data_subject_id().to_owned(),
            visibility_label: entry.visibility_label().to_owned(),
            visibility_subject: entry.visibility_subject().to_owned(),
            provenance_kind: entry.provenance_kind().to_owned(),
            provenance_reference: entry.provenance_reference().to_owned(),
            value_json: entry.value_json().to_owned(),
        };
        let plaintext = serde_json::to_vec(&protected).unwrap();
        let aad = [
            "redis_projection_cache_v1",
            namespace,
            version.as_str(),
            ttl.as_str(),
            subject_index_key.as_str(),
        ];
        let envelope = cache_crypto.cipher.encrypt_json(&plaintext, &aad).unwrap();
        let entry_digest = cache_crypto.digest(&plaintext, &aad).unwrap();
        let stored = StoredProjectionCacheEntry {
            schema_version: CACHE_SCHEMA_VERSION,
            version: entry.version(),
            ttl_seconds: entry.ttl_seconds(),
            subject_index_key,
            entry_digest,
            protected_entry: envelope,
        };
        let encoded = serde_json::to_string(&stored).unwrap();
        assert!(!encoded.contains("harbor ledger"));
        assert!(!encoded.contains("player_one"));
        assert!(!encoded.contains("campaign_one"));
        assert!(!encoded.contains("keeper_only"));
        assert!(!encoded.contains("decision_secret"));
        let debug = format!("{entry:?}");
        assert!(!debug.contains("harbor ledger"));
        assert!(!debug.contains("player_one"));
        assert!(!debug.contains("keeper_only"));
        assert!(!debug.contains("decision_secret"));
    }
}
