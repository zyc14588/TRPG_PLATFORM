#![forbid(unsafe_code)]

use std::env;
use std::future::Future;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

const MAX_HEADER_BYTES: usize = 8 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    name: String,
    passed: bool,
    detail: String,
}

impl Check {
    pub fn pass(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: true,
            detail: detail.into(),
        }
    }

    pub fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: false,
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Readiness {
    checks: Vec<Check>,
}

impl Readiness {
    pub fn new(checks: Vec<Check>) -> Self {
        Self { checks }
    }

    pub fn is_ready(&self) -> bool {
        !self.checks.is_empty() && self.checks.iter().all(|check| check.passed)
    }
}

pub struct ServiceConfig {
    pub service: &'static str,
    pub version: &'static str,
    pub default_bind_addr: &'static str,
    pub readiness: Readiness,
}

pub fn serve(config: ServiceConfig) -> io::Result<()> {
    validate_config(&config)?;
    let bind_addr =
        env::var("TRPG_BIND_ADDR").unwrap_or_else(|_| config.default_bind_addr.to_owned());
    let max_requests = max_requests_from_env()?;
    let listener = TcpListener::bind(&bind_addr)?;
    listener.set_nonblocking(true)?;
    let shutdown = Arc::new(AtomicBool::new(false));
    install_shutdown_handlers(&shutdown)?;
    println!("{} listening on {}", config.service, listener.local_addr()?);

    let mut served = 0usize;
    while !shutdown.load(Ordering::Relaxed) {
        let (mut stream, _) = match listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
                continue;
            }
            Err(error) => return Err(error),
        };
        if let Err(error) = handle_connection(&mut stream, &config) {
            eprintln!("{} request failed: {error}", config.service);
        }
        served += 1;
        if max_requests.is_some_and(|limit| served >= limit) {
            return Ok(());
        }
    }
    println!("{} shutdown complete", config.service);
    Ok(())
}

fn signal_runtime() -> io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}

fn install_shutdown_handlers(shutdown: &Arc<AtomicBool>) -> io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let interrupt_runtime = signal_runtime()?;
        let mut interrupt_signal = {
            let _runtime_guard = interrupt_runtime.enter();
            signal(SignalKind::interrupt())?
        };
        spawn_shutdown_waiter(
            "trpg-sigint",
            interrupt_runtime,
            Arc::clone(shutdown),
            async move { interrupt_signal.recv().await.is_some() },
        )?;

        let terminate_runtime = signal_runtime()?;
        let mut terminate_signal = {
            let _runtime_guard = terminate_runtime.enter();
            signal(SignalKind::terminate())?
        };
        spawn_shutdown_waiter(
            "trpg-sigterm",
            terminate_runtime,
            Arc::clone(shutdown),
            async move { terminate_signal.recv().await.is_some() },
        )?;
    }

    #[cfg(windows)]
    {
        let ctrl_c_runtime = signal_runtime()?;
        let mut ctrl_c_signal = {
            let _runtime_guard = ctrl_c_runtime.enter();
            tokio::signal::windows::ctrl_c()?
        };
        spawn_shutdown_waiter(
            "trpg-ctrl-c",
            ctrl_c_runtime,
            Arc::clone(shutdown),
            async move { ctrl_c_signal.recv().await.is_some() },
        )?;

        let ctrl_break_runtime = signal_runtime()?;
        let mut ctrl_break_signal = {
            let _runtime_guard = ctrl_break_runtime.enter();
            tokio::signal::windows::ctrl_break()?
        };
        spawn_shutdown_waiter(
            "trpg-ctrl-break",
            ctrl_break_runtime,
            Arc::clone(shutdown),
            async move { ctrl_break_signal.recv().await.is_some() },
        )?;
    }

    Ok(())
}

fn spawn_shutdown_waiter(
    name: &str,
    runtime: tokio::runtime::Runtime,
    shutdown: Arc<AtomicBool>,
    wait: impl Future<Output = bool> + Send + 'static,
) -> io::Result<()> {
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            if runtime.block_on(wait) {
                shutdown.store(true, Ordering::Relaxed);
            }
        })?;
    Ok(())
}

