crate::define_data_event_module!(
    RedisPresenceCommand,
    RedisPresenceOperation,
    append_redis_presence_event,
    "redis_presence",
    "RedisPresenceCached",
    "data_eventing.redis_presence.event_schema",
    crate::DataEventOperation::CacheWrite,
    ["presence_cache", "event_store"]
);

crate::define_data_event_artifacts!(
    RedisPresenceService,
    RedisPresenceRepository,
    RedisPresenceEvent,
    RedisPresenceError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const PRESENCE_CACHE_SCOPE: &str = "rebuildable_presence_cache";
