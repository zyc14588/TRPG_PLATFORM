async fn handle_connection(
    mut socket: TcpStream,
    provider_type: ProviderType,
    model_id: &str,
    behavior: Arc<Mutex<MockBehavior>>,
    requests: Arc<Mutex<Vec<RequestMetadata>>>,
    authorization_seen: Arc<AtomicBool>,
) {
    let Some((head, body)) = read_request(&mut socket).await else {
        return;
    };
    let first_line = head.lines().next().unwrap_or_default();
    let path = first_line
        .split_ascii_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    let authorization_present = head.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with("authorization: bearer ")
    });
    authorization_seen.fetch_or(authorization_present, Ordering::Relaxed);
    let streaming = body
        .windows(br#""stream":true"#.len())
        .any(|window| window == br#""stream":true"#);
    let request_json = serde_json::from_slice::<serde_json::Value>(&body).ok();
    let thinking_disabled = request_json
        .as_ref()
        .and_then(|value| value.get("think").and_then(serde_json::Value::as_bool));
    let max_output_tokens = request_json.as_ref().and_then(|value| match provider_type {
        ProviderType::Cloud => value
            .get("max_completion_tokens")
            .and_then(serde_json::Value::as_u64),
        ProviderType::Ollama => value
            .pointer("/options/num_predict")
            .and_then(serde_json::Value::as_u64),
        ProviderType::LlamaCpp | ProviderType::LocalOpenAiCompatible => {
            value.get("max_tokens").and_then(serde_json::Value::as_u64)
        }
    });
    let reasoning_effort = request_json
        .as_ref()
        .and_then(|value| value.get("reasoning_effort"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    requests.lock().unwrap().push(RequestMetadata {
        path: path.clone(),
        streaming,
        authorization_present,
        thinking_disabled,
        max_output_tokens,
        reasoning_effort,
    });

    let behavior = *behavior.lock().unwrap();
    let is_probe = path.ends_with("/models") || path == "/api/show";
    let is_chat = path.ends_with("/chat/completions") || path == "/api/chat";

    if is_probe {
        if let MockBehavior::ProbeStatus(status) = behavior {
            write_json_response(&mut socket, status, "{}").await;
            return;
        }
        let capabilities = if behavior == MockBehavior::CapabilitiesWithoutTools {
            r#"{"chat":true,"streaming":true,"structured_output":true,"tool_requests":false,"embeddings":true}"#
        } else {
            r#"{"chat":true,"streaming":true,"structured_output":true,"tool_requests":true,"embeddings":true}"#
        };
        let response = if provider_type == ProviderType::Ollama {
            format!(r#"{{"model_info":{{}},"capabilities":{capabilities}}}"#)
        } else {
            format!(r#"{{"data":[{{"id":"{model_id}","capabilities":{capabilities}}}]}}"#)
        };
        write_json_response(&mut socket, 200, &response).await;
        return;
    }

    if is_chat {
        match behavior {
            MockBehavior::ChatDelay(milliseconds) => {
                tokio::time::sleep(Duration::from_millis(milliseconds)).await;
            }
            MockBehavior::ChatStatus(status) => {
                write_json_response(&mut socket, status, "{}").await;
                return;
            }
            MockBehavior::ChatInvalidJson if !streaming => {
                write_json_response(&mut socket, 200, "{invalid").await;
                return;
            }
            MockBehavior::StreamDisconnect if streaming => {
                let body = if provider_type == ProviderType::Ollama {
                    "{\"message\":{\"content\":\"partial\"},\"done\":false}\n"
                } else {
                    "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n"
                };
                write_stream_response(&mut socket, body).await;
                return;
            }
            _ => {}
        }

        if streaming {
            let response = if provider_type == ProviderType::Ollama {
                concat!(
                    "{\"message\":{\"content\":\"The \"},\"done\":false}\n",
                    "{\"message\":{\"content\":\"door opens.\"},\"done\":true}\n"
                )
            } else {
                concat!(
                    "data: {\"choices\":[{\"delta\":{\"content\":\"The \"}}]}\n\n",
                    "data: {\"choices\":[{\"delta\":{\"content\":\"door opens.\"}}]}\n\n",
                    "data: [DONE]\n\n"
                )
            };
            write_stream_response(&mut socket, response).await;
            return;
        }

        let duplicate = behavior == MockBehavior::ChatDuplicateToolCall;
        let response = if provider_type == ProviderType::Ollama {
            let second = if duplicate {
                r#",{"id":"tool-call-1","function":{"name":"search_clue","arguments":{"query":"desk"}}}"#
            } else {
                ""
            };
            format!(
                r#"{{"message":{{"content":"{{\"scene\":\"library\"}}","tool_calls":[{{"id":"tool-call-1","function":{{"name":"search_clue","arguments":{{"query":"clue"}}}}}}{second}]}},"prompt_eval_count":7,"eval_count":5}}"#
            )
        } else {
            let second = if duplicate {
                r#",{"id":"tool-call-1","type":"function","function":{"name":"search_clue","arguments":"{\"query\":\"desk\"}"}}"#
            } else {
                ""
            };
            format!(
                r#"{{"choices":[{{"message":{{"content":"{{\"scene\":\"library\"}}","tool_calls":[{{"id":"tool-call-1","type":"function","function":{{"name":"search_clue","arguments":"{{\"query\":\"clue\"}}"}}}}{second}]}}}}],"usage":{{"prompt_tokens":7,"completion_tokens":5}}}}"#
            )
        };
        write_json_response(&mut socket, 200, &response).await;
        return;
    }

    let response = if provider_type == ProviderType::Ollama {
        r#"{"embeddings":[[0.1,0.2,0.3]],"prompt_eval_count":3}"#
    } else {
        r#"{"data":[{"embedding":[0.1,0.2,0.3]}],"usage":{"prompt_tokens":3}}"#
    };
    write_json_response(&mut socket, 200, response).await;
}

async fn read_request(socket: &mut TcpStream) -> Option<(String, Vec<u8>)> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let header_end = loop {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > 2 * 1024 * 1024 {
            return None;
        }
        if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let head = String::from_utf8(request[..header_end].to_vec()).ok()?;
    let content_length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    while request.len() < header_end.saturating_add(content_length) {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        request.extend_from_slice(&buffer[..read]);
    }
    Some((
        head,
        request[header_end..header_end + content_length].to_vec(),
    ))
}

async fn write_json_response(socket: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.shutdown().await;
}

async fn write_stream_response(socket: &mut TcpStream, body: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.shutdown().await;
}
