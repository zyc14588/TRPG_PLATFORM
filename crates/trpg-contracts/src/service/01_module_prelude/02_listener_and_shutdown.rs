async fn serve_listener<F>(
    spec: ServiceSpec,
    runtime_probes: Vec<RoleRuntimeProbe>,
    handler: Option<SharedRequestHandler>,
    listener: TcpListener,
    shutdown: F,
    limits: ServiceLimits,
) -> Result<(), ServiceError>
where
    F: Future<Output = ()> + Send,
{
    let local_address = listener.local_addr().map_err(io_service_error)?;
    let mut component_checks = vec![ComponentCheck::passing(
        "configuration",
        format!("bind={}", spec.bind_address),
    )];
    component_checks.push(match validate_event_registry() {
        Ok(()) => ComponentCheck::passing(
            "event_registry",
            format!("events={}", crate::canonical_event_registry().len()),
        ),
        Err(error) => ComponentCheck::failing("event_registry", error.to_string()),
    });
    component_checks.push(ComponentCheck::passing(
        "listener",
        format!("local={local_address}"),
    ));

    let probes = Arc::new(runtime_probes);
    let mut starting_checks = component_checks.clone();
    starting_checks.extend(probes.iter().map(|probe| {
        ComponentCheck::failing(probe.name, "runtime loop readiness check pending")
    }));
    let health = Arc::new(RwLock::new(HealthState::new(
        spec.kind,
        spec.version,
        ServicePhase::Starting,
        starting_checks,
    )));
    let serving = Arc::new(AtomicBool::new(true));
    let (cancellation, cancellation_rx) = watch::channel(false);
    let readiness_task = tokio::spawn(readiness_refresh_loop(
        spec.clone(),
        component_checks,
        Arc::clone(&probes),
        Arc::clone(&health),
        cancellation_rx.clone(),
        limits,
    ));
    let connection_slots = Arc::new(Semaphore::new(limits.max_connections));
    let handler_slots = Arc::new(Semaphore::new(limits.max_handlers));
    let mut connections = JoinSet::new();
    let mut accept_error = None;
    tokio::pin!(shutdown);

    eprintln!(
        "service={} state={} listening={local_address}",
        spec.kind.as_str(),
        ServicePhase::Starting.as_str(),
    );

    loop {
        while let Some(result) = connections.try_join_next() {
            if let Err(error) = result {
                eprintln!(
                    "service={} connection_task_error={error}",
                    spec.kind.as_str()
                );
            }
        }

        let connection_permit = tokio::select! {
            _ = &mut shutdown => break,
            permit = Arc::clone(&connection_slots).acquire_owned() => {
                match permit {
                    Ok(permit) => permit,
                    Err(_) => break,
                }
            }
        };
        let accepted = tokio::select! {
            _ = &mut shutdown => {
                drop(connection_permit);
                break;
            }
            accepted = listener.accept() => accepted,
        };
        match accepted {
            Ok((stream, peer)) => {
                let connection_health = Arc::clone(&health);
                let connection_serving = Arc::clone(&serving);
                let connection_handler = handler.clone();
                let connection_handler_slots = Arc::clone(&handler_slots);
                let mut connection_cancellation = cancellation_rx.clone();
                let service_name = spec.kind.as_str();
                connections.spawn(async move {
                    let result = serve_connection(
                        stream,
                        connection_health,
                        connection_serving,
                        connection_handler,
                        connection_handler_slots,
                        connection_permit,
                        &mut connection_cancellation,
                        limits,
                    )
                    .await;
                    if let Err(error) = result {
                        eprintln!(
                            "service={service_name} peer={peer} connection_error={} detail={}",
                            error.code, error.detail
                        );
                    }
                });
            }
            Err(error) => {
                drop(connection_permit);
                accept_error = Some(io_service_error(error));
                break;
            }
        }
    }

    drop(listener);
    serving.store(false, Ordering::Release);
    let _ = cancellation.send(true);
    for probe in probes.iter() {
        probe.request_shutdown();
    }
    readiness_task.abort();
    let _ = readiness_task.await;

    let shutdown_deadline = time::Instant::now() + limits.graceful_shutdown_timeout;
    let drain_connections = async {
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                eprintln!(
                    "service={} connection_task_error={error}",
                    spec.kind.as_str()
                );
            }
        }
    };
    if time::timeout_at(shutdown_deadline, drain_connections)
        .await
        .is_err()
    {
        connections.abort_all();
        let _ = time::timeout_at(shutdown_deadline, connections.shutdown()).await;
    }

    eprintln!("service={} state=stopped", spec.kind.as_str());
    match accept_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
