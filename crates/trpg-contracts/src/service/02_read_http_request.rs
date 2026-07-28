
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
