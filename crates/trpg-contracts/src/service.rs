use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::json;
use serde_json::Value;

use crate::{
    validate_event_registry, ComponentCheck, HealthState, ServiceKind, ServicePhase, WireErrorCode,
};

static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
static SERVING: AtomicBool = AtomicBool::new(false);

const READINESS_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_HTTP_REQUEST_BYTES: usize = 65_536;

type ReadinessResult = Result<String, String>;

enum RuntimeCommand {
    Check(SyncSender<ReadinessResult>),
    Shutdown,
}

pub struct RoleRuntimeProbe {
    name: &'static str,
    sender: Sender<RuntimeCommand>,
    worker: Option<JoinHandle<()>>,
}

impl RoleRuntimeProbe {
    pub fn spawn<F>(name: &'static str, check: F) -> Result<Self, ServiceError>
    where
        F: Fn() -> ReadinessResult + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name(format!("{name}-loop"))
            .spawn(move || role_runtime_loop(receiver, check))
            .map_err(|error| ServiceError {
                code: WireErrorCode::ServiceInitializationFailed,
                detail: format!("failed to start {name}: {error}"),
            })?;
        Ok(Self {
            name,
            sender,
            worker: Some(worker),
        })
    }

    pub fn component_check(&self) -> ComponentCheck {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        if self
            .sender
            .send(RuntimeCommand::Check(reply_sender))
            .is_err()
        {
            return ComponentCheck::failing(self.name, "runtime loop is not running");
        }
        match reply_receiver.recv_timeout(READINESS_TIMEOUT) {
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

    pub fn shutdown(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = self.sender.send(RuntimeCommand::Shutdown);
            let _ = worker.join();
        }
    }
}

impl Drop for RoleRuntimeProbe {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn role_runtime_loop<F>(receiver: Receiver<RuntimeCommand>, check: F)
where
    F: Fn() -> ReadinessResult,
{
    while let Ok(command) = receiver.recv() {
        match command {
            RuntimeCommand::Check(reply) => {
                let _ = reply.send(check());
            }
            RuntimeCommand::Shutdown => break,
        }
    }
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
    mut runtime_probes: Vec<RoleRuntimeProbe>,
    handler: Option<ServiceRequestHandler>,
) -> Result<(), ServiceError> {
    SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);
    SERVING.store(false, Ordering::SeqCst);
    install_shutdown_handlers()?;

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

    let listener = TcpListener::bind(spec.bind_address).map_err(|error| ServiceError {
        code: WireErrorCode::ServiceInitializationFailed,
        detail: format!("failed to bind {}: {error}", spec.bind_address),
    })?;
    listener.set_nonblocking(true).map_err(io_service_error)?;
    component_checks.push(ComponentCheck::passing(
        "listener",
        format!("local={}", listener.local_addr().map_err(io_service_error)?),
    ));

    let health = current_health(&spec, &component_checks, &runtime_probes);
    SERVING.store(true, Ordering::SeqCst);
    eprintln!(
        "service={} state={} listening={}",
        spec.kind.as_str(),
        health.phase.as_str(),
        listener.local_addr().map_err(io_service_error)?
    );

    while !SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let health = current_health(&spec, &component_checks, &runtime_probes);
                handle_connection(&mut stream, &health, handler.as_deref())?;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(io_service_error(error)),
        }
    }

    SERVING.store(false, Ordering::SeqCst);
    for probe in &mut runtime_probes {
        probe.shutdown();
    }
    eprintln!("service={} state=stopped", spec.kind.as_str());
    Ok(())
}

fn current_health(
    spec: &ServiceSpec,
    component_checks: &[ComponentCheck],
    runtime_probes: &[RoleRuntimeProbe],
) -> HealthState {
    let mut checks = component_checks.to_vec();
    checks.extend(runtime_probes.iter().map(RoleRuntimeProbe::component_check));
    let phase = if checks.iter().all(|check| check.ready) {
        ServicePhase::Ready
    } else {
        ServicePhase::Degraded
    };
    HealthState::new(spec.kind, spec.version, phase, checks)
}

