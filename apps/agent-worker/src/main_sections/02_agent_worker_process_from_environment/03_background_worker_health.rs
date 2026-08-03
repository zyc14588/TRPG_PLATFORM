#[derive(Debug, Default)]
struct BackgroundWorkerHealth {
    last_cycle_completed_at: Option<Instant>,
    cycle_error: Option<String>,
    stopped_error: Option<&'static str>,
}

impl BackgroundWorkerHealth {
    fn record_cycle(&mut self, completed_at: Instant, cycle_error: Option<String>) {
        self.last_cycle_completed_at = Some(completed_at);
        self.cycle_error = cycle_error;
        self.stopped_error = None;
    }

    fn record_stopped(&mut self, error: &'static str) {
        self.stopped_error = Some(error);
    }

    fn readiness_error(&self, now: Instant, stale_after: Duration) -> Option<String> {
        if let Some(error) = self.stopped_error {
            return Some(error.to_owned());
        }
        let Some(completed_at) = self.last_cycle_completed_at else {
            return Some("AGENT_WORKER_DEPENDENCY_CHECK_PENDING".to_owned());
        };
        if now.saturating_duration_since(completed_at) > stale_after {
            return Some("AGENT_WORKER_BACKGROUND_HEARTBEAT_STALE".to_owned());
        }
        self.cycle_error.clone()
    }
}
