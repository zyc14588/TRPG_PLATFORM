use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn read_http_request(
    stream: &mut TcpStream,
    limits: ServiceLimits,
) -> Result<HttpRequest, ServiceError> {
    let (bytes, header_end) = time::timeout(
        limits.header_timeout,
        read_http_headers(stream, limits),
    )
    .await
    .map_err(|_| timeout_service_error("HTTP request headers timed out"))??;
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

    let mut headers = HashMap::new();
    let mut content_length = None;
    for line in lines {
        let (raw_name, raw_value) = line.split_once(':').ok_or_else(|| ServiceError {
            code: WireErrorCode::ServiceInitializationFailed,
            detail: "invalid HTTP request header".to_owned(),
        })?;
        let name = raw_name.trim().to_ascii_lowercase();
        let value = raw_value.trim().to_owned();
        if name == "transfer-encoding" {
            return Err(ServiceError {
                code: WireErrorCode::ServiceInitializationFailed,
                detail: "Transfer-Encoding is not supported".to_owned(),
            });
        }
        if name == "content-length" {
            if content_length.is_some() {
                return Err(ServiceError {
                    code: WireErrorCode::ServiceInitializationFailed,
                    detail: "duplicate Content-Length".to_owned(),
                });
            }
            content_length = Some(value.parse::<usize>().map_err(|_| ServiceError {
                code: WireErrorCode::ServiceInitializationFailed,
                detail: "invalid Content-Length".to_owned(),
            })?);
        }
        headers.insert(name, value);
    }
    let content_length = content_length.unwrap_or(0);
    if content_length > limits.max_body_bytes {
        return Err(ServiceError {
            code: WireErrorCode::ServiceInitializationFailed,
            detail: "HTTP request body is too large".to_owned(),
        });
    }

    let body_start = header_end + 4;
    let mut body = bytes[body_start..].to_vec();
    body.truncate(content_length);
    if body.len() < content_length {
        body = time::timeout(
            limits.body_timeout,
            read_http_body(stream, body, content_length, limits.idle_timeout),
        )
        .await
        .map_err(|_| timeout_service_error("HTTP request body timed out"))??;
    }

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

async fn read_http_headers(
    stream: &mut TcpStream,
    limits: ServiceLimits,
) -> Result<(Vec<u8>, usize), ServiceError> {
    let mut bytes = Vec::with_capacity(limits.max_header_bytes.min(4_096));
    let mut buffer = [0_u8; 4_096];
    loop {
        let count = read_with_idle_timeout(stream, &mut buffer, limits.idle_timeout).await?;
        if count == 0 {
            return Err(ServiceError {
                code: WireErrorCode::ServiceInitializationFailed,
                detail: "incomplete HTTP request headers".to_owned(),
            });
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(boundary) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            if boundary > limits.max_header_bytes {
                return Err(ServiceError {
                    code: WireErrorCode::ServiceInitializationFailed,
                    detail: "HTTP request headers are too large".to_owned(),
                });
            }
            return Ok((bytes, boundary));
        }
        if bytes.len() > limits.max_header_bytes {
            return Err(ServiceError {
                code: WireErrorCode::ServiceInitializationFailed,
                detail: "HTTP request headers are too large".to_owned(),
            });
        }
    }
}

async fn read_http_body(
    stream: &mut TcpStream,
    mut body: Vec<u8>,
    content_length: usize,
    idle_timeout: Duration,
) -> Result<Vec<u8>, ServiceError> {
    let mut buffer = [0_u8; 4_096];
    while body.len() < content_length {
        let remaining = content_length - body.len();
        let read_length = remaining.min(buffer.len());
        let count =
            read_with_idle_timeout(stream, &mut buffer[..read_length], idle_timeout).await?;
        if count == 0 {
            return Err(ServiceError {
                code: WireErrorCode::ServiceInitializationFailed,
                detail: "incomplete HTTP request body".to_owned(),
            });
        }
        body.extend_from_slice(&buffer[..count]);
    }
    Ok(body)
}

async fn read_with_idle_timeout(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    idle_timeout: Duration,
) -> Result<usize, ServiceError> {
    time::timeout(idle_timeout, stream.read(buffer))
        .await
        .map_err(|_| timeout_service_error("HTTP connection idle timeout"))?
        .map_err(io_service_error)
}

async fn write_json_response(
    stream: &mut TcpStream,
    status: u16,
    body: &str,
    limits: ServiceLimits,
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
    time::timeout(limits.write_timeout, stream.write_all(response.as_bytes()))
        .await
        .map_err(|_| timeout_service_error("HTTP response write timed out"))?
        .map_err(io_service_error)
}

fn io_service_error(error: io::Error) -> ServiceError {
    ServiceError {
        code: WireErrorCode::ServiceInitializationFailed,
        detail: error.to_string(),
    }
}

#[cfg(unix)]
fn shutdown_signal() -> Result<ShutdownSignal, ServiceError> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate()).map_err(io_service_error)?;
    Ok(Box::pin(async move {
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(error) = result {
                    eprintln!("shutdown_signal_error={error}");
                }
            }
            _ = terminate.recv() => {}
        }
    }))
}

#[cfg(not(unix))]
fn shutdown_signal() -> Result<ShutdownSignal, ServiceError> {
    Ok(Box::pin(async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            eprintln!("shutdown_signal_error={error}");
        }
    }))
}