fn handle_connection(
    stream: &mut TcpStream,
    health: &HealthState,
    handler: Option<&ServiceRequestHandlerFn>,
) -> Result<(), ServiceError> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(io_service_error)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(io_service_error)?;

    let request = read_http_request(stream)?;

    let response = match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/health/live") => {
            let serving = SERVING.load(Ordering::SeqCst);
            HttpResponse::json(
                if health.live(serving) { 200 } else { 503 },
                health.live_document(serving),
            )
        }
        ("GET", "/health/ready") => HttpResponse::json(
            if health.ready() { 200 } else { 503 },
            health.ready_document(),
        ),
        _ => handler
            .and_then(|handler| handler(&request))
            .unwrap_or_else(|| {
                if request.method == "GET" {
                    HttpResponse::json(404, json!({"error": "NOT_FOUND"}))
                } else {
                    HttpResponse::json(405, json!({"error": "METHOD_NOT_ALLOWED"}))
                }
            }),
    };
    write_json_response(stream, response.status, &response.body.to_string())
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, ServiceError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let (header_end, content_length) = loop {
        let count = stream.read(&mut buffer).map_err(io_service_error)?;
        if count == 0 || bytes.len().saturating_add(count) > MAX_HTTP_REQUEST_BYTES {
            return Err(ServiceError {
                code: WireErrorCode::ServiceInitializationFailed,
                detail: "invalid or oversized HTTP request".to_owned(),
            });
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(boundary) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let header_text =
                std::str::from_utf8(&bytes[..boundary]).map_err(|_| ServiceError {
                    code: WireErrorCode::ServiceInitializationFailed,
                    detail: "HTTP request headers are not UTF-8".to_owned(),
                })?;
            let content_length = header_text
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<usize>())
                .transpose()
                .map_err(|_| ServiceError {
                    code: WireErrorCode::ServiceInitializationFailed,
                    detail: "invalid Content-Length".to_owned(),
                })?
                .unwrap_or(0);
            if boundary + 4 + content_length > MAX_HTTP_REQUEST_BYTES {
                return Err(ServiceError {
                    code: WireErrorCode::ServiceInitializationFailed,
                    detail: "HTTP request body is too large".to_owned(),
                });
            }
            if bytes.len() >= boundary + 4 + content_length {
                break (boundary, content_length);
            }
        }
    };

    let header_text = std::str::from_utf8(&bytes[..header_end]).map_err(|_| ServiceError {
        code: WireErrorCode::ServiceInitializationFailed,
        detail: "HTTP request headers are not UTF-8".to_owned(),
    })?;
    let mut lines = header_text.lines();
    let mut request_line = lines.next().unwrap_or_default().split_whitespace();
    let method = request_line.next().unwrap_or_default().to_owned();
    let path = request_line.next().unwrap_or_default().to_owned();
    if method.is_empty() || !path.starts_with('/') {
        return Err(ServiceError {
            code: WireErrorCode::ServiceInitializationFailed,
            detail: "invalid HTTP request line".to_owned(),
        });
    }
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Ok(HttpRequest {
        method,
        path,
        headers,
        body: bytes[header_end + 4..header_end + 4 + content_length].to_vec(),
    })
}

fn write_json_response(
    stream: &mut TcpStream,
    status: u16,
    body: &str,
) -> Result<(), ServiceError> {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .map_err(io_service_error)
}

fn io_service_error(error: io::Error) -> ServiceError {
    ServiceError {
        code: WireErrorCode::ServiceInitializationFailed,
        detail: error.to_string(),
    }
}

#[cfg(unix)]
fn install_shutdown_handlers() -> Result<(), ServiceError> {
    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;
    const SIG_ERR: usize = usize::MAX;

    unsafe extern "C" {
        fn signal(signal: i32, handler: usize) -> usize;
    }

    extern "C" fn request_shutdown(_: i32) {
        SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
    }

    // SAFETY: the handler only performs an atomic store, and both signal numbers are POSIX-defined.
    let int_result = unsafe { signal(SIGINT, request_shutdown as *const () as usize) };
    // SAFETY: same handler and contract as the SIGINT registration above.
    let term_result = unsafe { signal(SIGTERM, request_shutdown as *const () as usize) };
    if int_result == SIG_ERR || term_result == SIG_ERR {
        return Err(ServiceError {
            code: WireErrorCode::ServiceInitializationFailed,
            detail: "failed to install shutdown signal handlers".to_owned(),
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn install_shutdown_handlers() -> Result<(), ServiceError> {
    Ok(())
}