fn validate_config(config: &ServiceConfig) -> io::Result<()> {
    if config.service.trim().is_empty()
        || config.version.trim().is_empty()
        || config.default_bind_addr.trim().is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "service, version, and default bind address are required",
        ));
    }
    Ok(())
}

fn max_requests_from_env() -> io::Result<Option<usize>> {
    let Some(raw) = env::var_os("TRPG_MAX_REQUESTS") else {
        return Ok(None);
    };
    let raw = raw.into_string().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "TRPG_MAX_REQUESTS must be valid UTF-8",
        )
    })?;
    let value = raw.parse::<usize>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "TRPG_MAX_REQUESTS must be a positive integer",
        )
    })?;
    if value == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "TRPG_MAX_REQUESTS must be a positive integer",
        ));
    }
    Ok(Some(value))
}

fn handle_connection(stream: &mut TcpStream, config: &ServiceConfig) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let response = match read_request(stream) {
        Ok(request) => response_for(config, &request),
        Err(RequestError::Io(error)) => return Err(error),
        Err(RequestError::HeaderTooLarge | RequestError::Malformed) => {
            error_response(config, 400, "Bad Request", "bad_request")
        }
    };
    write_response(stream, &response)
}

#[derive(Debug, PartialEq, Eq)]
struct Request {
    method: String,
    path: String,
}

#[derive(Debug)]
enum RequestError {
    Io(io::Error),
    HeaderTooLarge,
    Malformed,
}

