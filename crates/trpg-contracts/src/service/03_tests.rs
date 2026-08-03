mod tests {
    use super::*;
    use std::io::ErrorKind;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::oneshot;

    struct TestServer {
        address: SocketAddr,
        shutdown: Option<oneshot::Sender<()>>,
        task: tokio::task::JoinHandle<Result<(), ServiceError>>,
        shutdown_timeout: Duration,
    }

    impl TestServer {
        async fn start(
            probes: Vec<RoleRuntimeProbe>,
            handler: Option<ServiceRequestHandler>,
            limits: ServiceLimits,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let spec = ServiceSpec {
                kind: ServiceKind::AdminServer,
                version: "test",
                bind_address: address,
            };
            let (shutdown, shutdown_rx) = oneshot::channel();
            let task = tokio::spawn(serve_listener(
                spec,
                probes,
                handler.map(Arc::from),
                listener,
                async move {
                    let _ = shutdown_rx.await;
                },
                limits,
            ));
            Self {
                address,
                shutdown: Some(shutdown),
                task,
                shutdown_timeout: limits.graceful_shutdown_timeout,
            }
        }

        async fn stop(mut self) -> Duration {
            let started = time::Instant::now();
            let _ = self.shutdown.take().unwrap().send(());
            let result = time::timeout(
                self.shutdown_timeout + Duration::from_secs(1),
                self.task,
            )
            .await
            .expect("service exceeded its shutdown deadline")
            .expect("service task panicked");
            result.expect("service returned an error");
            started.elapsed()
        }
    }

    fn test_limits() -> ServiceLimits {
        ServiceLimits {
            max_connections: 128,
            max_handlers: 16,
            max_header_bytes: 512,
            max_body_bytes: 512,
            header_timeout: Duration::from_millis(300),
            body_timeout: Duration::from_millis(300),
            idle_timeout: Duration::from_millis(150),
            handler_timeout: Duration::from_millis(1_500),
            write_timeout: Duration::from_millis(500),
            readiness_timeout: Duration::from_millis(100),
            readiness_cache_ttl: Duration::from_millis(20),
            graceful_shutdown_timeout: Duration::from_millis(500),
        }
    }

    async fn exchange(address: SocketAddr, request: &[u8]) -> Vec<u8> {
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(request).await.unwrap();
        stream.shutdown().await.unwrap();
        read_until_close(&mut stream).await
    }

    async fn read_until_close(stream: &mut TcpStream) -> Vec<u8> {
        let mut response = Vec::new();
        match time::timeout(Duration::from_secs(2), stream.read_to_end(&mut response)).await {
            Ok(Ok(_)) => response,
            Ok(Err(error)) if error.kind() == ErrorKind::ConnectionReset => response,
            Ok(Err(error)) => panic!("failed to read response: {error}"),
            Err(_) => panic!("connection did not close before the test deadline"),
        }
    }

    fn assert_status(response: &[u8], status: u16) {
        let response = std::str::from_utf8(response).unwrap();
        assert!(
            response.starts_with(&format!("HTTP/1.1 {status} ")),
            "unexpected response: {response}"
        );
    }

    async fn wait_for_flag(flag: &AtomicBool) {
        for _ in 0..100 {
            if flag.load(Ordering::Acquire) {
                return;
            }
            time::sleep(Duration::from_millis(5)).await;
        }
        panic!("timed out waiting for handler entry");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn one_slow_handler_does_not_block_one_hundred_health_and_api_requests() {
        let slow_entered = Arc::new(AtomicBool::new(false));
        let handler_flag = Arc::clone(&slow_entered);
        let handler: ServiceRequestHandler = Box::new(move |request| match request.path.as_str() {
            "/slow" => {
                handler_flag.store(true, Ordering::Release);
                thread::sleep(Duration::from_millis(800));
                Some(HttpResponse::json(200, json!({"slow": true})))
            }
            "/api" => Some(HttpResponse::json(200, json!({"api": true}))),
            _ => None,
        });
        let server = TestServer::start(Vec::new(), Some(handler), test_limits()).await;
        let address = server.address;
        let slow =
            tokio::spawn(async move { exchange(address, b"GET /slow HTTP/1.1\r\n\r\n").await });
        wait_for_flag(&slow_entered).await;

        let started = time::Instant::now();
        let mut requests = Vec::new();
        for index in 0..100 {
            let path = if index % 2 == 0 {
                "/health/live"
            } else {
                "/api"
            };
            let address = server.address;
            let request = format!("GET {path} HTTP/1.1\r\n\r\n").into_bytes();
            requests.push(tokio::spawn(async move {
                exchange(address, &request).await
            }));
        }
        for request in requests {
            assert_status(&request.await.unwrap(), 200);
        }
        assert!(
            !slow.is_finished(),
            "the slow handler completed before the independent requests"
        );
        assert!(
            started.elapsed() < Duration::from_millis(700),
            "independent requests followed the slow handler's latency"
        );
        assert_status(&slow.await.unwrap(), 200);
        server.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn slowloris_and_oversized_requests_release_bounded_connections() {
        let mut limits = test_limits();
        limits.max_connections = 2;
        limits.max_header_bytes = 128;
        limits.max_body_bytes = 32;
        limits.header_timeout = Duration::from_millis(220);
        limits.body_timeout = Duration::from_millis(220);
        limits.idle_timeout = Duration::from_millis(90);
        let server = TestServer::start(Vec::new(), None, limits).await;

        let mut slow_one = TcpStream::connect(server.address).await.unwrap();
        slow_one
            .write_all(b"GET /health/live HTTP/1.1\r\nX-Test: ")
            .await
            .unwrap();
        time::sleep(Duration::from_millis(10)).await;
        let mut slow_two = TcpStream::connect(server.address).await.unwrap();
        slow_two
            .write_all(b"GET /health/live HTTP/1.1\r\nX-Test: ")
            .await
            .unwrap();
        time::sleep(Duration::from_millis(10)).await;

        let queued = tokio::spawn(exchange(
            server.address,
            b"GET /health/live HTTP/1.1\r\n\r\n",
        ));
        time::sleep(Duration::from_millis(30)).await;
        assert!(
            !queued.is_finished(),
            "connection limit admitted more than two active connections"
        );
        assert_status(&queued.await.unwrap(), 200);
        assert!(read_until_close(&mut slow_one).await.is_empty());
        assert!(read_until_close(&mut slow_two).await.is_empty());

        let oversized_header = format!(
            "GET /health/live HTTP/1.1\r\nX-Oversized: {}\r\n\r\n",
            "a".repeat(limits.max_header_bytes)
        );
        assert!(
            exchange(server.address, oversized_header.as_bytes())
                .await
                .is_empty()
        );
        let oversized_body = format!(
            "POST /api HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            limits.max_body_bytes + 1
        );
        assert!(
            exchange(server.address, oversized_body.as_bytes())
                .await
                .is_empty()
        );

        let mut incomplete_body = TcpStream::connect(server.address).await.unwrap();
        incomplete_body
            .write_all(b"POST /api HTTP/1.1\r\nContent-Length: 4\r\n\r\nx")
            .await
            .unwrap();
        assert!(read_until_close(&mut incomplete_body).await.is_empty());
        server.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn header_and_body_deadlines_stop_trickling_clients() {
        let mut limits = test_limits();
        limits.header_timeout = Duration::from_millis(120);
        limits.body_timeout = Duration::from_millis(120);
        limits.idle_timeout = Duration::from_millis(500);
        let server = TestServer::start(Vec::new(), None, limits).await;

        let mut header_trickle = TcpStream::connect(server.address).await.unwrap();
        for byte in b"GET /health/live HTTP/1.1" {
            if header_trickle.write_all(&[*byte]).await.is_err() {
                break;
            }
            time::sleep(Duration::from_millis(20)).await;
        }
        assert!(read_until_close(&mut header_trickle).await.is_empty());

        let mut body_trickle = TcpStream::connect(server.address).await.unwrap();
        body_trickle
            .write_all(b"POST /api HTTP/1.1\r\nContent-Length: 16\r\n\r\n")
            .await
            .unwrap();
        for byte in b"trickling-body!!" {
            if body_trickle.write_all(&[*byte]).await.is_err() {
                break;
            }
            time::sleep(Duration::from_millis(20)).await;
        }
        assert!(read_until_close(&mut body_trickle).await.is_empty());
        server.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn slow_probe_is_cached_and_slow_handlers_remain_limited() {
        let probe_calls = Arc::new(AtomicUsize::new(0));
        let probe_counter = Arc::clone(&probe_calls);
        let probe = RoleRuntimeProbe::spawn("slow_dependency", move || {
            probe_counter.fetch_add(1, Ordering::AcqRel);
            thread::sleep(Duration::from_millis(500));
            Ok("ready".to_owned())
        })
        .unwrap();

        let handler_calls = Arc::new(AtomicUsize::new(0));
        let handler_counter = Arc::clone(&handler_calls);
        let handler: ServiceRequestHandler = Box::new(move |_| {
            handler_counter.fetch_add(1, Ordering::AcqRel);
            thread::sleep(Duration::from_millis(400));
            Some(HttpResponse::json(200, json!({"late": true})))
        });
        let mut limits = test_limits();
        limits.max_handlers = 1;
        limits.handler_timeout = Duration::from_millis(80);
        limits.readiness_timeout = Duration::from_millis(60);
        limits.graceful_shutdown_timeout = Duration::from_millis(250);
        let server = TestServer::start(vec![probe], Some(handler), limits).await;
        time::sleep(Duration::from_millis(150)).await;

        let started = time::Instant::now();
        let ready = exchange(
            server.address,
            b"GET /health/ready HTTP/1.1\r\n\r\n",
        )
        .await;
        assert_status(&ready, 503);
        assert!(started.elapsed() < Duration::from_millis(100));
        assert_eq!(
            probe_calls.load(Ordering::Acquire),
            1,
            "timed-out dependency checks must not queue without bound"
        );
        assert_status(
            &exchange(
                server.address,
                b"GET /health/live HTTP/1.1\r\n\r\n",
            )
            .await,
            200,
        );

        let first = exchange(server.address, b"GET /api HTTP/1.1\r\n\r\n").await;
        assert_status(&first, 503);
        let second = exchange(server.address, b"GET /api HTTP/1.1\r\n\r\n").await;
        assert_status(&second, 503);
        assert_eq!(
            handler_calls.load(Ordering::Acquire),
            1,
            "handler timeout released the concurrency permit too early"
        );
        assert!(
            std::str::from_utf8(&second)
                .unwrap()
                .contains(WireErrorCode::ServiceInitializationFailed.as_str())
        );
        server.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn graceful_shutdown_cancels_slow_clients_and_stops_accepting() {
        let mut limits = test_limits();
        limits.header_timeout = Duration::from_secs(2);
        limits.idle_timeout = Duration::from_secs(2);
        limits.graceful_shutdown_timeout = Duration::from_millis(250);
        let server = TestServer::start(Vec::new(), None, limits).await;
        let address = server.address;
        let mut slow_client = TcpStream::connect(address).await.unwrap();
        slow_client
            .write_all(b"GET /health/live HTTP/1.1\r\nX-Test: ")
            .await
            .unwrap();
        time::sleep(Duration::from_millis(20)).await;

        let elapsed = server.stop().await;
        assert!(
            elapsed < limits.graceful_shutdown_timeout,
            "shutdown exceeded the configured deadline: {elapsed:?}"
        );
        assert!(read_until_close(&mut slow_client).await.is_empty());
        assert!(
            TcpStream::connect(address).await.is_err(),
            "listener accepted a connection after shutdown"
        );
    }

    #[test]
    fn final_handler_reference_is_released_outside_the_service_runtime() {
        struct DropContextProbe(Arc<AtomicUsize>);

        impl Drop for DropContextProbe {
            fn drop(&mut self) {
                let context = usize::from(tokio::runtime::Handle::try_current().is_ok()) + 1;
                self.0.store(context, Ordering::Release);
            }
        }

        let drop_context = Arc::new(AtomicUsize::new(0));
        let probe = DropContextProbe(Arc::clone(&drop_context));
        let handler: ServiceRequestHandler = Box::new(move |_| {
            let _ = &probe;
            None
        });
        let retained_handler: SharedRequestHandler = Arc::from(handler);
        let runtime_handler = Arc::clone(&retained_handler);
        let runtime = tokio::runtime::Runtime::new().unwrap();

        runtime.block_on(async move { drop(runtime_handler) });
        assert_eq!(drop_context.load(Ordering::Acquire), 0);
        runtime.shutdown_timeout(Duration::from_millis(100));
        drop(retained_handler);
        assert_eq!(drop_context.load(Ordering::Acquire), 1);
    }
}
