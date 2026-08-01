use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle as ThreadJoinHandle};
use std::time::{Duration, Instant};

use serde_json::json;
use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;
use tokio::time;

use crate::{
    validate_event_registry, ComponentCheck, HealthState, ServiceKind, ServicePhase, WireErrorCode,
};

const READINESS_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_SHUTDOWN_GRACE: Duration = Duration::from_millis(100);
const MAX_HTTP_HEADER_BYTES: usize = 16_384;
const MAX_HTTP_BODY_BYTES: usize = 65_536;

type ReadinessResult = Result<String, String>;
type SharedHealth = Arc<RwLock<HealthState>>;
type SharedRequestHandler = Arc<ServiceRequestHandlerFn>;
type ShutdownSignal = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Clone, Copy, Debug)]
struct ServiceLimits {
    max_connections: usize,
    max_handlers: usize,
    max_header_bytes: usize,
    max_body_bytes: usize,
    header_timeout: Duration,
    body_timeout: Duration,
    idle_timeout: Duration,
    handler_timeout: Duration,
    write_timeout: Duration,
    readiness_timeout: Duration,
    readiness_cache_ttl: Duration,
    graceful_shutdown_timeout: Duration,
}

impl Default for ServiceLimits {
    fn default() -> Self {
        Self {
            max_connections: 256,
            max_handlers: 64,
            max_header_bytes: MAX_HTTP_HEADER_BYTES,
            max_body_bytes: MAX_HTTP_BODY_BYTES,
            header_timeout: Duration::from_secs(2),
            body_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(1),
            handler_timeout: Duration::from_secs(10),
            write_timeout: Duration::from_secs(2),
            readiness_timeout: Duration::from_secs(2),
            readiness_cache_ttl: Duration::from_millis(500),
            graceful_shutdown_timeout: Duration::from_secs(5),
        }
    }
}

enum RuntimeCommand {
    Check(SyncSender<ReadinessResult>),
    Shutdown,
}

pub struct RoleRuntimeProbe {
    name: &'static str,
    sender: Sender<RuntimeCommand>,
    active: Arc<AtomicBool>,
    checking: Arc<AtomicBool>,
    worker: Option<ThreadJoinHandle<()>>,
}

impl RoleRuntimeProbe {
    pub fn spawn<F>(name: &'static str, check: F) -> Result<Self, ServiceError>
    where
        F: Fn() -> ReadinessResult + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel();
        let active = Arc::new(AtomicBool::new(true));
        let checking = Arc::new(AtomicBool::new(false));
        let worker_active = Arc::clone(&active);
        let worker_checking = Arc::clone(&checking);
        let worker = thread::Builder::new()
            .name(format!("{name}-loop"))
            .spawn(move || {
                role_runtime_loop(receiver, check, worker_active, worker_checking)
            })
            .map_err(|error| ServiceError {
                code: WireErrorCode::ServiceInitializationFailed,
                detail: format!("failed to start {name}: {error}"),
            })?;
        Ok(Self {
            name,
            sender,
            active,
            checking,
            worker: Some(worker),
        })
    }

    pub fn component_check(&self) -> ComponentCheck {
        self.component_check_with_timeout(READINESS_TIMEOUT)
    }

    fn component_check_with_timeout(&self, timeout: Duration) -> ComponentCheck {
        if !self.active.load(Ordering::Acquire) {
            return ComponentCheck::failing(self.name, "runtime loop is not running");
        }
        if self
            .checking
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return ComponentCheck::failing(
                self.name,
                "runtime loop readiness check is already in progress",
            );
        }

        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        if self
            .sender
            .send(RuntimeCommand::Check(reply_sender))
            .is_err()
        {
            self.checking.store(false, Ordering::Release);
            self.active.store(false, Ordering::Release);
            return ComponentCheck::failing(self.name, "runtime loop is not running");
        }
        match reply_receiver.recv_timeout(timeout) {
            Ok(Ok(detail)) => ComponentCheck::passing(self.name, detail),
            Ok(Err(detail)) => ComponentCheck::failing(self.name, detail),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                ComponentCheck::failing(self.name, "runtime loop readiness check timed out")
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                ComponentCheck::failing(self.name, "runtime loop stopped before replying")
            }
        }
    }

    fn request_shutdown(&self) {
        self.active.store(false, Ordering::Release);
        let _ = self.sender.send(RuntimeCommand::Shutdown);
    }

    pub fn shutdown(&mut self) {
        self.request_shutdown();
        let Some(worker) = self.worker.take() else {
            return;
        };
        let deadline = Instant::now() + PROBE_SHUTDOWN_GRACE;
        while !worker.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        if worker.is_finished() {
            let _ = worker.join();
        }
    }
}

