#[allow(clippy::too_many_arguments)]
async fn serve_connection(
    mut stream: TcpStream,
    health: SharedHealth,
    serving: Arc<AtomicBool>,
    handler: Option<SharedRequestHandler>,
    handler_slots: Arc<Semaphore>,
    _connection_permit: OwnedSemaphorePermit,
    cancellation: &mut watch::Receiver<bool>,
    limits: ServiceLimits,
) -> Result<(), ServiceError> {
    tokio::select! {
        _ = cancellation.changed() => Ok(()),
        result = handle_connection(
            &mut stream,
            &health,
            &serving,
            handler,
            handler_slots,
            limits,
        ) => result,
    }
}

async fn readiness_refresh_loop(
    spec: ServiceSpec,
    component_checks: Vec<ComponentCheck>,
    runtime_probes: Arc<Vec<RoleRuntimeProbe>>,
    health: SharedHealth,
    mut cancellation: watch::Receiver<bool>,
    limits: ServiceLimits,
) {
    loop {
        if *cancellation.borrow() {
            break;
        }
        let refresh_spec = spec.clone();
        let refresh_checks = component_checks.clone();
        let refresh_probes = Arc::clone(&runtime_probes);
        let refresh = tokio::task::spawn_blocking(move || {
            current_health(
                &refresh_spec,
                &refresh_checks,
                &refresh_probes,
                limits.readiness_timeout,
            )
        });
        let refreshed = tokio::select! {
            _ = cancellation.changed() => break,
            result = refresh => result,
        };
        let next_health = match refreshed {
            Ok(health) => health,
            Err(error) => readiness_failure_health(
                &spec,
                &component_checks,
                &runtime_probes,
                format!("readiness refresh failed: {error}"),
            ),
        };
        replace_health(&health, next_health);

        tokio::select! {
            _ = cancellation.changed() => break,
            _ = time::sleep(limits.readiness_cache_ttl) => {}
        }
    }
}

fn current_health(
    spec: &ServiceSpec,
    component_checks: &[ComponentCheck],
    runtime_probes: &[RoleRuntimeProbe],
    readiness_timeout: Duration,
) -> HealthState {
    let mut checks = component_checks.to_vec();
    checks.extend(
        runtime_probes
            .iter()
            .map(|probe| probe.component_check_with_timeout(readiness_timeout)),
    );
    let phase = if checks.iter().all(|check| check.ready) {
        ServicePhase::Ready
    } else {
        ServicePhase::Degraded
    };
    HealthState::new(spec.kind, spec.version, phase, checks)
}

fn readiness_failure_health(
    spec: &ServiceSpec,
    component_checks: &[ComponentCheck],
    runtime_probes: &[RoleRuntimeProbe],
    detail: String,
) -> HealthState {
    let mut checks = component_checks.to_vec();
    if runtime_probes.is_empty() {
        checks.push(ComponentCheck::failing("readiness_refresh", detail));
    } else {
        checks.extend(
            runtime_probes
                .iter()
                .map(|probe| ComponentCheck::failing(probe.name, detail.clone())),
        );
    }
    HealthState::new(spec.kind, spec.version, ServicePhase::Degraded, checks)
}

fn replace_health(cache: &SharedHealth, health: HealthState) {
    match cache.write() {
        Ok(mut current) => *current = health,
        Err(poisoned) => *poisoned.into_inner() = health,
    }
}

fn health_snapshot(cache: &SharedHealth) -> HealthState {
    match cache.read() {
        Ok(health) => health.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

async fn handle_connection(
    stream: &mut TcpStream,
    health: &SharedHealth,
    serving: &AtomicBool,
    handler: Option<SharedRequestHandler>,
    handler_slots: Arc<Semaphore>,
    limits: ServiceLimits,
) -> Result<(), ServiceError> {
    let request = read_http_request(stream, limits).await?;
    let health = health_snapshot(health);
    let response = match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/health/live") => {
            let serving = serving.load(Ordering::Acquire);
            HttpResponse::json(
                if health.live(serving) { 200 } else { 503 },
                health.live_document(serving),
            )
        }
        ("GET", "/health/ready") => HttpResponse::json(
            if health.ready() { 200 } else { 503 },
            health.ready_document(),
        ),
        _ => {
            let is_get = request.method == "GET";
            match handler {
                Some(handler) => {
                    match invoke_handler(handler, request, handler_slots, limits).await {
                        Ok(Some(response)) => response,
                        Ok(None) => default_response(is_get),
                        Err(_) => HttpResponse::json(
                            503,
                            json!({
                                "error": WireErrorCode::ServiceInitializationFailed.as_str()
                            }),
                        ),
                    }
                }
                None => default_response(is_get),
            }
        }
    };
    write_json_response(stream, response.status, &response.body.to_string(), limits).await
}

async fn invoke_handler(
    handler: SharedRequestHandler,
    request: HttpRequest,
    handler_slots: Arc<Semaphore>,
    limits: ServiceLimits,
) -> Result<Option<HttpResponse>, ServiceError> {
    let deadline = time::Instant::now() + limits.handler_timeout;
    let permit = time::timeout_at(deadline, handler_slots.acquire_owned())
        .await
        .map_err(|_| timeout_service_error("HTTP handler capacity timed out"))?
        .map_err(|_| timeout_service_error("HTTP handler capacity is closed"))?;
    let task = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        handler(&request)
    });
    time::timeout_at(deadline, task)
        .await
        .map_err(|_| timeout_service_error("HTTP handler timed out"))?
        .map_err(|error| ServiceError {
            code: WireErrorCode::ServiceInitializationFailed,
            detail: format!("HTTP handler task failed: {error}"),
        })
}

fn default_response(is_get: bool) -> HttpResponse {
    if is_get {
        HttpResponse::json(404, json!({"error": "NOT_FOUND"}))
    } else {
        HttpResponse::json(405, json!({"error": "METHOD_NOT_ALLOWED"}))
    }
}

fn timeout_service_error(detail: &str) -> ServiceError {
    ServiceError {
        code: WireErrorCode::ServiceInitializationFailed,
        detail: detail.to_owned(),
    }
}
