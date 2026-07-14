use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceKind {
    ApiServer,
    RealtimeServer,
    AgentWorker,
    AdminServer,
    MigrationRunner,
}

impl ServiceKind {
    pub const ALL: &'static [Self] = &[
        Self::ApiServer,
        Self::RealtimeServer,
        Self::AgentWorker,
        Self::AdminServer,
        Self::MigrationRunner,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApiServer => "api-server",
            Self::RealtimeServer => "realtime-server",
            Self::AgentWorker => "agent-worker",
            Self::AdminServer => "admin-server",
            Self::MigrationRunner => "migration-runner",
        }
    }

    pub const fn default_port(self) -> u16 {
        match self {
            Self::ApiServer => 8080,
            Self::RealtimeServer => 8081,
            Self::AgentWorker => 8082,
            Self::AdminServer => 8083,
            Self::MigrationRunner => 8084,
        }
    }

    pub const fn bind_environment_key(self) -> &'static str {
        match self {
            Self::ApiServer => "TRPG_API_SERVER_BIND",
            Self::RealtimeServer => "TRPG_REALTIME_SERVER_BIND",
            Self::AgentWorker => "TRPG_AGENT_WORKER_BIND",
            Self::AdminServer => "TRPG_ADMIN_SERVER_BIND",
            Self::MigrationRunner => "TRPG_MIGRATION_RUNNER_BIND",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServicePhase {
    Starting,
    Ready,
    Degraded,
    Stopping,
}

impl ServicePhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Stopping => "stopping",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentCheck {
    pub name: &'static str,
    pub ready: bool,
    pub detail: String,
}

impl ComponentCheck {
    pub fn passing(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            ready: true,
            detail: detail.into(),
        }
    }

    pub fn failing(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            ready: false,
            detail: detail.into(),
        }
    }

    fn document(&self) -> Value {
        json!({
            "name": self.name,
            "status": if self.ready { "pass" } else { "fail" },
            "detail": self.detail
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthState {
    pub service: ServiceKind,
    pub version: &'static str,
    pub phase: ServicePhase,
    pub checks: Vec<ComponentCheck>,
}

impl HealthState {
    pub fn new(
        service: ServiceKind,
        version: &'static str,
        phase: ServicePhase,
        checks: Vec<ComponentCheck>,
    ) -> Self {
        Self {
            service,
            version,
            phase,
            checks,
        }
    }

    pub fn live(&self, serving: bool) -> bool {
        serving && self.phase != ServicePhase::Stopping
    }

    pub fn ready(&self) -> bool {
        self.phase == ServicePhase::Ready && self.checks.iter().all(|check| check.ready)
    }

    pub fn live_document(&self, serving: bool) -> Value {
        json!({
            "status": if self.live(serving) { "live" } else { "stopping" },
            "service": self.service.as_str(),
            "version": self.version,
            "state": self.phase.as_str()
        })
    }

    pub fn ready_document(&self) -> Value {
        json!({
            "status": if self.ready() { "ready" } else { "not_ready" },
            "service": self.service.as_str(),
            "version": self.version,
            "state": self.phase.as_str(),
            "checks": self.checks.iter().map(ComponentCheck::document).collect::<Vec<_>>()
        })
    }
}
