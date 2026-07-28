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
use std::sync::Arc;

use crate::event_store_sqlx_outbox_projection::PayloadCipher;
use hmac::{Hmac, Mac};
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use trpg_identity::ReplayAuthorization;
use trpg_shared_kernel::{EntityId, Visibility};
use url::Url;
use zeroize::{Zeroize, Zeroizing};

type HmacSha256 = Hmac<Sha256>;

const CACHE_SCHEMA_VERSION: u16 = 1;
const MAX_CACHE_VALUE_BYTES: usize = 1_048_576;

const MONOTONIC_PROJECTION_SCRIPT: &str = r#"
local function extend_ttl(key, proposed)
    local current_ttl = redis.call('TTL', key)
    local proposed_ttl = tonumber(proposed)
    if current_ttl < proposed_ttl then
        redis.call('EXPIRE', key, proposed_ttl)
    end
end

local current = redis.call('GET', KEYS[1])
if current then
    local decoded = cjson.decode(current)
    local current_version = tonumber(decoded['version'])
    local proposed_version = tonumber(ARGV[1])
    if current_version > proposed_version then
        return -1
    end
    if current_version == proposed_version then
        if decoded['entry_digest'] ~= ARGV[4] then
            return -2
        end
        extend_ttl(KEYS[1], ARGV[3])
        redis.call('SADD', KEYS[2], KEYS[1])
        extend_ttl(KEYS[2], ARGV[3])
        return 1
    end
    if decoded['subject_index_key'] then
        redis.call('SREM', decoded['subject_index_key'], KEYS[1])
    end
end
redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
redis.call('SADD', KEYS[2], KEYS[1])
extend_ttl(KEYS[2], ARGV[3])
return 1
"#;

const INVALIDATE_ENTRY_SCRIPT: &str = r#"
local current = redis.call('GET', KEYS[1])
if not current then
    return 0
end
local decoded = cjson.decode(current)
if decoded['subject_index_key'] then
    redis.call('SREM', decoded['subject_index_key'], KEYS[1])
end
return redis.call('DEL', KEYS[1])
"#;

const INVALIDATE_SUBJECT_SCRIPT: &str = r#"
local members = redis.call('SMEMBERS', KEYS[1])
local removed = 0
for _, key in ipairs(members) do
    removed = removed + redis.call('DEL', key)
end
redis.call('DEL', KEYS[1])
return removed
"#;

/// A decrypted cache value. Fields are private and Debug redacts the logical
/// key, provenance reference, data subject, and JSON value.
#[derive(Clone, PartialEq, Eq)]
pub struct ProjectionCacheEntry {
    key: String,
    campaign_id: String,
    data_subject_id: String,
    version: i64,
    visibility_label: String,
    visibility_subject: String,
    provenance_kind: String,
    provenance_reference: String,
    value_json: String,
    ttl_seconds: u64,
}

impl fmt::Debug for ProjectionCacheEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectionCacheEntry")
            .field("key", &"[REDACTED]")
            .field("campaign_id", &self.campaign_id)
            .field("data_subject_id", &"[REDACTED]")
            .field("version", &self.version)
            .field("visibility_label", &"[REDACTED]")
            .field("visibility_subject", &"[REDACTED]")
            .field("provenance_kind", &self.provenance_kind)
            .field("provenance_reference", &"[REDACTED]")
            .field("value_json", &"[REDACTED]")
            .field("ttl_seconds", &self.ttl_seconds)
            .finish()
    }
}

impl ProjectionCacheEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: impl Into<String>,
        campaign_id: impl Into<String>,
        data_subject_id: impl Into<String>,
        version: i64,
        visibility_label: impl Into<String>,
        visibility_subject: impl Into<String>,
        provenance_kind: impl Into<String>,
        provenance_reference: impl Into<String>,
        value_json: impl Into<String>,
        ttl_seconds: u64,
    ) -> Result<Self, RedisProjectionError> {
        normalize_entry(&Self {
            key: key.into(),
            campaign_id: campaign_id.into(),
            data_subject_id: data_subject_id.into(),
            version,
            visibility_label: visibility_label.into(),
            visibility_subject: visibility_subject.into(),
            provenance_kind: provenance_kind.into(),
            provenance_reference: provenance_reference.into(),
            value_json: value_json.into(),
            ttl_seconds,
        })
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }

    pub fn data_subject_id(&self) -> &str {
        &self.data_subject_id
    }

    pub const fn version(&self) -> i64 {
        self.version
    }

    pub fn visibility_label(&self) -> &str {
        &self.visibility_label
    }

    pub fn visibility_subject(&self) -> &str {
        &self.visibility_subject
    }

    pub fn provenance_kind(&self) -> &str {
        &self.provenance_kind
    }

    pub fn provenance_reference(&self) -> &str {
        &self.provenance_reference
    }

    pub fn value_json(&self) -> &str {
        &self.value_json
    }

    pub const fn ttl_seconds(&self) -> u64 {
        self.ttl_seconds
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RedisProjectionError {
    Configuration(&'static str),
    Unavailable,
    InvalidEntry(&'static str),
    AuthorizationDenied,
    Cryptography,
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
            Self::AuthorizationDenied => formatter.write_str("redis projection access denied"),
            Self::Cryptography => formatter.write_str("redis projection cryptography failed"),
            Self::VersionRegression => formatter.write_str("projection version regression"),
            Self::VersionCollision => formatter.write_str("projection version collision"),
        }
    }
}

impl std::error::Error for RedisProjectionError {}

struct CacheCryptography {
    cipher: PayloadCipher,
    digest_key: Zeroizing<[u8; 32]>,
}

impl Drop for CacheCryptography {
    fn drop(&mut self) {
        self.digest_key.zeroize();
    }
}

#[derive(Clone)]
pub struct RedisProjectionCache {
    connection: ConnectionManager,
    namespace: String,
    cryptography: Arc<CacheCryptography>,
}

impl fmt::Debug for RedisProjectionCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisProjectionCache")
            .field("connection", &"[REDIS CONNECTION MANAGER]")
            .field("namespace", &self.namespace)
            .field("cryptography", &"[REDACTED]")
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredProjectionCacheEntry {
    schema_version: u16,
    version: i64,
    ttl_seconds: u64,
    subject_index_key: String,
    entry_digest: String,
    protected_entry: Value,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedProjectionCacheEntry {
    key: String,
    campaign_id: String,
    data_subject_id: String,
    visibility_label: String,
    visibility_subject: String,
    provenance_kind: String,
    provenance_reference: String,
    value_json: String,
}
