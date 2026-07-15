crate::define_data_event_module!(
    CacheRedisImplCommand,
    CacheRedisImplOperation,
    append_cache_redis_impl_event,
    "cache_redis_impl",
    "CacheRedisImplRecorded",
    "data_eventing.cache_redis_impl.event_schema",
    crate::DataEventOperation::CacheWrite,
    ["event_store", "redis_cache", "projection_checkpoint"]
);

crate::define_data_event_artifacts!(
    CacheRedisImplService,
    CacheRedisImplRepository,
    CacheRedisImplEvent,
    CacheRedisImplError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const CACHE_REBUILD_SOURCE: &str = crate::EVENT_STORE_TABLE;
pub const CACHE_IS_CANONICAL: bool = false;

use std::fmt;

use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

const MONOTONIC_PROJECTION_SCRIPT: &str = r#"
local current = redis.call('GET', KEYS[1])
if current then
    local decoded = cjson.decode(current)
    local current_version = tonumber(decoded['version'])
    local proposed_version = tonumber(ARGV[1])
    if current_version > proposed_version then
        return -1
    end
    if current_version == proposed_version and current ~= ARGV[2] then
        return -2
    end
end
redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
return 1
"#;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionCacheEntry {
    pub key: String,
    pub version: i64,
    pub visibility_label: String,
    pub visibility_subject: String,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub value_json: String,
    pub ttl_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RedisProjectionError {
    Configuration(&'static str),
    Unavailable,
    InvalidEntry(&'static str),
    VersionRegression,
    VersionCollision,
}

impl fmt::Display for RedisProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(formatter, "redis configuration error: {reason}"),
            Self::Unavailable => formatter.write_str("redis projection cache unavailable"),
            Self::InvalidEntry(reason) => {
                write!(formatter, "invalid projection cache entry: {reason}")
            }
            Self::VersionRegression => formatter.write_str("projection version regression"),
            Self::VersionCollision => formatter.write_str("projection version collision"),
        }
    }
}

impl std::error::Error for RedisProjectionError {}

#[derive(Clone)]
pub struct RedisProjectionCache {
    connection: ConnectionManager,
    namespace: String,
}

impl fmt::Debug for RedisProjectionCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisProjectionCache")
            .field("connection", &"[REDIS CONNECTION MANAGER]")
            .field("namespace", &self.namespace)
            .finish()
    }
}

impl RedisProjectionCache {
    pub async fn connect(redis_url: &str, namespace: &str) -> Result<Self, RedisProjectionError> {
        validate_redis_url(redis_url)?;
        validate_name(namespace, "namespace_required")?;
        let client = redis::Client::open(redis_url)
            .map_err(|_| RedisProjectionError::Configuration("invalid_redis_url"))?;
        let connection = ConnectionManager::new(client)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        let mut cache = Self {
            connection,
            namespace: namespace.to_owned(),
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
        let encoded = serde_json::to_string(&normalized)
            .map_err(|_| RedisProjectionError::InvalidEntry("serialization_failed"))?;
        let mut connection = self.connection.clone();
        let outcome: i64 = redis::Script::new(MONOTONIC_PROJECTION_SCRIPT)
            .key(self.redis_key(&normalized.key))
            .arg(normalized.version)
            .arg(encoded)
            .arg(normalized.ttl_seconds)
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

    pub async fn get(
        &self,
        key: &str,
    ) -> Result<Option<ProjectionCacheEntry>, RedisProjectionError> {
        validate_name(key, "cache_key_required")?;
        let mut connection = self.connection.clone();
        let encoded: Option<String> = redis::cmd("GET")
            .arg(self.redis_key(key))
            .query_async(&mut connection)
            .await
            .map_err(|_| RedisProjectionError::Unavailable)?;
        encoded
            .map(|encoded| {
                let entry: ProjectionCacheEntry = serde_json::from_str(&encoded)
                    .map_err(|_| RedisProjectionError::InvalidEntry("stored_entry_invalid"))?;
                normalize_entry(&entry)
            })
            .transpose()
    }

    pub async fn invalidate(&self, key: &str) -> Result<(), RedisProjectionError> {
        validate_name(key, "cache_key_required")?;
        let mut connection = self.connection.clone();
        redis::cmd("DEL")
            .arg(self.redis_key(key))
            .query_async::<i64>(&mut connection)
            .await
            .map(|_| ())
            .map_err(|_| RedisProjectionError::Unavailable)
    }

    fn redis_key(&self, key: &str) -> String {
        format!("{}:{}", self.namespace, key)
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
    if entry.version < 0 {
        return Err(RedisProjectionError::InvalidEntry(
            "non_negative_version_required",
        ));
    }
    if entry.ttl_seconds == 0 || entry.ttl_seconds > 86_400 {
        return Err(RedisProjectionError::InvalidEntry("invalid_ttl"));
    }
    if !matches!(
        entry.visibility_label.as_str(),
        "public"
            | "party_visible"
            | "keeper_only"
            | "private_to_player"
            | "investigator_private"
            | "ai_internal"
            | "system_only"
            | "system_private"
    ) {
        return Err(RedisProjectionError::InvalidEntry(
            "unknown_visibility_label",
        ));
    }
    let private = matches!(
        entry.visibility_label.as_str(),
        "private_to_player" | "investigator_private"
    );
    if private == (entry.visibility_subject == "not_applicable") {
        return Err(RedisProjectionError::InvalidEntry(
            "visibility_subject_mismatch",
        ));
    }
    if !matches!(
        entry.provenance_kind.as_str(),
        "human_keeper_statement"
            | "rules_engine_decision"
            | "tool_result"
            | "agent_proposal"
            | "imported_source"
            | "system_fixture"
    ) || entry.provenance_reference.trim().is_empty()
    {
        return Err(RedisProjectionError::InvalidEntry("provenance_required"));
    }
    if entry.value_json.len() > 1_048_576 {
        return Err(RedisProjectionError::InvalidEntry("value_too_large"));
    }
    let value: Value = serde_json::from_str(&entry.value_json)
        .map_err(|_| RedisProjectionError::InvalidEntry("value_must_be_json"))?;
    let mut normalized = entry.clone();
    normalized.value_json = serde_json::to_string(&value)
        .map_err(|_| RedisProjectionError::InvalidEntry("value_must_be_json"))?;
    Ok(normalized)
}

fn validate_name(value: &str, reason: &'static str) -> Result<(), RedisProjectionError> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_whitespace) {
        Err(RedisProjectionError::InvalidEntry(reason))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
