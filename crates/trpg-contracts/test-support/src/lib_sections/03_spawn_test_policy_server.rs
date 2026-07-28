
fn spawn_test_policy_server(body: &'static str, extra_headers: &'static str) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test policy server");
    let address = listener.local_addr().expect("test policy server address");
    thread::Builder::new()
        .name("formal-commit-test-policy".to_owned())
        .spawn(move || {
            for connection in listener.incoming() {
                let Ok(mut stream) = connection else {
                    break;
                };
                if read_complete_http_request(&mut stream).is_err() {
                    continue;
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        })
        .expect("spawn test policy server");
    address
}

fn read_complete_http_request(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Ok(());
        }
        request.extend_from_slice(&buffer[..count]);
        let Some(boundary) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..boundary]);
        let content_length = headers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        if request.len() >= boundary + 4 + content_length {
            return Ok(());
        }
    }
}
