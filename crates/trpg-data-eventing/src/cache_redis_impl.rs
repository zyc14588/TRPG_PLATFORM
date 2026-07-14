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
