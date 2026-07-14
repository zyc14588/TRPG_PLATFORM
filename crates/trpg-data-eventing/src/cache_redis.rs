crate::define_data_event_module!(
    CacheRedisCommand,
    CacheRedisOperation,
    append_cache_redis_event,
    "cache_redis",
    "CacheRedisEventRecorded",
    "data_eventing.cache_redis.event_schema",
    crate::DataEventOperation::CacheWrite,
    ["redis_cache", "presence_cache"]
);
