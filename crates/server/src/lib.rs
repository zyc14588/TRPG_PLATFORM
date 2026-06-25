use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use observability::prometheus_bootstrap_metrics;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, env, fs, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    pub bind_addr: String,
    pub decision_baseline: String,
    pub region_id: String,
}

#[derive(Debug, Deserialize, Default)]
struct ConfigFile {
    project: Option<ProjectSection>,
}

#[derive(Debug, Deserialize)]
struct ProjectSection {
    decision_baseline: Option<String>,
    region_id: Option<String>,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let path =
            env::var("TRPG_CONFIG_PATH").unwrap_or_else(|_| "config/default.toml".to_owned());
        let parsed = read_config_file(PathBuf::from(path))?;

        Ok(Self {
            bind_addr: env::var("TRPG_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_owned()),
            decision_baseline: parsed
                .project
                .as_ref()
                .and_then(|project| project.decision_baseline.clone())
                .unwrap_or_else(|| "2026-06-25-final".to_owned()),
            region_id: parsed
                .project
                .and_then(|project| project.region_id)
                .unwrap_or_else(|| "local-1".to_owned()),
        })
    }
}

fn read_config_file(path: PathBuf) -> anyhow::Result<ConfigFile> {
    if !path.exists() {
        return Ok(ConfigFile::default());
    }

    let contents = fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: AppConfig,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub decision_baseline: String,
    pub region_id: String,
}

#[derive(Debug, Serialize)]
pub struct ReadyResponse {
    pub status: &'static str,
    pub checks: BTreeMap<&'static str, &'static str>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("service is not ready: {0}")]
    NotReady(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::SERVICE_UNAVAILABLE, self.to_string()).into_response()
    }
}

pub fn router(config: AppConfig) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .with_state(AppState { config })
}

async fn healthz(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "trpg-platform-api",
        decision_baseline: state.config.decision_baseline,
        region_id: state.config.region_id,
    })
}

async fn readyz() -> Json<ReadyResponse> {
    let mut checks = BTreeMap::new();
    checks.insert("database", "not_checked_phase_0");
    checks.insert("redis", "not_checked_phase_0");
    checks.insert("object_storage", "not_checked_phase_0");

    Json(ReadyResponse {
        status: "ready_phase_0",
        checks,
    })
}

async fn metrics() -> &'static str {
    prometheus_bootstrap_metrics()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn healthz_returns_ok() {
        let app = router(AppConfig {
            bind_addr: "127.0.0.1:0".to_owned(),
            decision_baseline: "test".to_owned(),
            region_id: "local-test".to_owned(),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::OK);
    }
}
