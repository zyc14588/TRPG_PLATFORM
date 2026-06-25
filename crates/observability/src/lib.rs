use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServiceHealth {
    pub service: String,
    pub status: String,
}

pub fn prometheus_bootstrap_metrics() -> &'static str {
    "# HELP trpg_phase0_info Phase 0 bootstrap marker\n# TYPE trpg_phase0_info gauge\ntrpg_phase0_info 1\n"
}
