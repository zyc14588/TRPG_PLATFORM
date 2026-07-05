crate::define_data_event_module!(
    RedisCachePresenceCommand,
    RedisCachePresenceOperation,
    append_redis_cache_presence_event,
    "CODEX-0596-06-DATA-EVENTING-526d27d67f",
    "redis_cache_presence",
    "RedisCachePresenceRecorded",
    "data_eventing.redis_cache_presence.event_schema",
    crate::DataEventOperation::CacheWrite,
    ["redis_cache", "presence_cache", "short_lock_read_model"]
);
