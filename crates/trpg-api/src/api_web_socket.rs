crate::define_api_realtime_contract_module!(
    "api_web_socket",
    "ApiWebSocketContractRecorded",
    "api_web_socket.event_schema",
    crate::contract_core::ApiRealtimeOperation::PublishRealtimeDelta
);

use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealtimeLimits {
    pub max_connections: usize,
    pub max_message_bytes: usize,
    pub max_messages_per_window: u32,
    pub rate_window: Duration,
    pub max_pending_events: usize,
    pub replay_page_size: usize,
    pub heartbeat_interval: Duration,
    pub heartbeat_timeout: Duration,
    pub durable_poll_interval: Duration,
    pub reauthorization_interval: Duration,
    pub write_timeout: Duration,
    pub subscribe_timeout: Duration,
}

impl Default for RealtimeLimits {
    fn default() -> Self {
        Self {
            max_connections: 2_048,
            max_message_bytes: 64 * 1_024,
            max_messages_per_window: 120,
            rate_window: Duration::from_secs(60),
            max_pending_events: 100,
            replay_page_size: 100,
            heartbeat_interval: Duration::from_secs(15),
            heartbeat_timeout: Duration::from_secs(45),
            durable_poll_interval: Duration::from_secs(2),
            reauthorization_interval: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),
            subscribe_timeout: Duration::from_secs(10),
        }
    }
}

impl RealtimeLimits {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.max_connections == 0
            || !(1_024..=1_048_576).contains(&self.max_message_bytes)
            || self.max_messages_per_window == 0
            || self.rate_window.is_zero()
            || !(1..=500).contains(&self.max_pending_events)
            || !(1..=500).contains(&self.replay_page_size)
            || self.replay_page_size > self.max_pending_events
            || self.heartbeat_interval.is_zero()
            || self.heartbeat_timeout <= self.heartbeat_interval
            || self.durable_poll_interval.is_zero()
            || self.reauthorization_interval.is_zero()
            || self.write_timeout.is_zero()
            || self.subscribe_timeout.is_zero()
        {
            return Err("REALTIME_LIMITS_INVALID");
        }
        Ok(())
    }
}