impl From<io::Error> for RequestError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn read_request(reader: &mut impl Read) -> Result<Request, RequestError> {
    let mut header = Vec::with_capacity(512);
    while header.len() < MAX_HEADER_BYTES {
        let mut chunk = [0u8; 512];
        let remaining = MAX_HEADER_BYTES - header.len();
        let chunk_len = remaining.min(chunk.len());
        let read = reader.read(&mut chunk[..chunk_len])?;
        if read == 0 {
            break;
        }
        header.extend_from_slice(&chunk[..read]);
        if header.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    if !header.windows(4).any(|window| window == b"\r\n\r\n") {
        return Err(if header.len() == MAX_HEADER_BYTES {
            RequestError::HeaderTooLarge
        } else {
            RequestError::Malformed
        });
    }

    let line_end = header
        .windows(2)
        .position(|window| window == b"\r\n")
        .ok_or(RequestError::Malformed)?;
    let request_line =
        std::str::from_utf8(&header[..line_end]).map_err(|_| RequestError::Malformed)?;
    let mut parts = request_line.split(' ');
    let method = parts.next().ok_or(RequestError::Malformed)?;
    let path = parts.next().ok_or(RequestError::Malformed)?;
    let version = parts.next().ok_or(RequestError::Malformed)?;
    if parts.next().is_some()
        || method.is_empty()
        || !method.bytes().all(|byte| byte.is_ascii_alphabetic())
        || !path.starts_with('/')
        || !matches!(version, "HTTP/1.0" | "HTTP/1.1")
    {
        return Err(RequestError::Malformed);
    }

    Ok(Request {
        method: method.to_owned(),
        path: path.to_owned(),
    })
}

struct Response {
    code: u16,
    reason: &'static str,
    body: String,
    allow_get: bool,
}

fn response_for(config: &ServiceConfig, request: &Request) -> Response {
    if request.method != "GET" {
        let mut response = error_response(config, 405, "Method Not Allowed", "method_not_allowed");
        response.allow_get = true;
        return response;
    }

    match request.path.as_str() {
        "/health/live" => Response {
            code: 200,
            reason: "OK",
            body: health_json(
                config,
                "live",
                &[Check::pass("listener", "request accepted by this process")],
            ),
            allow_get: false,
        },
        "/health/ready" => {
            let ready = config.readiness.is_ready();
            Response {
                code: if ready { 200 } else { 503 },
                reason: if ready { "OK" } else { "Service Unavailable" },
                body: health_json(
                    config,
                    if ready { "ready" } else { "not_ready" },
                    &config.readiness.checks,
                ),
                allow_get: false,
            }
        }
        _ => error_response(config, 404, "Not Found", "not_found"),
    }
}

fn error_response(
    config: &ServiceConfig,
    code: u16,
    reason: &'static str,
    status: &'static str,
) -> Response {
    Response {
        code,
        reason,
        body: health_json(
            config,
            status,
            &[Check::fail("request", reason.to_ascii_lowercase())],
        ),
        allow_get: false,
    }
}

fn health_json(config: &ServiceConfig, status: &str, checks: &[Check]) -> String {
    let checks = checks
        .iter()
        .map(|check| {
            format!(
                "{{\"name\":\"{}\",\"status\":\"{}\",\"detail\":\"{}\"}}",
                json_escape(&check.name),
                if check.passed { "pass" } else { "fail" },
                json_escape(&check.detail),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"service\":\"{}\",\"version\":\"{}\",\"status\":\"{}\",\"checks\":[{}]}}",
        json_escape(config.service),
        json_escape(config.version),
        json_escape(status),
        checks,
    )
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn write_response(writer: &mut impl Write, response: &Response) -> io::Result<()> {
    let allow = if response.allow_get {
        "Allow: GET\r\n"
    } else {
        ""
    };
    write!(
        writer,
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\n{}Connection: close\r\n\r\n{}",
        response.code,
        response.reason,
        response.body.len(),
        allow,
        response.body,
    )?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn config(readiness: Readiness) -> ServiceConfig {
        ServiceConfig {
            service: "test-service",
            version: "0.1.0",
            default_bind_addr: "127.0.0.1:0",
            readiness,
        }
    }

    #[test]
    fn parses_supported_health_request() {
        let mut request = Cursor::new(b"GET /health/live HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert_eq!(
            read_request(&mut request).unwrap(),
            Request {
                method: "GET".to_owned(),
                path: "/health/live".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_malformed_and_oversized_requests() {
        let mut malformed = Cursor::new(b"GET /health/live\r\n\r\n");
        assert!(matches!(
            read_request(&mut malformed),
            Err(RequestError::Malformed)
        ));

        let mut oversized = Cursor::new(vec![b'x'; MAX_HEADER_BYTES]);
        assert!(matches!(
            read_request(&mut oversized),
            Err(RequestError::HeaderTooLarge)
        ));
    }

    #[test]
    fn routes_only_get_health_endpoints() {
        let ready = config(Readiness::new(vec![Check::pass("init", "loaded")]));
        let method = response_for(
            &ready,
            &Request {
                method: "POST".to_owned(),
                path: "/health/live".to_owned(),
            },
        );
        assert_eq!(method.code, 405);
        assert!(method.allow_get);

        let missing = response_for(
            &ready,
            &Request {
                method: "GET".to_owned(),
                path: "/health/live?verbose=true".to_owned(),
            },
        );
        assert_eq!(missing.code, 404);
    }

    #[test]
    fn liveness_survives_failed_readiness() {
        let failed = config(Readiness::new(vec![Check::fail("init", "not loaded")]));
        let live = response_for(
            &failed,
            &Request {
                method: "GET".to_owned(),
                path: "/health/live".to_owned(),
            },
        );
        let ready = response_for(
            &failed,
            &Request {
                method: "GET".to_owned(),
                path: "/health/ready".to_owned(),
            },
        );
        assert_eq!(live.code, 200);
        assert_eq!(ready.code, 503);
        assert!(live.body.contains("\"status\":\"live\""));
        assert!(ready.body.contains("\"status\":\"not_ready\""));
    }

    #[test]
    fn responses_are_cross_origin_and_not_cached() {
        let response = response_for(
            &config(Readiness::new(vec![Check::pass("init", "loaded")])),
            &Request {
                method: "GET".to_owned(),
                path: "/health/live".to_owned(),
            },
        );
        let mut wire = Vec::new();
        write_response(&mut wire, &response).unwrap();
        let wire = String::from_utf8(wire).unwrap();
        assert!(wire.contains("\r\nAccess-Control-Allow-Origin: *\r\n"));
        assert!(wire.contains("\r\nCache-Control: no-store\r\n"));
    }
}