impl Drop for RoleRuntimeProbe {
    fn drop(&mut self) {
        self.request_shutdown();
        let _ = self.worker.take();
    }
}

fn role_runtime_loop<F>(
    receiver: Receiver<RuntimeCommand>,
    check: F,
    active: Arc<AtomicBool>,
    checking: Arc<AtomicBool>,
) where
    F: Fn() -> ReadinessResult,
{
    while let Ok(command) = receiver.recv() {
        match command {
            RuntimeCommand::Check(reply) => {
                let result = check();
                checking.store(false, Ordering::Release);
                let _ = reply.send(result);
            }
            RuntimeCommand::Shutdown => break,
        }
    }
    checking.store(false, Ordering::Release);
    active.store(false, Ordering::Release);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceSpec {
    pub kind: ServiceKind,
    pub version: &'static str,
    pub bind_address: SocketAddr,
}

impl ServiceSpec {
    pub fn from_environment(
        kind: ServiceKind,
        version: &'static str,
    ) -> Result<Self, ServiceError> {
        let raw_address = std::env::var(kind.bind_environment_key())
            .or_else(|_| std::env::var("TRPG_BIND_ADDR"))
            .unwrap_or_else(|_| format!("127.0.0.1:{}", kind.default_port()));
        let bind_address = raw_address
            .parse::<SocketAddr>()
            .map_err(|error| ServiceError {
                code: WireErrorCode::ServiceConfigurationInvalid,
                detail: format!("invalid {}: {error}", kind.bind_environment_key()),
            })?;
        Ok(Self {
            kind,
            version,
            bind_address,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceError {
    pub code: WireErrorCode,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Value,
}

impl HttpResponse {
    pub fn json(status: u16, body: Value) -> Self {
        Self { status, body }
    }
}

pub type ServiceRequestHandlerFn = dyn Fn(&HttpRequest) -> Option<HttpResponse> + Send + Sync;
pub type ServiceRequestHandler = Box<ServiceRequestHandlerFn>;

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl Error for ServiceError {}

pub fn run_service(
    spec: ServiceSpec,
    runtime_probes: Vec<RoleRuntimeProbe>,
) -> Result<(), ServiceError> {
    run_service_internal(spec, runtime_probes, None)
}

pub fn run_service_with_handler(
    spec: ServiceSpec,
    runtime_probes: Vec<RoleRuntimeProbe>,
    handler: ServiceRequestHandler,
) -> Result<(), ServiceError> {
    run_service_internal(spec, runtime_probes, Some(handler))
}

fn run_service_internal(
    spec: ServiceSpec,
    runtime_probes: Vec<RoleRuntimeProbe>,
    handler: Option<ServiceRequestHandler>,
) -> Result<(), ServiceError> {
    let limits = ServiceLimits::default();
    let worker_threads = thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(2)
        .clamp(2, 4);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .thread_name(format!("{}-http", spec.kind.as_str()))
        .enable_all()
        .build()
        .map_err(|error| ServiceError {
            code: WireErrorCode::ServiceInitializationFailed,
            detail: format!("failed to initialize async HTTP runtime: {error}"),
        })?;
    // Keep the final handler reference outside Tokio. Production handlers own
    // synchronous PostgreSQL clients whose Drop implementation starts its own
    // runtime and therefore must not run from inside this runtime's future.
    let retained_handler: Option<SharedRequestHandler> = handler.map(Arc::from);
    let result = runtime.block_on(run_service_async(
        spec,
        runtime_probes,
        retained_handler.clone(),
        limits,
    ));
    runtime.shutdown_timeout(limits.graceful_shutdown_timeout);
    drop(retained_handler);
    result
}

async fn run_service_async(
    spec: ServiceSpec,
    runtime_probes: Vec<RoleRuntimeProbe>,
    handler: Option<SharedRequestHandler>,
    limits: ServiceLimits,
) -> Result<(), ServiceError> {
    let shutdown = shutdown_signal()?;
    let listener = TcpListener::bind(spec.bind_address)
        .await
        .map_err(|error| ServiceError {
            code: WireErrorCode::ServiceInitializationFailed,
            detail: format!("failed to bind {}: {error}", spec.bind_address),
        })?;
    serve_listener(
        spec,
        runtime_probes,
        handler,
        listener,
        shutdown,
        limits,
    )
    .await
}
